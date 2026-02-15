use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{
    Event as TermEvent, EventStream, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, trace, warn};

use crate::cli::StartupState;
use crate::config::keybindings::{self, KeyAction};
use crate::core::command::CoreAction;
use crate::core::jobs::{ConsensusRequest, spawn_consensus_worker};
use crate::core::{CoreAsyncEvent, CoreState, LoadingState};
use crate::ui::layout::AppLayout;
use crate::ui::render;
use crate::ui::selection::visible_sequence_rows;
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
    Quit,
}

#[derive(Debug)]
pub struct App {
    core: CoreState,
    ui: UiState,
    should_quit: bool,
    terminal_size: Rect,
    box_selection_anchor: Option<(usize, usize)>,
    middle_pan_anchor: Option<(u16, u16)>,
}

impl App {
    #[must_use]
    pub fn new(startup: StartupState) -> Self {
        Self {
            core: CoreState::new(startup),
            ui: UiState::default(),
            should_quit: false,
            terminal_size: Rect::new(0, 0, 0, 0),
            box_selection_anchor: None,
            middle_pan_anchor: None,
        }
    }

    #[must_use]
    fn terminal_area(&self) -> Rect {
        Rect::new(
            0,
            0,
            self.terminal_size.width,
            self.terminal_size.height.saturating_sub(1),
        )
    }

    /// Applies a sequence of [`Action`] values in order and performs their side effects immediately.
    ///
    /// Actions are either core commands, UI actions, or quit signals and define how the app state
    /// should change in response to user input or async events.
    ///
    /// Core actions are forwarded to [`CoreState::apply_action`], which may run async functions
    /// via the `*_tx` channels.
    ///
    /// UI actions are applied to [`UiState`] via its own handler.
    ///
    /// [`Action::Quit`] sets `should_quit`, which causes the main event loop to exit on its next check.
    fn apply_actions<I>(
        &mut self,
        actions: I,
        async_tx: &mpsc::Sender<CoreAsyncEvent>,
        consensus_tx: &tokio::sync::watch::Sender<Option<ConsensusRequest>>,
    ) where
        I: IntoIterator<Item = Action>,
    {
        for action in actions {
            match action {
                Action::Core(action) => {
                    let resets_mouse_selection = matches!(
                        action,
                        CoreAction::SetFilter { .. }
                            | CoreAction::ClearFilter
                            | CoreAction::SetReference(_)
                            | CoreAction::ClearReference
                            | CoreAction::LoadAlignment { .. }
                    );
                    trace!(?action, "dispatching core action");
                    self.core.apply_action(action, async_tx, consensus_tx);
                    if resets_mouse_selection {
                        self.clear_box_selection_anchor();
                        self.ui
                            .apply_action(UiAction::ClearMouseSelection, &self.core);
                    }
                }
                Action::Ui(action) => {
                    self.ui.apply_action(action, &self.core);
                }
                Action::Quit => {
                    self.should_quit = true;
                }
            }
        }
    }

    /// Simple match to convert keybinding actions where extra logic isnt needed
    ///
    /// `match_action` handles key actions that require extra logic,
    #[must_use]
    fn map_key_action(action: KeyAction) -> Option<Action> {
        match action {
            KeyAction::Quit => Some(Action::Quit),
            KeyAction::OpenCommandPalette => Some(Action::Ui(UiAction::OpenCommandPalette)),
            KeyAction::ToggleTranslationView => {
                Some(Action::Core(CoreAction::ToggleTranslationView))
            }
            KeyAction::ScrollDown => Some(Action::Core(CoreAction::ScrollDown {
                amount: SCROLL_STEP,
            })),
            KeyAction::SkipDown => Some(Action::Core(CoreAction::ScrollDown {
                amount: FAST_SCROLL_STEP,
            })),
            KeyAction::ScrollUp => Some(Action::Core(CoreAction::ScrollUp {
                amount: SCROLL_STEP,
            })),
            KeyAction::SkipUp => Some(Action::Core(CoreAction::ScrollUp {
                amount: FAST_SCROLL_STEP,
            })),
            KeyAction::ScrollLeft => Some(Action::Core(CoreAction::ScrollLeft {
                amount: SCROLL_STEP,
            })),
            KeyAction::SkipLeft => Some(Action::Core(CoreAction::ScrollLeft {
                amount: FAST_SCROLL_STEP,
            })),
            KeyAction::ScrollRight => Some(Action::Core(CoreAction::ScrollRight {
                amount: SCROLL_STEP,
            })),
            KeyAction::SkipRight => Some(Action::Core(CoreAction::ScrollRight {
                amount: FAST_SCROLL_STEP,
            })),
            KeyAction::ScrollNamesLeft => Some(Action::Core(CoreAction::ScrollNamesLeft {
                amount: SCROLL_STEP,
            })),
            KeyAction::ScrollNamesRight => Some(Action::Core(CoreAction::ScrollNamesRight {
                amount: SCROLL_STEP,
            })),
            KeyAction::JumpToStart | KeyAction::JumpToEnd => None,
        }
    }

    /// Maps a keybinding action to an app action with any extra logic needed and applies it.
    fn match_action(
        &mut self,
        action: KeyAction,
        async_tx: &mpsc::Sender<CoreAsyncEvent>,
        consensus_tx: &tokio::sync::watch::Sender<Option<ConsensusRequest>>,
    ) {
        match action {
            KeyAction::JumpToStart => {
                if self.core.data.sequence_length > 0 {
                    self.apply_actions(
                        [Action::Core(CoreAction::JumpToPosition(0))],
                        async_tx,
                        consensus_tx,
                    );
                }
            }
            KeyAction::JumpToEnd => {
                if let Some(position) = self.core.data.sequence_length.checked_sub(1) {
                    self.apply_actions(
                        [Action::Core(CoreAction::JumpToPosition(position))],
                        async_tx,
                        consensus_tx,
                    );
                }
            }
            other => {
                if let Some(mapped_action) = Self::map_key_action(other) {
                    self.apply_actions([mapped_action], async_tx, consensus_tx);
                }
            }
        }
    }

    /// Handles a terminal key event.
    ///
    /// When the command palette is open, all key input is passed to the palette until it closes
    /// Otherwise, the key is resolved through configured keybindings. Unbound keys are ignored.
    fn handle_key_event(
        &mut self,
        key: KeyEvent,
        async_tx: &mpsc::Sender<CoreAsyncEvent>,
        consensus_tx: &tokio::sync::watch::Sender<Option<ConsensusRequest>>,
    ) {
        trace!(?key, "received key event");
        self.ui
            .apply_action(UiAction::ClearCommandError, &self.core);

        // if a palette is open, all key events go to it until it's closed
        if let Some(palette) = self.ui.overlay.palette.as_mut() {
            trace!("forwarding key event to command palette");
            // the palette will only ever return an action, even if its an action to close itself,
            // so they are immediately applied
            let actions = palette.handle_key_event(key);
            self.apply_actions(actions, async_tx, consensus_tx);
            return;
        }
        if let Some(action) = keybindings::lookup(key.code, key.modifiers) {
            trace!(?action, "resolved keybinding action");
            self.match_action(action, async_tx, consensus_tx);
        } else {
            trace!("no keybinding action for key event");
        }
    }

    /// Try and load an alignment file.
    ///
    /// If no file path is configured, loading is marked as [`LoadingState::Idle`] so the UI can
    /// present an idle status. If a path exists, this issues [`CoreAction::LoadAlignment`]
    /// command, which performs the loading async.
    fn try_file_load(
        &mut self,
        async_tx: &mpsc::Sender<CoreAsyncEvent>,
        consensus_tx: &tokio::sync::watch::Sender<Option<ConsensusRequest>>,
    ) {
        let Some(file_path) = self.core.data.file_path.as_ref() else {
            info!("no startup file provided; entering idle loading state");
            self.core.loading_state = LoadingState::Idle;
            return;
        };
        let file_path = file_path.clone();
        debug!(path = ?file_path, "queueing startup alignment load");
        self.core.apply_action(
            CoreAction::LoadAlignment { path: file_path },
            async_tx,
            consensus_tx,
        );
    }

    /// Updates viewport after a terminal resize.
    ///
    /// A viewport update can trigger a consensus recalculation if the visible alignment pane width
    /// changes enough to alter the current windowing.
    fn handle_resize(
        &mut self,
        width: u16,
        height: u16,
        consensus_tx: &tokio::sync::watch::Sender<Option<ConsensusRequest>>,
    ) {
        self.terminal_size = Rect::new(0, 0, width, height);
        let content_height = height.saturating_sub(1);
        let layout = AppLayout::new(Rect::new(0, 0, width, content_height));

        let visible_width = layout.alignment_pane_area.width.saturating_sub(2) as usize;
        let visible_height = layout.alignment_pane_area.height.saturating_sub(4) as usize;
        let number_width = self.core.data.sequences.len().max(1).to_string().len();
        let number_prefix_width = number_width + 1;
        let name_visible_width = layout
            .sequence_id_pane_area
            .width
            .saturating_sub(2)
            .saturating_sub(number_prefix_width as u16) as usize;

        debug!(
            terminal_width = width,
            terminal_height = height,
            visible_width,
            visible_height,
            name_visible_width,
            "applied viewport dimensions after terminal resize"
        );

        self.core.update_viewport_dimensions(
            visible_width,
            visible_height,
            name_visible_width,
            consensus_tx,
        );
    }

    #[must_use]
    fn selection_point_crosshair(&self, mouse_x: u16, mouse_y: u16) -> Option<(usize, usize)> {
        let sequence_rows_area =
            AppLayout::new(self.terminal_area()).alignment_pane_sequence_rows_area();

        // stops panic in debug mode when clicking outside the alignment pane sequence rows area.
        if !sequence_rows_area.contains((mouse_x, mouse_y).into()) {
            return None;
        }

        let row_index = usize::from(mouse_y - sequence_rows_area.y);
        let col_index = usize::from(mouse_x - sequence_rows_area.x);
        let row_capacity = usize::from(sequence_rows_area.height);
        let sequence_id = visible_sequence_rows(&self.core, row_capacity)
            .get(row_index)
            .copied()
            .flatten()?;
        let absolute_col = self.core.viewport.window().col_range.start + col_index;
        // limits selection in short alignments where the pane can extend beyond sequence length.
        (absolute_col < self.core.data.sequence_length).then_some((sequence_id, absolute_col))
    }

    fn clear_box_selection_anchor(&mut self) {
        self.box_selection_anchor = None;
        self.middle_pan_anchor = None;
    }

    fn apply_mouse_selection(
        &mut self,
        start_sequence_id: usize,
        start_column: usize,
        end_sequence_id: usize,
        end_column: usize,
    ) {
        self.ui.apply_action(
            UiAction::SetMouseSelection(MouseSelection {
                sequence_id: start_sequence_id,
                column: start_column,
                end_sequence_id,
                end_column,
            }),
            &self.core,
        );
    }

    fn handle_mouse_event(
        &mut self,
        mouse: MouseEvent,
        async_tx: &mpsc::Sender<CoreAsyncEvent>,
        consensus_tx: &tokio::sync::watch::Sender<Option<ConsensusRequest>>,
    ) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some((sequence_id, column)) =
                    self.selection_point_crosshair(mouse.column, mouse.row)
                else {
                    // user can click outside the alignment pane to clear a selection.
                    self.clear_box_selection_anchor();
                    self.ui
                        .apply_action(UiAction::ClearMouseSelection, &self.core);
                    return;
                };

                // handle box mode modifier with CTRL
                self.box_selection_anchor = mouse
                    .modifiers
                    .contains(KeyModifiers::CONTROL)
                    .then_some((sequence_id, column));
                self.apply_mouse_selection(sequence_id, column, sequence_id, column);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some((sequence_id, column)) =
                    self.selection_point_crosshair(mouse.column, mouse.row)
                else {
                    return;
                };

                let (start_seq, start_col) =
                    self.box_selection_anchor.unwrap_or((sequence_id, column));
                self.apply_mouse_selection(start_seq, start_col, sequence_id, column);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let Some((sequence_id, column)) =
                    self.selection_point_crosshair(mouse.column, mouse.row)
                else {
                    self.clear_box_selection_anchor();
                    return;
                };

                let (start_seq, start_col) =
                    self.box_selection_anchor.unwrap_or((sequence_id, column));
                self.apply_mouse_selection(start_seq, start_col, sequence_id, column);
                self.clear_box_selection_anchor();
            }
            MouseEventKind::Down(MouseButton::Middle) => {
                self.middle_pan_anchor = Some((mouse.column, mouse.row));
            }
            MouseEventKind::Drag(MouseButton::Middle) => {
                let Some((anchor_x, anchor_y)) = self.middle_pan_anchor else {
                    return;
                };

                let (dy_amount, scroll_up) = if mouse.row >= anchor_y {
                    (usize::from(mouse.row - anchor_y), true)
                } else {
                    (usize::from(anchor_y - mouse.row), false)
                };

                let (dx_amount, scroll_left) = if mouse.column >= anchor_x {
                    (usize::from(mouse.column - anchor_x), true)
                } else {
                    (usize::from(anchor_x - mouse.column), false)
                };

                let actions = [
                    (dy_amount > 0).then(|| {
                        if scroll_up {
                            Action::Core(CoreAction::ScrollUp { amount: dy_amount })
                        } else {
                            Action::Core(CoreAction::ScrollDown { amount: dy_amount })
                        }
                    }),
                    (dx_amount > 0).then(|| {
                        if scroll_left {
                            Action::Core(CoreAction::ScrollLeft { amount: dx_amount })
                        } else {
                            Action::Core(CoreAction::ScrollRight { amount: dx_amount })
                        }
                    }),
                ];
                self.apply_actions(actions.into_iter().flatten(), async_tx, consensus_tx);

                self.middle_pan_anchor = Some((mouse.column, mouse.row));
            }
            MouseEventKind::Up(MouseButton::Middle) => {
                self.middle_pan_anchor = None;
            }
            _ => {}
        }
    }

    /// Entrypoint for main app loop
    ///
    /// The loop handles three event sources:
    /// - frame ticks at `RENDER_TARGET_FPS` for rendering,
    /// - terminal input (including resize)
    /// - async events produced by background jobs.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        info!(target_fps = RENDER_FPS, "starting app runtime");
        let (async_tx, mut async_rx) = mpsc::channel(128);
        let (consensus_tx, consensus_rx) = tokio::sync::watch::channel(None::<ConsensusRequest>);
        let consensus_worker = spawn_consensus_worker(consensus_rx, async_tx.clone());
        debug!("spawned consensus worker");

        // will try and load input fasta file immediately
        // if none, core is set to idle and a status message is displayed
        // otherwise starts an async load job
        self.try_file_load(&async_tx, &consensus_tx);

        match terminal.size() {
            Ok(area) => {
                debug!(
                    width = area.width,
                    height = area.height,
                    "captured initial terminal size"
                );
                self.handle_resize(area.width, area.height, &consensus_tx);
            }
            Err(error_value) => {
                warn!(error = ?error_value, "failed to capture initial terminal size");
            }
        }

        let period = Duration::from_secs_f32(1.0 / RENDER_FPS);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();

        // main event loop
        // TODO: maybe let ctrl+c break the loop
        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(error_value) = terminal.draw(|frame| { render(frame, &self.core, &self.ui) }) {
                        error!(error = ?error_value, "terminal draw failed");
                        return Err(error_value.into());
                    }
                },
                Some(Ok(event)) = events.next() => {
                    match event {
                        TermEvent::Resize(width, height) => {
                            self.handle_resize(width, height, &consensus_tx);
                        }
                        TermEvent::Key(key) => {
                            self.handle_key_event(key, &async_tx, &consensus_tx);
                        }
                        TermEvent::Mouse(mouse) => {
                            if self.ui.overlay.palette.is_none() {
                                self.handle_mouse_event(mouse, &async_tx, &consensus_tx);
                            }
                        }
                        _ => {}
                    }
                }
                Some(event) = async_rx.recv() => {
                    match &event {
                        CoreAsyncEvent::AlignmentsLoaded(result) => {
                            match result {
                                Ok(alignments) => {
                                    trace!(
                                        sequence_count = alignments.len(),
                                        "received alignments loaded event"
                                    );
                                }
                                Err(error_value) => {
                                    trace!(
                                        error = %error_value,
                                        "received alignment load failure event"
                                    );
                                }
                            }
                        }
                        CoreAsyncEvent::ConsensusUpdated { updates } => {
                            trace!(
                                updated_positions = updates.len(),
                                "received consensus update event"
                            );
                        }
                    }
                    self.core.handle_event(event, &consensus_tx);
                }
            }
        }

        info!("quit requested, stopping consensus worker");
        drop(consensus_tx);
        if let Err(join_error) = consensus_worker.await {
            warn!(
                error = ?join_error,
                "consensus worker failed to shut down cleanly"
            );
        }
        Ok(())
    }
}
