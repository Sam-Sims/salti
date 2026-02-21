use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{
    Event as TermEvent, EventStream, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::cli::StartupState;
use crate::config::keybindings::{self, KeyAction};
use crate::core::column_stats::{ColumnStats, ColumnStatsRequest, compute_column_stats};
use crate::core::command::CoreAction;
use crate::core::parser::{self, Alignment};
use crate::core::{CoreState, LoadingState};
use crate::ui::layout::{AppLayout, FrameLayout};
use crate::ui::render;
use crate::ui::selection::selection_point_crosshair;
use crate::ui::{MouseSelection, UiAction, UiState};

/// step size (rows) for single scroll commands
const SCROLL_STEP: usize = 1;
/// step size (rows) for fast scroll commands
const FAST_SCROLL_STEP: usize = 10;
/// fps of render loop
const RENDER_FPS: f32 = 120.0;

#[derive(Debug)]
pub enum Action {
    Core(CoreAction),
    Ui(UiAction),
    LoadFile { input: String },
    Quit,
}

#[derive(Debug)]
struct AsyncJob<T> {
    handle: JoinHandle<T>,
    cancel: CancellationToken,
}

#[derive(Debug)]
pub struct App {
    core: CoreState,
    ui: UiState,
    should_quit: bool,
    layout_area: Rect,
    frame_layout: FrameLayout,
    app_layout: AppLayout,
    load_job: Option<AsyncJob<Result<Vec<Alignment>, String>>>,
    stats_job: Option<AsyncJob<ColumnStats>>,
}

impl App {
    #[must_use]
    pub fn new(startup: StartupState) -> Self {
        let layout_area = Rect::new(0, 0, 0, 0);
        let frame_layout = FrameLayout::new(layout_area);
        let app_layout = AppLayout::new(frame_layout.content_area);
        Self {
            core: CoreState::new(startup),
            ui: UiState::default(),
            should_quit: false,
            layout_area,
            frame_layout,
            app_layout,
            load_job: None,
            stats_job: None,
        }
    }

    /// Updates cached layouts and viewport dimensions from a terminal area.
    ///
    /// Draw-time `Frame::area()` is the authoritative source. Backend resize events and
    /// `terminal.size()` values are only bootstrap hints before the first draw.
    fn update_layout(&mut self, area: Rect) {
        if area == self.layout_area {
            return;
        }
        self.layout_area = area;
        self.frame_layout = FrameLayout::new(area);
        self.app_layout = AppLayout::new(self.frame_layout.content_area);

        let visible_width = self.app_layout.alignment_pane.width.saturating_sub(2) as usize;
        let visible_height = self.app_layout.alignment_pane.height.saturating_sub(4) as usize;
        let number_width = self.core.data.sequences.len().max(1).to_string().len();
        let number_prefix_width = number_width + 1;
        let name_visible_width = self
            .app_layout
            .sequence_id_pane
            .width
            .saturating_sub(2)
            .saturating_sub(number_prefix_width as u16) as usize;

        debug!(
            terminal_width = area.width,
            terminal_height = area.height,
            visible_width,
            visible_height,
            name_visible_width,
            "Terminal resized, viewport updated:"
        );

        self.core
            .update_viewport_dimensions(visible_width, visible_height, name_visible_width);
        self.rebuild_visible_rows();
    }

    /// Rebuilds the list of visible rows in the UI state
    fn rebuild_visible_rows(&mut self) {
        let row_capacity = self.app_layout.alignment_pane_sequence_rows.height as usize;
        let window = self.core.viewport.window();
        self.ui
            .rebuild_visible_rows(&self.core, &window, row_capacity);
    }

    /// Event handler for mouse events when the minimap is open.
    ///
    /// Minimap has own handler which will return the action to run
    fn handle_minimap_mouse_event(&mut self, mouse: MouseEvent) {
        let Some(minimap) = self.ui.overlay.minimap.as_mut() else {
            return;
        };

        let window = self.core.viewport.window();
        if let Some(action) = minimap.handle_mouse(
            mouse,
            self.frame_layout.overlay_area,
            &window.col_range,
            self.core.data.sequence_length,
        ) {
            self.apply_actions([Action::Core(action)]);
        }
    }

    /// Spawns an async task to load alignments from an input source and cancels any previous load task.
    fn start_load_job(&mut self, input: String) {
        if let Some(previous) = self.load_job.take() {
            debug!("Previous load job found, cancelling");
            previous.cancel.cancel();
            previous.handle.abort();
        }

        self.core.data.file_path = Some(input.clone());

        let cancel = CancellationToken::new();
        debug!(input = %input, "Spawning new load job for input");
        let handle = tokio::task::spawn_blocking({
            let cancel = cancel.clone();
            move || parser::parse_fasta_file(&input, &cancel).map_err(|error| error.to_string())
        });

        self.load_job = Some(AsyncJob { handle, cancel });
    }

    /// Try and load an alignment file.
    ///
    /// If no file path is configured, loading is marked as [`LoadingState::Idle`] so the UI can
    /// present an idle status. If a path exists, a load job is spawned immediately.
    fn try_file_load(&mut self) {
        let Some(input) = self.core.data.file_path.clone() else {
            info!("No startup file provided; entering idle loading state");
            self.core.loading_state = LoadingState::Idle;
            return;
        };
        debug!(input = %input, "Loading startup alignment");
        self.start_load_job(input);
    }

    /// Cancels any running stats job and spawns a new one for the given request.
    fn spawn_stats_job(&mut self, request: ColumnStatsRequest) {
        if let Some(previous) = self.stats_job.take() {
            debug!("Previous stats job found, cancelling");
            previous.cancel.cancel();
            previous.handle.abort();
        }
        if request.positions.is_empty() {
            debug!("Skipping stats job spawn because request has no positions");
            return;
        }
        debug!("Spawning new stats job for viewport");
        let cancel = CancellationToken::new();
        let handle = tokio::task::spawn_blocking({
            let cancel = cancel.clone();
            move || {
                let ColumnStatsRequest {
                    sequences,
                    positions,
                    method,
                    sequence_type,
                } = request;

                compute_column_stats(
                    sequences.as_slice(),
                    &positions,
                    method,
                    sequence_type,
                    &cancel,
                )
            }
        });
        self.stats_job = Some(AsyncJob { handle, cancel });
    }

    /// Checks if the viewport has crossed the stats margin threshold and spawns a new stats job if needed.
    ///
    /// Called after horizontal scrolls, jumps, and resizes etc.
    fn extend_stats_if_needed(&mut self) {
        if !self.core.viewport_crossed_margin() {
            return;
        }
        let request = self.core.build_column_stats_request();
        self.spawn_stats_job(request);
    }

    /// Invalidates cached column stats and spawns a new stats job for the current viewport and sequence set.
    ///
    /// Called after state changes that alter the visible sequence set, consensus
    /// method, or sequence type etc.
    fn invalidate_and_recompute_stats(&mut self) {
        self.core.invalidate_column_stats();
        let request = self.core.build_column_stats_request();
        self.spawn_stats_job(request);
    }

    /// Maps a keybinding action to an app action and applies it.
    fn handle_key_action(&mut self, action: KeyAction) {
        let action = match action {
            KeyAction::Quit => Action::Quit,
            KeyAction::OpenCommandPalette => Action::Ui(UiAction::OpenCommandPalette),
            KeyAction::ToggleMinimap => Action::Ui(UiAction::ToggleMinimap),
            KeyAction::ToggleTranslationView => Action::Core(CoreAction::ToggleTranslationView),
            KeyAction::JumpToStart => {
                if self.core.data.sequence_length > 0 {
                    Action::Core(CoreAction::JumpToPosition(0))
                } else {
                    return;
                }
            }
            KeyAction::JumpToEnd => {
                let Some(position) = self.core.data.sequence_length.checked_sub(1) else {
                    return;
                };
                Action::Core(CoreAction::JumpToPosition(position))
            }
            KeyAction::ScrollDown => Action::Core(CoreAction::ScrollDown {
                amount: SCROLL_STEP,
            }),
            KeyAction::SkipDown => Action::Core(CoreAction::ScrollDown {
                amount: FAST_SCROLL_STEP,
            }),
            KeyAction::ScrollUp => Action::Core(CoreAction::ScrollUp {
                amount: SCROLL_STEP,
            }),
            KeyAction::SkipUp => Action::Core(CoreAction::ScrollUp {
                amount: FAST_SCROLL_STEP,
            }),
            KeyAction::ScrollLeft => Action::Core(CoreAction::ScrollLeft {
                amount: SCROLL_STEP,
            }),
            KeyAction::SkipLeft => Action::Core(CoreAction::ScrollLeft {
                amount: FAST_SCROLL_STEP,
            }),
            KeyAction::ScrollRight => Action::Core(CoreAction::ScrollRight {
                amount: SCROLL_STEP,
            }),
            KeyAction::SkipRight => Action::Core(CoreAction::ScrollRight {
                amount: FAST_SCROLL_STEP,
            }),
            KeyAction::ScrollNamesLeft => Action::Core(CoreAction::ScrollNamesLeft {
                amount: SCROLL_STEP,
            }),
            KeyAction::ScrollNamesRight => Action::Core(CoreAction::ScrollNamesRight {
                amount: SCROLL_STEP,
            }),
        };
        self.apply_actions([action]);
    }

    /// Applies a sequence of [`Action`] values in order and performs their side effects immediately.
    ///
    /// Actions are either core commands, UI actions, or quit signals and define how the app state
    /// should change in response to user input or async events.
    ///
    /// Core actions are forwarded to [`CoreState::apply_action`].
    ///
    /// UI actions are applied to [`UiState`] via its own handler.
    ///
    /// There are 2 direct actions that dont fall under core/ui: [`Action::Quit`] and [`Action::LoadFile`]:
    /// [`Action::Quit`] sets `should_quit`, which causes the main event loop to exit on its next check.
    /// [`Action::LoadFile`] sets the file path in core state and starts a new async load job, cancelling any previous job if it exists.
    /// task
    fn apply_actions<I>(&mut self, actions: I)
    where
        I: IntoIterator<Item = Action>,
    {
        for action in actions {
            match action {
                Action::Core(action) => {
                    // some actions change the visible alignment in a way that would make any existing mouse selection invalid,
                    // they are specified here to be cleared after action is applied
                    // add to this list as needed if actions are added that should reset the mouse selection
                    let resets_mouse_selection = matches!(
                        &action,
                        CoreAction::SetFilter { .. }
                            | CoreAction::ClearFilter
                            | CoreAction::SetReference(_)
                            | CoreAction::ClearReference
                            | CoreAction::PinSequence(_)
                    );
                    // actions that should invalidate the cached column stats and SHOULD trigger a full re-run
                    // these are actions that change the visible sequence set, or modify the alignments
                    // in a way that could change the stats for any column in the viewport
                    let invalidates_stats = matches!(
                        &action,
                        CoreAction::SetFilter { .. }
                            | CoreAction::ClearFilter
                            | CoreAction::SetReference(_)
                            | CoreAction::ClearReference
                            | CoreAction::SetConsensusMethod(_)
                            | CoreAction::SetSequenceType(_)
                    );
                    // actions that move the viewport horizontally and COULD trigger a stats update if the margin threshold is crossed,
                    // but dont necessarily require one on every execution like the invalidating actions above
                    // define any horizontal move actions in this list to ensure the check is performed after they run
                    let extends_stats = matches!(
                        &action,
                        CoreAction::ScrollLeft { .. }
                            | CoreAction::ScrollRight { .. }
                            | CoreAction::JumpToPosition(_)
                    );

                    self.core.apply_action(action);
                    self.rebuild_visible_rows();

                    // handle the side effects defined above
                    if resets_mouse_selection {
                        self.ui.mouse.clear_all();
                    }
                    if invalidates_stats {
                        self.invalidate_and_recompute_stats();
                    } else if extends_stats {
                        self.extend_stats_if_needed();
                    }
                }
                Action::Ui(action) => {
                    self.ui.apply_action(action, &self.core);
                }
                Action::LoadFile { input } => {
                    self.ui.mouse.clear_all();
                    self.start_load_job(input);
                }
                Action::Quit => {
                    self.should_quit = true;
                }
            }
        }
    }

    /// Event handler for mouse input.
    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        if self.ui.overlay.minimap.is_some() {
            self.handle_minimap_mouse_event(mouse);
            return;
        }

        let crosshair = selection_point_crosshair(
            &self.core,
            &self.ui.visible_rows,
            self.app_layout.alignment_pane_sequence_rows,
            mouse.column,
            mouse.row,
        );

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some((sequence_id, column)) = crosshair else {
                    // user can click outside the alignment pane to clear a selection.
                    self.ui.mouse.clear_all();
                    return;
                };

                // handle box mode modifier with CTRL
                self.ui.mouse.box_anchor = mouse
                    .modifiers
                    .contains(KeyModifiers::CONTROL)
                    .then_some((sequence_id, column));
                self.ui.mouse.selection = Some(MouseSelection {
                    sequence_id,
                    column,
                    end_sequence_id: sequence_id,
                    end_column: column,
                });
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some((sequence_id, column)) = crosshair else {
                    return;
                };

                let (start_seq, start_col) =
                    self.ui.mouse.box_anchor.unwrap_or((sequence_id, column));
                self.ui.mouse.selection = Some(MouseSelection {
                    sequence_id: start_seq,
                    column: start_col,
                    end_sequence_id: sequence_id,
                    end_column: column,
                });
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let Some((sequence_id, column)) = crosshair else {
                    self.ui.mouse.clear_anchors();
                    return;
                };

                let (start_seq, start_col) =
                    self.ui.mouse.box_anchor.unwrap_or((sequence_id, column));
                self.ui.mouse.selection = Some(MouseSelection {
                    sequence_id: start_seq,
                    column: start_col,
                    end_sequence_id: sequence_id,
                    end_column: column,
                });
                self.ui.mouse.clear_anchors();
            }
            MouseEventKind::Down(MouseButton::Middle) => {
                self.ui.mouse.pan_anchor = Some((mouse.column, mouse.row));
            }
            MouseEventKind::Drag(MouseButton::Middle) => {
                let actions = self
                    .ui
                    .mouse
                    .pan_drag_actions(mouse.column, mouse.row)
                    .into_iter()
                    .flatten()
                    .map(Action::Core);
                self.apply_actions(actions);
            }
            MouseEventKind::Up(MouseButton::Middle) => {
                self.ui.mouse.pan_anchor = None;
            }
            _ => {}
        }
    }

    /// Event handler for key input.
    ///
    /// When the command palette is open, all key input is passed to the palette until it closes
    /// Otherwise, the key is resolved through configured keybindings. Unbound keys are ignored.
    fn handle_key_event(&mut self, key: KeyEvent) {
        self.ui
            .apply_action(UiAction::ClearCommandError, &self.core);

        // if a palette is open, all key events go to it until it's closed
        if let Some(palette) = self.ui.overlay.palette.as_mut() {
            // the palette will only ever return an action, even if its an action to close itself,
            // so they are immediately applied
            let actions = palette.handle_key_event(key);
            self.apply_actions(actions);
            return;
        }
        if let Some(action) = keybindings::lookup(key.code, key.modifiers) {
            self.handle_key_action(action);
        }
    }

    /// Entrypoint for main app loop
    ///
    /// The loop handles four event sources:
    /// - frame ticks at `RENDER_TARGET_FPS` for rendering,
    /// - terminal input (including resize)
    /// - alignment load job completion
    /// - column stats job completion
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        info!(target_fps = RENDER_FPS, "Starting runtime");

        // will try and load input fasta file immediately
        // if none, core is set to idle and a status message is displayed
        self.try_file_load();

        match terminal.size() {
            Ok(area) => {
                debug!(
                    width = area.width,
                    height = area.height,
                    "Captured initial terminal size"
                );
                self.update_layout(area.into());
            }
            Err(error_value) => {
                warn!(error = ?error_value, "Failed to capture initial terminal size");
            }
        }

        self.extend_stats_if_needed();

        let period = Duration::from_secs_f32(1.0 / RENDER_FPS);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();
        let mut needs_redraw = true;

        // main event loop
        // TODO: maybe let ctrl+c break the loop
        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => {
                    if needs_redraw {
                        if let Err(error_value) = terminal.draw(|frame| {
                            self.update_layout(frame.area());
                            render(
                                frame,
                                &self.core,
                                &self.ui,
                                &self.frame_layout,
                                &self.app_layout,
                            )
                        }) {
                            error!(error = ?error_value, "terminal draw failed");
                            return Err(error_value.into());
                        }
                        needs_redraw = false;
                    }
                },
                Some(Ok(event)) = events.next() => {
                    match event {
                        TermEvent::Resize(width, height) => {
                            self.update_layout(Rect::new(0, 0, width, height));
                            // a resize can enlarge the window size, and so could require new stats values
                            self.extend_stats_if_needed();
                        }
                        TermEvent::Key(key) => {
                            self.handle_key_event(key);
                        }
                        TermEvent::Mouse(mouse) => {
                            if self.ui.overlay.palette.is_none() {
                                self.handle_mouse_event(mouse);
                            }
                        }
                        _ => {}
                    }

                    needs_redraw = true;
                }

                // alignment load completion
                Some(join_result) = async {
                    match self.load_job.as_mut() {
                        Some(job) => Some((&mut job.handle).await),
                        None => None,
                    }
                } => {
                    self.load_job = None;
                    match join_result {
                        Ok(result) => {
                            self.core.handle_alignments_loaded(result);
                            self.rebuild_visible_rows();
                            self.invalidate_and_recompute_stats();
                        }
                        Err(join_error) => {
                            if !join_error.is_cancelled() {
                                error!(error = ?join_error, "Alignment load task panicked");
                            }
                        }
                    }
                    needs_redraw = true;
                }

                // column stats completion
                Some(join_result) = async {
                    match self.stats_job.as_mut() {
                        Some(job) => Some((&mut job.handle).await),
                        None => None,
                    }
                } => {
                    self.stats_job = None;
                    match join_result {
                        Ok(stats) => {
                            self.core.apply_column_stats(stats);
                        }
                        Err(join_error) => {
                            if !join_error.is_cancelled() {
                                error!(error = ?join_error, "Column stats task panicked");
                            }
                        }
                    }
                    needs_redraw = true;
                }
            }
        }

        info!("Quit requested, cancelling background tasks");
        if let Some(job) = self.load_job.take() {
            job.cancel.cancel();
            job.handle.abort();
        }
        if let Some(job) = self.stats_job.take() {
            job.cancel.cancel();
            job.handle.abort();
        }
        Ok(())
    }
}
