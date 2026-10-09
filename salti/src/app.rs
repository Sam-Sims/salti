use std::path::Path;

use anyhow::{Context, Result};
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};
use tracing::debug;

use crate::{
    cli::StartupState,
    command::Command,
    core::{
        gff::{self, Gff},
        session::{Position, Session, ViewState},
    },
    input::{self, MouseTracker},
    jobs::{JobEvent, Jobs},
    ui::{
        layers::{
            notification::{Notification, NotificationLevel},
            palette::CommandPaletteState,
        },
        layout::ScreenLayout,
        panes::gff::GffPaneState,
        render::render,
        ui_state::UiState,
        utils::input_name,
    },
    update::{CRATE_VERSION, UpdateResult},
};

#[derive(Debug)]
pub(crate) struct App {
    session: Option<Session>,
    gff: Option<Gff>,
    ui: UiState,
    mouse_tracker: MouseTracker,
    jobs: Jobs,
    should_quit: bool,
    screen: ScreenLayout,
}

impl App {
    pub(crate) fn new(startup: StartupState) -> Self {
        let mut ui = UiState::default();
        let screen = ScreenLayout::new(Rect::default(), None, None, &mut ui.position);
        let mut app = Self {
            session: None,
            gff: None,
            ui,
            mouse_tracker: MouseTracker::default(),
            jobs: Jobs::new(),
            should_quit: false,
            screen,
        };
        if let Some(input) = startup.file_path {
            app.load(input);
        }
        if startup.update_check {
            app.jobs.check_for_update(false);
        }
        app
    }

    pub(crate) fn handle(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                self.ui.clear_notification();
                let commands = input::handle_key_event(&mut self.ui, key);
                self.execute_commands(commands);
            }
            Event::Mouse(mouse) => {
                let commands = input::handle_mouse_event(
                    &mut self.mouse_tracker,
                    self.session.as_ref(),
                    self.gff.as_ref(),
                    &mut self.ui,
                    &self.screen,
                    mouse,
                );
                self.execute_commands(commands);
            }
            Event::Resize(area) => self.rebuild_layout(area),
            Event::Job(JobEvent::Loaded { input, result }) => self.finish_load(input, result),
            Event::Job(JobEvent::UpdateChecked { result, requested }) => {
                self.finish_update_check(result, requested);
            }
        }
    }

    pub(crate) async fn next_job_event(&mut self) -> JobEvent {
        self.jobs.next().await
    }

    pub(crate) fn draw(&self, frame: &mut Frame) {
        render(
            frame,
            &self.screen,
            self.session.as_ref(),
            self.gff.as_ref(),
            &self.ui,
        );
    }

    pub(crate) fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn rebuild_layout(&mut self, area: Rect) {
        self.screen = ScreenLayout::new(
            area,
            self.session.as_ref(),
            self.gff.as_ref(),
            &mut self.ui.position,
        );
    }

    fn finish_load(&mut self, input: String, result: Result<libmsa::Alignment, String>) {
        self.ui.loading = None;
        match result {
            Ok(alignment) => {
                self.session = Some(Session::new(alignment, input));
                self.ui.clear_transient_state();
                self.mouse_tracker.clear_anchors();
                self.ui.position = Position::default();
                self.rebuild_layout(self.screen.area);
            }
            Err(error) => self.ui.notify(Notification {
                level: NotificationLevel::Error,
                message: format!("Failed to load {}: {error}", input_name(&input)),
            }),
        }
    }

    fn finish_update_check(&mut self, result: Option<UpdateResult>, requested: bool) {
        let message = match result {
            Some(UpdateResult::UpdateAvailable(latest)) => {
                format!(
                    "A new version of salti is available: {latest} (installed: {CRATE_VERSION})"
                )
            }
            Some(UpdateResult::UpToDate) if requested => "salti is up to date".to_string(),
            Some(UpdateResult::UpToDate) | None => return,
        };
        self.show_info(message);
    }

    fn execute_commands(&mut self, commands: Vec<Command>) {
        if commands.is_empty() {
            return;
        }
        for command in commands {
            if let Err(error) = self.execute_command(command) {
                debug!(error = ?error, "Command failed");
                self.ui.notify(Notification {
                    level: NotificationLevel::Error,
                    message: error.to_string(),
                });
            }
        }
        self.rebuild_layout(self.screen.area);
    }

    #[expect(clippy::too_many_lines)]
    fn execute_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Quit => self.should_quit = true,
            Command::OpenCommandPalette => self.open_command_palette(),
            Command::CloseOverlay => self.ui.layers.close_active(),
            Command::ToggleMinimap => self.ui.layers.toggle_minimap(),
            Command::SetTheme(theme_id) => self.ui.set_theme(theme_id),
            Command::ShowNotification(notification) => self.ui.notify(notification),
            Command::LoadAlignment(input) => {
                self.clear_mouse_selection();
                self.load(input);
            }
            Command::LoadGff { path } => {
                self.gff = Some(gff::parse_gff(Path::new(&path))?);
                self.ui.gff_pane = GffPaneState::default();
                self.show_info(format!("Loaded GFF file: {path}"));
            }
            Command::CheckForUpdate => self.jobs.check_for_update(true),

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
            Command::JumpToColumn(column) => {
                self.ui.position.column = self
                    .session()?
                    .layout()
                    .visible_column_position(column)
                    .with_context(|| {
                        format!("No visible column at or after position {}", column + 1)
                    })?;
            }
            Command::JumpToFeature(index) => {
                let gff = self
                    .gff
                    .as_ref()
                    .context("No GFF file is loaded. Load one with load-gff")?;
                let feature = &gff.features[index];
                let session = self.session()?;
                let column = session.column_range_at(feature.range.clone()).start;
                self.ui.position.column = session
                    .layout()
                    .visible_column_position(column)
                    .with_context(|| {
                        format!("No visible column at or after feature {}", feature.name)
                    })?;
            }
            Command::JumpToSequence(row) => {
                self.ui.position.row = self.session()?.layout().row_position(row);
            }
            Command::JumpToEnd => {
                self.ui.position.column =
                    self.session()?.layout().columns().len().saturating_sub(1);
            }
            Command::PinSequence(row) => self.apply(|s| {
                s.pin(row);
                Ok(())
            })?,
            Command::UnpinSequence(row) => self.apply(|s| {
                s.unpin(row);
                Ok(())
            })?,
            Command::SetReference(row) => self.apply(|s| {
                s.set_reference(row);
                Ok(())
            })?,
            Command::SetRowFilter(pattern) => {
                let regex = pattern
                    .as_deref()
                    .map(regex::Regex::new)
                    .transpose()
                    .context("Invalid row filter pattern")?;
                self.apply(|s| {
                    s.row_regex_filter = regex;
                    Ok(())
                })?;
            }
            Command::SetGapFilter(fraction) => self.apply(|s| s.set_gap_filter(fraction))?,
            Command::SetConstantFilter(fraction) => {
                self.apply(|s| s.set_constant_filter(fraction))?;
            }
            Command::ClearAllFilters => self.apply(|s| {
                s.row_regex_filter = None;
                s.clear_column_filter();
                Ok(())
            })?,
            Command::ToggleTranslationOverlay => {
                let mut selection = self.ui.selection;
                self.apply(ViewState::toggle_translation_overlay)?;
                let session = self.session()?;
                if let Some(selection) = &mut selection {
                    selection.columns = session.selection_columns(selection.columns);
                }
                self.ui.selection = selection;
            }
            Command::ToggleProteinView => self.apply(ViewState::toggle_protein_view)?,
            Command::SetTranslationFrame(frame) => self.apply(|s| {
                s.frame = frame;
                Ok(())
            })?,
            Command::SetActiveType(alignment_type) => {
                self.apply(|s| s.set_alignment_type(alignment_type))?;
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
            .map_or_else(CommandPaletteState::empty, |session| {
                CommandPaletteState::from_session(session, self.gff.as_ref())
            });
        self.ui.layers.open_palette(palette);
    }

    fn clear_mouse_selection(&mut self) {
        self.ui.selection = None;
        self.mouse_tracker.clear_anchors();
    }

    fn show_info(&mut self, message: String) {
        self.ui.notify(Notification {
            level: NotificationLevel::Info,
            message,
        });
    }

    fn session(&self) -> Result<&Session> {
        self.session
            .as_ref()
            .context("No alignment is loaded. Open one with load-alignment")
    }

    fn session_mut(&mut self) -> Result<&mut Session> {
        self.session
            .as_mut()
            .context("No alignment is loaded. Open one with load-alignment")
    }

    fn apply(&mut self, change: impl FnOnce(&mut ViewState) -> Result<()>) -> Result<()> {
        let position = self.ui.position;
        self.ui.position = self.session_mut()?.update(position, change)?;
        self.clear_mouse_selection();
        Ok(())
    }

    fn load(&mut self, input: String) {
        self.ui.loading = Some(input.clone());
        self.jobs.load(input);
    }
}

#[derive(Debug)]
pub(crate) enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(Rect),
    Job(JobEvent),
}

#[cfg(test)]
mod tests {
    use std::ops;

    use rstest::rstest;

    use super::*;
    use crate::{test_utils::session, ui::selection::Selection};

    const AREA: Rect = Rect::new(0, 0, 80, 24);
    const DNA: &[&[u8]] = &[b"ATGAAATTTCCC", b"ATG---TTTCCC", b"ATGAAGTTTCCC"];

    fn app(sequences: &[&[u8]]) -> App {
        let mut app = App::new(StartupState {
            file_path: None,
            update_check: false,
        });
        app.session = Some(session(sequences));
        app.handle(Event::Resize(AREA));
        app
    }

    fn selection(
        rows: ops::RangeInclusive<usize>,
        columns: ops::RangeInclusive<usize>,
    ) -> Selection {
        Selection {
            rows: rows.into(),
            columns: columns.into(),
        }
    }

    fn alignment(sequences: &[&[u8]]) -> libmsa::Alignment {
        libmsa::Alignment::new(
            sequences
                .iter()
                .map(|residues| libmsa::Sequence {
                    id: "new".to_string(),
                    residues: residues.to_vec(),
                })
                .collect(),
        )
        .unwrap()
    }

    fn notification_level(app: &App) -> Option<NotificationLevel> {
        app.ui.notification().map(|notification| notification.level)
    }

    #[test]
    fn execute_commands_clamps_once_after_batch() {
        let gapped: Vec<u8> = (0..200)
            .map(|i| if i % 2 == 0 { b'A' } else { b'-' })
            .collect();
        let mut app = app(&[&[b'A'; 200], &gapped]);

        app.execute_commands(vec![
            Command::ScrollRight { amount: 1000 },
            Command::SetGapFilter(Some(0.25)),
        ]);

        let window = &app.screen.window;
        assert_eq!(window.columns.end, 100);
        assert_eq!(app.ui.position.column, window.columns.start);
    }

    #[test]
    fn execute_commands_runs_commands_after_failure() {
        let mut app = app(&[&[b'A'; 200]]);

        app.execute_commands(vec![
            Command::SetRowFilter(Some("(".to_string())),
            Command::ScrollRight { amount: 3 },
        ]);

        assert_eq!(app.ui.position.column, 3);
        assert_eq!(notification_level(&app), Some(NotificationLevel::Error));
    }

    #[rstest]
    #[case::pin(Command::PinSequence(1))]
    #[case::reference(Command::SetReference(Some(1)))]
    #[case::row_filter(Command::SetRowFilter(Some("s0".to_string())))]
    #[case::gap_filter(Command::SetGapFilter(Some(0.5)))]
    #[case::clear_filters(Command::ClearAllFilters)]
    #[case::protein_view(Command::ToggleProteinView)]
    #[case::frame(Command::SetTranslationFrame(libmsa::ReadingFrame::Frame2))]
    fn execute_commands_view_change_clears_selection(#[case] command: Command) {
        let mut app = app(DNA);
        app.ui.selection = Some(selection(0..=1, 4..=4));

        app.execute_commands(vec![command]);

        assert_eq!(app.ui.selection, None);
    }

    #[test]
    fn execute_commands_translation_overlay_widens_selection() {
        let mut app = app(DNA);
        app.ui.selection = Some(selection(0..=1, 4..=4));

        app.execute_commands(vec![Command::ToggleTranslationOverlay]);

        assert_eq!(app.ui.selection, Some(selection(0..=1, 3..=5)));
    }

    #[test]
    fn execute_commands_quit_sets_should_quit() {
        let mut app = app(DNA);

        app.execute_commands(vec![Command::Quit]);

        assert!(app.should_quit());
    }

    #[test]
    fn finish_load_works() {
        let mut app = app(DNA);
        app.execute_commands(vec![Command::ScrollRight { amount: 2 }]);
        app.ui.selection = Some(selection(0..=0, 0..=0));
        app.ui.loading = Some("next.fasta".to_string());

        app.finish_load("next.fasta".to_string(), Ok(alignment(&[b"ACGT"])));

        let session = app.session.as_ref().unwrap();
        assert_eq!(session.source(), "next.fasta");
        assert_eq!(session.base_alignment().width(), 4);
        assert_eq!(app.ui.position.column, 0);
        assert_eq!(app.ui.selection, None);
        assert_eq!(app.ui.loading, None);
    }

    #[test]
    fn finish_load_rejects_keeps_open_alignment() {
        let mut app = app(DNA);
        app.ui.loading = Some("bad.fasta".to_string());

        app.finish_load("bad.fasta".to_string(), Err("not fasta".to_string()));

        assert_eq!(app.session.as_ref().unwrap().source(), "test.fasta");
        assert_eq!(app.ui.loading, None);
        assert_eq!(notification_level(&app), Some(NotificationLevel::Error));
    }

    #[rstest]
    #[case::available_at_startup(Some(UpdateResult::UpdateAvailable("99.0.0".to_string())), false)]
    #[case::up_to_date_requested(Some(UpdateResult::UpToDate), true)]
    fn finish_update_check_notifies(#[case] result: Option<UpdateResult>, #[case] requested: bool) {
        let mut app = app(DNA);

        app.finish_update_check(result, requested);

        assert_eq!(notification_level(&app), Some(NotificationLevel::Info));
    }

    #[rstest]
    #[case::up_to_date_at_startup(Some(UpdateResult::UpToDate), false)]
    #[case::failed(None, true)]
    fn finish_update_check_is_silent(
        #[case] result: Option<UpdateResult>,
        #[case] requested: bool,
    ) {
        let mut app = app(DNA);

        app.finish_update_check(result, requested);

        assert_eq!(notification_level(&app), None);
    }
}
