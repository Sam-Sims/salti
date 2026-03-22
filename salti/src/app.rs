use std::{env, time::Duration};

use anyhow::{Result, format_err};
use crossterm::event::{Event as TermEvent, EventStream, KeyEvent, MouseEvent};
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;
use tokio::{
    sync::mpsc::{UnboundedSender, unbounded_channel},
    task::{JoinError, JoinHandle, JoinSet},
};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::cli::StartupState;
use crate::command::Command;
use crate::core::model::{AlignmentModel, StatsView};
use crate::core::parser;
use crate::core::stats_cache::{ColumnStatsCache, StatsJobRequest, StatsJobResult};
use crate::input;
use crate::input::MouseTracker;
use crate::overlay::command_palette::CommandPaletteState;
use crate::ui::layout::{AppLayout, FrameLayout, pinned_section_layout};
use crate::ui::notification::{Notification, NotificationLevel};
use crate::ui::render::render;
use crate::ui::ui_state::{LoadingState, UiState};
use crate::update::UpdateResult;

const RENDER_FPS: f32 = 120.0;

const INSTALLED_VERSION: &str = env!("CARGO_PKG_VERSION");
const UPDATE_CHECK_ENV_VAR: &str = "SALTI_SKIP_UPDATE_CHECK";

#[derive(Debug)]
enum AppEvent {
    UpdateAvailable { latest: String },
    UpToDate,
}

#[derive(Debug)]
struct AsyncJob<T> {
    handle: JoinHandle<T>,
    cancel: CancellationToken,
}

#[derive(Debug)]
pub(crate) struct App {
    alignment: Option<AlignmentModel>,
    ui: UiState,
    mouse_tracker: MouseTracker,
    stats_cache: ColumnStatsCache,
    raw_stats_jobs: JoinSet<StatsJobResult>,
    translated_stats_jobs: JoinSet<StatsJobResult>,
    load_job: Option<AsyncJob<Result<Vec<libmsa::RawSequence>, String>>>,
    event_tx: Option<UnboundedSender<AppEvent>>,
    should_quit: bool,
    layout_area: Rect,
    frame_layout: FrameLayout,
    app_layout: AppLayout,
}

impl App {
    pub(crate) fn new(startup: StartupState) -> Self {
        let layout_area = Rect::default();
        let frame_layout = FrameLayout::new(layout_area);
        let app_layout = AppLayout::new(frame_layout.content_area);
        Self {
            alignment: None,
            ui: UiState::new(startup),
            mouse_tracker: MouseTracker::default(),
            stats_cache: ColumnStatsCache::default(),
            raw_stats_jobs: JoinSet::new(),
            translated_stats_jobs: JoinSet::new(),
            load_job: None,
            event_tx: None,
            should_quit: false,
            layout_area,
            frame_layout,
            app_layout,
        }
    }

    pub(crate) async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        info!(target_fps = RENDER_FPS, "Starting runtime");

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
            Err(error) => {
                warn!(error = ?error, "Failed to capture initial terminal size");
            }
        }

        self.extend_stats_if_needed();

        let period = Duration::from_secs_f32(1.0 / RENDER_FPS);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();
        let (event_tx, mut event_rx) = unbounded_channel::<AppEvent>();
        self.event_tx = Some(event_tx);
        let mut needs_redraw = true;
        if Self::startup_update_check_enabled() {
            self.execute_commands([Command::CheckForUpdate {
                show_success_message: false,
            }]);
        } else {
            debug!(
                env_var = UPDATE_CHECK_ENV_VAR,
                "Startup update check disabled via environment variable"
            );
        }

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => {
                    if needs_redraw {
                        if let Err(error) = terminal.draw(|frame| {
                            self.update_layout(frame.area());
                            render(
                                frame,
                                self.alignment.as_ref(),
                                &self.ui,
                                &self.stats_cache,
                                &self.frame_layout,
                                &self.app_layout,
                            )
                        }) {
                            error!(error = ?error, "terminal draw failed");
                            return Err(error.into());
                        }
                        needs_redraw = false;
                    }
                }
                Some(Ok(event)) = events.next() => {
                    match event {
                        TermEvent::Resize(width, height) => {
                            self.update_layout(Rect::new(0, 0, width, height));
                            self.extend_stats_if_needed();
                        }
                        TermEvent::Key(key) => {
                            self.handle_key_event(key);
                        }
                        TermEvent::Mouse(mouse) => {
                            self.handle_mouse_event(mouse);
                        }
                        _ => (),
                    }

                    needs_redraw = true;
                }
                Some(event) = event_rx.recv() => {
                    self.handle_app_event(event);
                    needs_redraw = true;
                }
                Some(join_result) = self.raw_stats_jobs.join_next() => {
                    self.handle_stats_result(join_result);
                    needs_redraw = true;
                }
                Some(join_result) = self.translated_stats_jobs.join_next() => {
                    self.handle_stats_result(join_result);
                    needs_redraw = true;
                }
                Some(join_result) = async {
                    match self.load_job.as_mut() {
                        Some(job) => Some((&mut job.handle).await),
                        None => None,
                    }
                } => {
                    self.load_job = None;
                    match join_result {
                        Ok(Ok(raw_sequences)) => match libmsa::Alignment::new(raw_sequences)
                            .and_then(AlignmentModel::new) {
                            Ok(model) => {
                                self.raw_stats_jobs.abort_all();
                                self.translated_stats_jobs.abort_all();
                                self.stats_cache.init(model.view().column_count());
                                self.alignment = Some(model);
                                self.ui.meta.loading_state = LoadingState::Loaded;
                                self.ui.clear_transient_state();
                                self.mouse_tracker.clear_anchors();
                                self.refresh_viewport_bounds();
                                self.ui.viewport.jump_to_position(self.ui.meta.initial_position);
                                self.try_spawn_stats_jobs();
                            }
                            Err(error) => {
                                self.ui.meta.loading_state = LoadingState::Failed(error.to_string());
                            }
                        },
                        Ok(Err(error)) => {
                            self.ui.meta.loading_state = LoadingState::Failed(error);
                        }
                        Err(join_error) => {
                            if !join_error.is_cancelled() {
                                error!(error = ?join_error, "Alignment load task panicked");
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
        self.raw_stats_jobs.abort_all();
        self.translated_stats_jobs.abort_all();
        Ok(())
    }

    fn startup_update_check_enabled() -> bool {
        !matches!(env::var(UPDATE_CHECK_ENV_VAR), Ok(value) if value.eq_ignore_ascii_case("true"))
    }

    fn try_file_load(&mut self) {
        let Some(input) = self.ui.meta.input_path.clone() else {
            info!("No startup file provided; entering idle loading state");
            self.ui.meta.loading_state = LoadingState::Idle;
            return;
        };

        debug!(input = %input, "Loading startup alignment");
        self.start_load_job(input);
    }

    fn update_layout(&mut self, area: Rect) {
        if area == self.layout_area {
            return;
        }

        self.layout_area = area;
        self.frame_layout = FrameLayout::new(area);
        self.app_layout = AppLayout::new(self.frame_layout.content_area);

        let visible_width = self.app_layout.alignment_pane.width.saturating_sub(2) as usize;
        let available_sequence_rows = self.app_layout.alignment_pane_sequence_rows.height as usize;
        let alignment = self.alignment.as_ref();
        let pinned_count = alignment
            .map(|alignment| alignment.rows().pinned().len())
            .unwrap_or(0);
        let scrollable_height =
            pinned_section_layout(pinned_count, available_sequence_rows).scrollable_height;
        let row_count = alignment
            .map(|alignment| alignment.base().row_count())
            .unwrap_or(0)
            .max(1);
        let number_width = row_count
            .checked_ilog10()
            .map_or(1, |digits| digits as usize + 1);
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
            available_sequence_rows,
            scrollable_height,
            name_visible_width,
            "Terminal resized, viewport updated"
        );

        self.ui
            .viewport
            .update_dimensions(visible_width, scrollable_height, name_visible_width);
        self.refresh_viewport_bounds();
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        self.ui.notification = None;
        let commands = input::handle_key_event(&mut self.ui, key);
        self.execute_commands(commands);
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        let commands = input::handle_mouse_event(
            &mut self.mouse_tracker,
            self.alignment.as_ref(),
            &mut self.ui,
            &self.frame_layout,
            &self.app_layout,
            mouse,
        );
        self.execute_commands(commands);
    }

    fn handle_app_event(&mut self, event: AppEvent) {
        let notification = match event {
            AppEvent::UpdateAvailable { latest } => Notification {
                level: NotificationLevel::Info,
                message: format!(
                    "A new version of salti is available: {latest} (installed: {INSTALLED_VERSION})"
                ),
            },
            AppEvent::UpToDate => Notification {
                level: NotificationLevel::Info,
                message: "salti is up to date".to_string(),
            },
        };
        self.execute_commands([Command::ShowNotification(notification)]);
    }

    fn execute_commands<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = Command>,
    {
        for command in commands {
            if let Err(error) = self.execute_command(command) {
                warn!(error = ?error, "Command failed");
                self.ui.notification = Some(Notification {
                    level: NotificationLevel::Error,
                    message: error.to_string(),
                });
            }
        }
    }

    fn execute_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Quit => {
                self.should_quit = true;
            }
            Command::OpenCommandPalette => {
                self.open_command_palette();
            }
            Command::CloseOverlay => {
                self.ui.overlay.close();
            }
            Command::ToggleMinimap => {
                self.ui.overlay.toggle_minimap();
            }
            Command::SetTheme(theme_id) => {
                self.ui.set_theme(theme_id);
            }
            Command::ShowNotification(notification) => {
                self.ui.notification = Some(notification);
            }
            Command::LoadFile { input } => {
                self.clear_mouse_selection();
                self.start_load_job(input);
            }
            Command::CheckForUpdate {
                show_success_message,
            } => {
                self.spawn_update_check(show_success_message);
            }

            Command::ScrollDown { amount } => self.ui.viewport.scroll_down(amount),
            Command::ScrollUp { amount } => self.ui.viewport.scroll_up(amount),
            Command::ScrollLeft { amount } => self.ui.viewport.scroll_left(amount),
            Command::ScrollRight { amount } => self.ui.viewport.scroll_right(amount),
            Command::ScrollNamesLeft { amount } => self.ui.viewport.scroll_names_left(amount),
            Command::ScrollNamesRight { amount } => self.ui.viewport.scroll_names_right(amount),

            Command::JumpToPosition(relative_col) => {
                let has_column = self
                    .alignment
                    .as_ref()
                    .is_some_and(|alignment| relative_col < alignment.view().column_count());
                if has_column {
                    self.ui.viewport.jump_to_position(relative_col);
                }
            }
            Command::JumpToSequence(abs_row) => {
                let Some(alignment) = self.alignment.as_ref() else {
                    return Ok(());
                };
                if let Some(relative_row) = alignment.view().relative_row_id(abs_row) {
                    self.ui.viewport.jump_to_sequence(relative_row);
                }
                if let Some(message) = alignment.jump_to_sequence(abs_row) {
                    self.show_info(message);
                }
            }
            Command::JumpToStart => {
                let has_columns = self
                    .alignment
                    .as_ref()
                    .is_some_and(|alignment| alignment.view().column_count() > 0);
                if has_columns {
                    self.ui.viewport.jump_to_position(0);
                }
            }
            Command::JumpToEnd => {
                let last_col = self
                    .alignment
                    .as_ref()
                    .and_then(|alignment| alignment.view().column_count().checked_sub(1));
                if let Some(last_col) = last_col {
                    self.ui.viewport.jump_to_position(last_col);
                }
            }

            Command::PinSequence(abs_row) => {
                self.alignment_mut()?.pin(abs_row)?;
                self.clear_mouse_selection();
                self.on_view_rebuilt();
                return Ok(());
            }
            Command::UnpinSequence(abs_row) => {
                self.alignment_mut()?.unpin(abs_row)?;
                self.clear_mouse_selection();
                self.on_view_rebuilt();
                return Ok(());
            }
            Command::SetReference(abs_row) => {
                self.alignment_mut()?.set_reference(abs_row)?;
                self.clear_mouse_selection();
                self.on_view_rebuilt();
                return Ok(());
            }
            Command::ClearReference => {
                self.alignment_mut()?.clear_reference()?;
                self.clear_mouse_selection();
                self.on_view_rebuilt();
                return Ok(());
            }

            Command::SetFilter(pattern) => {
                self.alignment_mut()?.set_filter(pattern)?;
                self.on_view_rebuilt();
                return Ok(());
            }
            Command::SetGapFilter(max_gap_fraction) => {
                let alignment = self.alignment_mut()?;
                if max_gap_fraction.is_some() && alignment.translation().is_some() {
                    return Err(format_err!(
                        "filter-gaps is unavailable while translation is active"
                    ));
                }
                alignment.set_gap_filter(max_gap_fraction)?;
                self.on_view_rebuilt();
                return Ok(());
            }
            Command::ClearFilter => {
                self.alignment_mut()?.clear_filter()?;
                self.on_view_rebuilt();
                return Ok(());
            }
            Command::SetActiveType(kind) => {
                self.alignment_mut()?.set_active_kind(kind)?;
                self.on_view_rebuilt();
                return Ok(());
            }

            Command::ToggleTranslationView => {
                let alignment = self.alignment_mut()?;
                if alignment.translation().is_none() && alignment.filter().has_column_filter() {
                    return Err(format_err!(
                        "translation is unavailable while filter-gaps is active"
                    ));
                }
                alignment.toggle_translation_view()?;
                self.invalidate_all_stats();
                return Ok(());
            }
            Command::SetTranslationFrame(frame) => {
                let alignment = self.alignment_mut()?;
                let was_enabled = alignment.translation().is_some();
                alignment.set_translation_frame(frame)?;
                if was_enabled {
                    self.invalidate_translated_stats();
                }
                return Ok(());
            }

            Command::SetConsensusMethod(method) => {
                self.alignment_mut()?.consensus_method = method;
                self.invalidate_all_stats();
                return Ok(());
            }
            Command::SetDiffMode(mode) => {
                self.alignment_mut()?.diff_mode = mode;
            }
        }

        self.extend_stats_if_needed();
        Ok(())
    }

    fn open_command_palette(&mut self) {
        let palette = self
            .alignment
            .as_ref()
            .map(CommandPaletteState::from_alignment)
            .unwrap_or_else(CommandPaletteState::empty);
        self.ui.overlay.open_palette(palette);
    }

    fn on_view_rebuilt(&mut self) {
        self.refresh_viewport_bounds();
        self.invalidate_all_stats();
    }

    fn clear_mouse_selection(&mut self) {
        self.ui.selection = None;
        self.mouse_tracker.clear_anchors();
    }

    fn show_info(&mut self, message: String) {
        self.ui.notification = Some(Notification {
            level: NotificationLevel::Info,
            message,
        });
    }

    fn refresh_viewport_bounds(&mut self) {
        let Some(alignment) = self.alignment.as_ref() else {
            return;
        };
        self.ui.viewport.set_bounds(
            alignment.view().row_count(),
            alignment.view().column_count(),
            alignment.base().max_id_len(),
        );
    }

    fn alignment_mut(&mut self) -> Result<&mut AlignmentModel> {
        self.alignment
            .as_mut()
            .ok_or_else(|| format_err!("no alignment is loaded"))
    }

    fn start_load_job(&mut self, input: String) {
        if let Some(previous) = self.load_job.take() {
            debug!("Previous load job found, cancelling");
            previous.cancel.cancel();
            previous.handle.abort();
        }

        self.ui.meta.input_path = Some(input.clone());
        self.ui.meta.loading_state = LoadingState::Loading;

        let cancel = CancellationToken::new();
        debug!(input = %input, "Spawning new load job for input");
        let handle = tokio::task::spawn_blocking({
            let cancel = cancel.clone();
            move || parser::parse_fasta_file(&input, &cancel).map_err(|error| error.to_string())
        });

        self.load_job = Some(AsyncJob { handle, cancel });
    }

    fn spawn_update_check(&self, show_up_to_date: bool) {
        let Some(event_tx) = self.event_tx.clone() else {
            return;
        };

        tokio::spawn(async move {
            let Some(result) = crate::update::check_for_update().await else {
                return;
            };
            match result {
                UpdateResult::UpdateAvailable(latest) => {
                    let _ = event_tx.send(AppEvent::UpdateAvailable { latest });
                }
                UpdateResult::UpToDate => {
                    if show_up_to_date {
                        let _ = event_tx.send(AppEvent::UpToDate);
                    }
                }
            }
        });
    }

    fn handle_stats_result(&mut self, join_result: std::result::Result<StatsJobResult, JoinError>) {
        let Ok(result) = join_result else {
            return;
        };
        let error_message = result.summaries.as_ref().err().cloned();
        if !self.stats_cache.store(result)
            && let Some(error_message) = error_message
        {
            warn!(error = %error_message, "Stats chunk failed");
        }
    }

    fn try_spawn_stats_jobs(&mut self) {
        let Some(alignment) = self.alignment.as_ref() else {
            return;
        };
        let col_range = self.ui.viewport.window().col_range;
        let generation = self.stats_cache.generation();

        for chunk_idx in self.stats_cache.raw_chunks_to_spawn(&col_range) {
            self.stats_cache.mark_raw_pending(chunk_idx);
            let request = StatsJobRequest {
                alignment: alignment.view().clone(),
                view: StatsView::Raw,
                chunk_idx,
                range: self.stats_cache.raw_chunk_range(chunk_idx),
                method: alignment.consensus_method,
                generation,
            };
            self.raw_stats_jobs.spawn_blocking(move || {
                let StatsJobRequest {
                    alignment,
                    view,
                    chunk_idx,
                    range,
                    method,
                    generation,
                } = request;
                let summaries = alignment
                    .column_summaries_range(range.clone(), method)
                    .map_err(|error| error.to_string());
                StatsJobResult {
                    generation,
                    chunk_idx,
                    view,
                    summaries,
                }
            });
        }

        let Some(ctx) = alignment.stats_context(col_range) else {
            return;
        };
        if let StatsView::Translated(frame) = ctx.view {
            for chunk_idx in
                self.stats_cache
                    .translated_chunks_to_spawn(&ctx.range, frame, ctx.total_columns)
            {
                self.stats_cache.mark_translated_pending(chunk_idx);
                let request = StatsJobRequest {
                    alignment: alignment.view().clone(),
                    view: StatsView::Translated(frame),
                    chunk_idx,
                    range: self.stats_cache.translated_chunk_range(chunk_idx),
                    method: alignment.consensus_method,
                    generation,
                };
                self.translated_stats_jobs.spawn_blocking(move || {
                    let StatsJobRequest {
                        alignment,
                        view,
                        chunk_idx,
                        range,
                        method,
                        generation,
                    } = request;
                    let summaries = alignment
                        .translated(frame)
                        .and_then(|translated| {
                            translated.column_summaries_range(range.clone(), method)
                        })
                        .map_err(|error| error.to_string());
                    StatsJobResult {
                        generation,
                        chunk_idx,
                        view,
                        summaries,
                    }
                });
            }
        }
    }

    fn extend_stats_if_needed(&mut self) {
        self.try_spawn_stats_jobs();
    }

    fn invalidate_all_stats(&mut self) {
        let Some(alignment) = self.alignment.as_ref() else {
            return;
        };
        self.raw_stats_jobs.abort_all();
        self.translated_stats_jobs.abort_all();
        self.stats_cache
            .invalidate_all(alignment.view().column_count());
        self.try_spawn_stats_jobs();
    }

    fn invalidate_translated_stats(&mut self) {
        self.translated_stats_jobs.abort_all();
        self.stats_cache.invalidate_translated();
        self.try_spawn_stats_jobs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};

    use crate::ui::ui_state::MouseSelection;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn app_with_alignment(sequences: Vec<libmsa::RawSequence>) -> App {
        let startup = StartupState {
            file_path: None,
            initial_position: 0,
        };
        let mut app = App::new(startup);
        let alignment = libmsa::Alignment::new(sequences).expect("alignment should load");
        let model = AlignmentModel::new(alignment).expect("alignment model should build");
        app.stats_cache.init(model.view().column_count());
        app.alignment = Some(model);
        app.ui.meta.loading_state = LoadingState::Loaded;
        app.refresh_viewport_bounds();
        app.update_layout(Rect::new(0, 0, 40, 12));
        app
    }

    fn left_mouse_event(
        kind: MouseEventKind,
        area: Rect,
        column_offset: u16,
        row_offset: u16,
    ) -> MouseEvent {
        MouseEvent {
            kind,
            column: area.x + column_offset,
            row: area.y + row_offset,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[test]
    fn translated_click_selects_a_full_codon_span() {
        let mut app =
            app_with_alignment(vec![raw("row1", b"ATGAAATTT"), raw("row2", b"ATGAAATTT")]);
        app.alignment
            .as_mut()
            .unwrap()
            .set_translation_frame(libmsa::ReadingFrame::Frame1)
            .expect("setting translation frame should succeed");
        app.alignment
            .as_mut()
            .unwrap()
            .toggle_translation_view()
            .expect("translation should enable");

        let area = app.app_layout.alignment_pane_sequence_rows;
        app.handle_mouse_event(left_mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            area,
            1,
            0,
        ));

        let selection = app.ui.selection.expect("selection should be created");
        assert_eq!(selection.sequence_id, 0);
        assert_eq!(selection.column, 0);
        assert_eq!(selection.end_column, 2);
    }

    #[test]
    fn translated_drag_extends_selection_in_whole_codons() {
        let mut app =
            app_with_alignment(vec![raw("row1", b"ATGAAATTT"), raw("row2", b"ATGAAATTT")]);
        app.alignment
            .as_mut()
            .unwrap()
            .set_translation_frame(libmsa::ReadingFrame::Frame1)
            .expect("setting translation frame should succeed");
        app.alignment
            .as_mut()
            .unwrap()
            .toggle_translation_view()
            .expect("translation should enable");

        let area = app.app_layout.alignment_pane_sequence_rows;
        app.handle_mouse_event(left_mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            area,
            1,
            0,
        ));
        app.handle_mouse_event(left_mouse_event(
            MouseEventKind::Drag(MouseButton::Left),
            area,
            7,
            0,
        ));
        app.handle_mouse_event(left_mouse_event(
            MouseEventKind::Up(MouseButton::Left),
            area,
            7,
            0,
        ));

        let selection = app.ui.selection.expect("selection should be created");
        assert_eq!(selection.sequence_id, 0);
        assert_eq!(selection.column, 0);
        assert_eq!(selection.end_sequence_id, 0);
        assert_eq!(selection.end_column, 8);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn toggling_translation_preserves_the_stored_nucleotide_selection() {
        let mut app =
            app_with_alignment(vec![raw("row1", b"ATGAAATTT"), raw("row2", b"ATGAAATTT")]);
        let selection = MouseSelection {
            sequence_id: 0,
            column: 4,
            end_sequence_id: 0,
            end_column: 4,
        };
        app.ui.selection = Some(selection);

        app.execute_commands([Command::ToggleTranslationView]);
        assert_eq!(app.ui.selection, Some(selection));

        app.execute_commands([Command::ToggleTranslationView]);
        assert_eq!(app.ui.selection, Some(selection));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn filter_gaps_shows_notification_when_translation_is_active() {
        let mut app =
            app_with_alignment(vec![raw("row1", b"ATGAAATTT"), raw("row2", b"ATGAAATTT")]);
        app.execute_commands([Command::ToggleTranslationView]);
        app.execute_commands([Command::SetGapFilter(Some(0.25))]);

        let notification = app
            .ui
            .notification
            .as_ref()
            .expect("notification should be created");
        assert_eq!(
            notification.message,
            "filter-gaps is unavailable while translation is active"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn translation_shows_notification_when_gap_filter_is_active() {
        let mut app = app_with_alignment(vec![raw("row1", b"ATG---"), raw("row2", b"ATG---")]);
        app.execute_commands([Command::SetGapFilter(Some(0.0))]);
        app.execute_commands([Command::ToggleTranslationView]);

        let notification = app
            .ui
            .notification
            .as_ref()
            .expect("notification should be created");
        assert_eq!(
            notification.message,
            "translation is unavailable while filter-gaps is active"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn key_events_are_forwarded_to_command_execution() {
        let mut app = app_with_alignment(vec![raw("row1", b"ACGT"), raw("row2", b"ACGT")]);
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

        app.handle_key_event(key);

        assert!(app.should_quit);
    }
}
