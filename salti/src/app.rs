use std::{env, path::Path, time::Duration};

use anyhow::{Result, bail, format_err};
use crossterm::event::{Event as TermEvent, EventStream, KeyEvent, MouseEvent};
use ratatui::{DefaultTerminal, layout::Rect};
use tokio::{
    sync::mpsc::{UnboundedSender, unbounded_channel},
    task::JoinHandle,
};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::{
    cli::StartupState,
    command::Command,
    core::{
        gff::{self, Gff},
        parser,
        session::{Position, Session, ViewMode, ViewState},
    },
    input::{self, MouseTracker},
    ui::{
        layers::{
            notification::{Notification, NotificationLevel},
            palette::CommandPaletteState,
        },
        layout::{
            AlignmentHeaderLayout, AppLayout, FrameLayout, Window, fit, gff_pane_height,
            pinned_section_layout,
        },
        panes::{gff::feature_row_count, local_feature_track::local_feature_row_count},
        render::render,
        ui_state::{LoadingState, UiState},
    },
    update::UpdateResult,
};

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
    session: Option<Session>,
    gff: Option<Gff>,
    ui: UiState,
    mouse_tracker: MouseTracker,
    load_job: Option<AsyncJob<Result<Vec<libmsa::Sequence>, String>>>,
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
        let app_layout = AppLayout::new(
            frame_layout.content_area,
            // TODO: remove magic number similar to AlignmentHeaderLayout - currently 0 at start since no GFF loaded
            0,
            AlignmentHeaderLayout::without_features(),
        );
        Self {
            session: None,
            gff: None,
            ui: UiState::new(startup),
            mouse_tracker: MouseTracker::default(),
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
                self.rebuild_layout(area.into());
            }
            Err(error) => {
                warn!(error = ?error, "Failed to capture initial terminal size");
            }
        }

        let period = Duration::from_secs_f32(1.0 / RENDER_FPS);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();
        let (event_tx, mut event_rx) = unbounded_channel::<AppEvent>();
        self.event_tx = Some(event_tx);
        let mut needs_redraw = true;
        if Self::startup_update_check_enabled() {
            self.execute_commands([Command::CheckForUpdate]);
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
                            let area = frame.area();
                            if area != self.layout_area {
                                self.rebuild_layout(area);
                            }
                            let columns = self.session.as_ref().map(|session| {
                                session.window_columns(self.ui.window.columns.clone())
                            });
                            render(
                                frame,
                                self.session.as_ref(),
                                self.gff.as_ref(),
                                &self.ui,
                                columns.as_ref(),
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
                            self.rebuild_layout(Rect::new(0, 0, width, height));
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
                Some(join_result) = async {
                    match self.load_job.as_mut() {
                        Some(job) => Some((&mut job.handle).await),
                        None => None,
                    }
                } => {
                    self.load_job = None;
                    match join_result {
                        Ok(Ok(raw_sequences)) => match libmsa::Alignment::new(raw_sequences)
                            .map(Session::new) {
                            Ok(session) => {
                                self.session = Some(session);
                                self.ui.meta.loading_state = LoadingState::Loaded;
                                self.ui.clear_transient_state();
                                self.mouse_tracker.clear_anchors();
                                self.ui.position = Position {
                                    column: self.ui.meta.initial_position,
                                    ..Position::default()
                                };
                                self.rebuild_layout(self.layout_area);
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

    fn rebuild_layout(&mut self, area: Rect) {
        self.layout_area = area;
        self.frame_layout = FrameLayout::new(area);
        let session = self.session.as_ref();

        // TODO: revisit - probably a better way of doing this without need to probe first

        let probe = AppLayout::new(
            self.frame_layout.content_area,
            gff_pane_height(1),
            AlignmentHeaderLayout::without_features(),
        );

        let visible_width = usize::from(probe.alignment_pane.width.saturating_sub(2));
        let total_columns = session.map_or(0, |session| session.layout().columns().len());

        let columns = fit(&mut self.ui.position.column, visible_width, total_columns);
        let (gff_height, local_feature_rows) = match (self.gff.as_ref(), session) {
            (Some(gff), Some(session)) => {
                let gff_width = usize::from(probe.gff_pane_rows.width);
                let gff_rows = feature_row_count(gff, session, gff_width).max(1);
                let local_rows = local_feature_row_count(gff, session, &columns);
                (
                    gff_pane_height(gff_rows),
                    u16::try_from(local_rows).unwrap_or(u16::MAX),
                )
            }
            (None, _) | (_, None) => (0, 0),
        };

        self.app_layout = AppLayout::new(
            self.frame_layout.content_area,
            gff_height,
            AlignmentHeaderLayout::with_features(local_feature_rows),
        );

        let available_sequence_rows =
            usize::from(self.app_layout.alignment_pane_sequence_rows.height);
        let pinned = session.map_or(0, |session| session.layout().pinned());
        let scrollable_height =
            pinned_section_layout(pinned, available_sequence_rows).scrollable_height;
        let main_rows = session.map_or(0, |session| session.layout().main().len());
        let rows = fit(&mut self.ui.position.row, scrollable_height, main_rows);

        let row_count = session.map_or(0, |session| session.base_alignment().row_count());
        let number_prefix_width = row_count.max(1).ilog10() as usize + 2;
        let name_visible_width =
            usize::from(self.app_layout.sequence_id_pane.width.saturating_sub(2))
                .saturating_sub(number_prefix_width);
        let max_id_len = session.map_or(0, |session| session.base_alignment().max_id_len());
        let names = fit(&mut self.ui.position.name, name_visible_width, max_id_len);

        debug!(
            terminal_width = area.width,
            terminal_height = area.height,
            visible_width,
            available_sequence_rows,
            scrollable_height,
            name_visible_width,
            "Screen layout rebuilt"
        );

        self.ui.window = Window {
            rows,
            columns,
            names,
        };
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        self.ui.notification = None;
        let commands = input::handle_key_event(&mut self.ui, key);
        self.execute_commands(commands);
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        let commands = input::handle_mouse_event(
            &mut self.mouse_tracker,
            self.session.as_ref(),
            self.gff.as_ref(),
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
        self.rebuild_layout(self.layout_area);
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
                self.ui.layers.close_active();
            }
            Command::ToggleMinimap => {
                self.ui.layers.toggle_minimap();
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
            Command::LoadGff { path } => {
                self.gff = Some(gff::parse_gff(Path::new(&path))?);
                self.ui.gff_pane = Default::default();
                self.show_info(format!("Loaded GFF file: {path}"));
            }
            Command::CheckForUpdate => {
                self.spawn_update_check(false);
            }
            Command::CheckForUpdateAndNotify => {
                self.spawn_update_check(true);
            }

            Command::ScrollDown { amount } => self.ui.position.row += amount,
            Command::ScrollUp { amount } => {
                self.ui.position.row = self.ui.position.row.saturating_sub(amount);
            }
            Command::ScrollLeft { amount } => {
                self.ui.position.column = self.ui.position.column.saturating_sub(amount);
            }
            Command::ScrollRight { amount } => self.ui.position.column += amount,
            Command::ScrollNamesLeft { amount } => {
                self.ui.position.name = self.ui.position.name.saturating_sub(amount);
            }
            Command::ScrollNamesRight { amount } => self.ui.position.name += amount,
            Command::JumpToIndex(index) => self.ui.position.column = index,
            Command::JumpToColumn(column) => self.jump_to_column(column)?,
            Command::JumpToFeature(index) => {
                let gff = self
                    .gff
                    .as_ref()
                    .ok_or_else(|| format_err!("no GFF file is loaded"))?;
                let column = self
                    .session()?
                    .column_range_at(gff.features[index].range.clone())
                    .start;
                self.jump_to_column(column)?;
            }
            Command::JumpToSequence(row) => {
                self.ui.position.row = self.session()?.layout().row_position(row);
            }
            Command::JumpToEnd => {
                self.ui.position.column = self.session()?.layout().columns().len() - 1;
            }
            Command::PinSequence(row) => self.apply(|s| s.pinned.push(row))?,
            Command::UnpinSequence(row) => self.apply(|s| s.pinned.retain(|&r| r != row))?,
            Command::SetReference(row) => self.apply(|s| {
                s.pinned.retain(|&r| Some(r) != row);
                s.reference = row;
            })?,
            Command::SetRowFilter(pattern) => {
                let regex = pattern.as_deref().map(regex::Regex::new).transpose()?;
                self.apply(|s| s.row_regex_filter = regex)?;
            }
            Command::SetGapFilter(fraction) => {
                self.apply(|s| s.filter.max_gap_fraction = fraction)?;
            }
            Command::SetConstantFilter(fraction) => {
                self.apply(|s| s.filter.min_const_fraction = fraction)?;
            }
            Command::ClearAllFilters => self.apply(|s| {
                s.row_regex_filter = None;
                s.filter = libmsa::ColumnFilter::default();
            })?,
            Command::ToggleTranslationView => {
                let session = self.session()?;
                if !session.grid().alignment_type().supports_translation() {
                    bail!("translation needs a DNA alignment");
                }

                let next = match session.state().mode {
                    ViewMode::QuickTranslate => ViewMode::Default,
                    ViewMode::Default | ViewMode::FullTranslate => ViewMode::QuickTranslate,
                };

                let selection = self.ui.selection.take();
                let result = self.apply(|s| s.mode = next);
                self.ui.selection = selection;
                result?;
            }
            Command::ReloadAsProtein { frame } => self.apply(|s| {
                if let Some(frame) = frame {
                    s.frame = frame;
                }
                s.mode = match s.mode {
                    ViewMode::FullTranslate => ViewMode::Default,
                    ViewMode::Default | ViewMode::QuickTranslate => ViewMode::FullTranslate,
                };
            })?,
            Command::SetTranslationFrame(frame) => self.apply(|s| s.frame = frame)?,
            Command::SetActiveType(alignment_type) => {
                if self.session()?.state().mode == ViewMode::FullTranslate {
                    bail!("leave the translated view (T) before changing the sequence type");
                }
                self.apply(|s| {
                    s.alignment_type = alignment_type;
                    if !alignment_type.supports_translation() && s.mode == ViewMode::QuickTranslate
                    {
                        s.mode = ViewMode::Default;
                    }
                })?;
            }
            Command::SetConsensusMethod(method) => self.session_mut()?.consensus_method = method,
            Command::SetDiffMode(mode) => self.session_mut()?.diff_mode = mode,
        }

        Ok(())
    }

    fn open_command_palette(&mut self) {
        let palette = self
            .session
            .as_ref()
            .map(|alignment| CommandPaletteState::from_alignment(alignment, self.gff.as_ref()))
            .unwrap_or_else(CommandPaletteState::empty);
        self.ui.layers.open_palette(palette);
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

    fn session(&self) -> Result<&Session> {
        self.session
            .as_ref()
            .ok_or_else(|| format_err!("no alignment loaded"))
    }

    fn session_mut(&mut self) -> Result<&mut Session> {
        self.session
            .as_mut()
            .ok_or_else(|| format_err!("no alignment loaded"))
    }

    fn apply(&mut self, change: impl FnOnce(&mut ViewState)) -> Result<()> {
        let position = self.ui.position;
        self.ui.position = self.session_mut()?.update(position, change)?;
        self.clear_mouse_selection();
        Ok(())
    }

    fn jump_to_column(&mut self, column: usize) -> Result<()> {
        let layout = self.session()?.layout();
        let (idx, len) = (layout.column_position(column), layout.columns().len());
        if idx == len {
            bail!("no visible column at or after position {}", column + 1);
        }
        self.ui.position.column = idx;
        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::ui_state::MouseSelection;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::Sequence {
        libmsa::Sequence {
            id: id.to_string(),
            residues: sequence.to_vec(),
        }
    }

    fn app_with_alignment(sequences: Vec<libmsa::Sequence>) -> App {
        let startup = StartupState {
            file_path: None,
            initial_position: 0,
        };
        let mut app = App::new(startup);
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        let model = AlignmentModel::new(alignment).unwrap();
        app.session = Some(model);
        app.ui.meta.loading_state = LoadingState::Loaded;
        app.refresh_viewport_bounds();
        app.rebuild_layout(Rect::new(0, 0, 40, 12));
        app
    }

    fn gff_with_overlapping_features() -> Gff {
        Gff {
            features: vec![
                gff::Feature {
                    name: "gene1".to_string(),
                    kind: gff::FeatureType::Gene,
                    range: 0..10,
                    strand: gff::Strand::Forward,
                },
                gff::Feature {
                    name: "gene2".to_string(),
                    kind: gff::FeatureType::Gene,
                    range: 0..10,
                    strand: gff::Strand::Forward,
                },
            ],
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn horizontal_scroll_updates_layout_local_feature_stacked() {
        let sequence = vec![b'A'; 120];
        let mut app = app_with_alignment(vec![raw("seq1", &sequence)]);
        app.gff = Some(gff_with_overlapping_features());
        app.rebuild_layout(app.layout_area);
        assert_eq!(app.app_layout.alignment_header.local_feature_rows, 2);

        app.execute_commands([Command::ScrollRight { amount: 50 }]);

        assert_eq!(app.ui.viewport.offsets.cols, 50);
        assert_eq!(app.app_layout.alignment_header.local_feature_rows, 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn jump_to_position_updates_layout_local_feature_stacked() {
        let sequence = vec![b'A'; 120];
        let mut app = app_with_alignment(vec![raw("seq1", &sequence)]);
        app.gff = Some(gff_with_overlapping_features());
        app.rebuild_layout(app.layout_area);
        assert_eq!(app.app_layout.alignment_header.local_feature_rows, 2);

        app.execute_commands([Command::JumpToPosition(50)]);

        assert_eq!(app.ui.viewport.offsets.cols, 50);
        assert_eq!(app.app_layout.alignment_header.local_feature_rows, 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn translation_toggle_keeps_selection() {
        let mut app =
            app_with_alignment(vec![raw("seq1", b"ATGAAATTT"), raw("seq2", b"ATGAAATTT")]);
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
    async fn reload_as_protein_clears_selection() {
        let mut app =
            app_with_alignment(vec![raw("seq1", b"ATGAAATTT"), raw("seq2", b"ATGAAATTT")]);
        app.ui.selection = Some(MouseSelection {
            sequence_id: 0,
            column: 0,
            end_sequence_id: 0,
            end_column: 2,
        });

        app.execute_commands([Command::ReloadAsProtein { frame: None }]);

        assert!(app.session.as_ref().unwrap().is_reloaded_as_protein());
        assert_eq!(
            app.session.as_ref().unwrap().base().active_type(),
            libmsa::AlignmentType::Protein
        );
        assert_eq!(app.ui.selection, None);

        app.execute_commands([Command::ReloadAsProtein { frame: None }]);

        assert!(!app.session.as_ref().unwrap().is_reloaded_as_protein());
        assert_eq!(
            app.session.as_ref().unwrap().base().active_type(),
            libmsa::AlignmentType::Dna
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reload_as_protein_keeps_locus_from_nt() {
        let sequence = vec![b'C'; 360];
        let mut app = app_with_alignment(vec![raw("seq1", &sequence)]);
        app.ui.viewport.jump_to_position(200);

        app.execute_commands([Command::ReloadAsProtein { frame: None }]);

        assert_eq!(app.ui.viewport.window().col_range.start, 66);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reload_as_dna_keeps_locus_from_aa() {
        let sequence = vec![b'C'; 360];
        let mut app = app_with_alignment(vec![raw("seq1", &sequence)]);
        app.ui.viewport.jump_to_position(200);

        app.execute_commands([Command::ReloadAsProtein { frame: None }]);
        app.ui.viewport.jump_to_position(70);

        app.execute_commands([Command::ReloadAsProtein { frame: None }]);

        assert_eq!(app.ui.viewport.window().col_range.start, 212);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gap_filter_blocked_during_translation() {
        let mut app =
            app_with_alignment(vec![raw("seq1", b"ATGAAATTT"), raw("seq2", b"ATGAAATTT")]);
        app.execute_commands([Command::ToggleTranslationView]);
        app.execute_commands([Command::SetGapFilter(Some(0.25))]);

        let notification = app.ui.notification.as_ref().unwrap();
        assert_eq!(
            notification.message,
            "filter-gaps is unavailable while translation is active"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn constant_filter_blocked_during_translation() {
        let mut app =
            app_with_alignment(vec![raw("seq1", b"ATGAAATTT"), raw("seq2", b"ATGAAATTT")]);
        app.execute_commands([Command::ToggleTranslationView]);
        app.execute_commands([Command::SetConstantFilter(Some(0.9))]);

        let notification = app.ui.notification.as_ref().unwrap();
        assert_eq!(
            notification.message,
            "filter-constant is unavailable while translation is active"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn translation_blocked_by_gap_filter() {
        let mut app = app_with_alignment(vec![raw("seq1", b"ATG---"), raw("seq2", b"ATG---")]);
        app.execute_commands([Command::SetGapFilter(Some(0.0))]);
        app.execute_commands([Command::ToggleTranslationView]);

        let notification = app.ui.notification.as_ref().unwrap();
        assert_eq!(
            notification.message,
            "translation is unavailable while a column filter is active"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn translation_blocked_by_constant_filter() {
        let mut app = app_with_alignment(vec![raw("seq1", b"ATGAAA"), raw("seq2", b"ATGAAA")]);
        app.execute_commands([Command::SetConstantFilter(Some(1.0))]);
        app.execute_commands([Command::ToggleTranslationView]);

        let notification = app.ui.notification.as_ref().unwrap();
        assert_eq!(
            notification.message,
            "translation is unavailable while a column filter is active"
        );
    }
}
