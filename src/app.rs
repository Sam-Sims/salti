use std::time::Duration;

use color_eyre::{Result, eyre::eyre};
use crossterm::event::{Event, EventStream, KeyCode};

use crate::components::alignment::AlignmentComponent;
use crate::components::consensus::ConsensusComponent;
use crate::components::help::HelpComponent;
use crate::components::jump::JumpComponent;
use crate::components::sequence_id::SequenceIdComponent;
use crate::components::ui::UiComponent;
use crate::config::keybindings::{KeyAction, KeyBindings};
use crate::config::options::Options;
use crate::config::schemes::ColorSchemeFormatter;
use crate::layout::AppLayout;
use crate::parser::{self, Alignment};
use crate::state::{LoadingState, State};
use crate::viewport::Viewport;
use ratatui::{DefaultTerminal, Frame};
use tokio_stream::StreamExt;

const SCROLL_AMOUNT: usize = 1;
const SKIP_SCROLL_AMOUNT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Alignment,
    Help,
    Jump,
}

#[derive(Debug)]
pub struct App {
    should_quit: bool,
    state: AppMode,
    app_state: State,
    viewport: Viewport,
    ui_component: UiComponent,
    alignment_component: AlignmentComponent,
    consensus_component: ConsensusComponent,
    sequence_id_component: SequenceIdComponent,
    help_component: HelpComponent,
    jump_component: JumpComponent,
    keybindings: KeyBindings,
    options: Options,
    loading_receiver: Option<tokio::sync::oneshot::Receiver<Result<Vec<Alignment>>>>,
    sequence_type: Option<parser::SequenceType>,
}

impl App {
    pub fn new(options: Options) -> Self {
        let keybindings = KeyBindings::default();

        let (consensus_tx, consensus_rx) = tokio::sync::watch::channel(Vec::new());

        let mut app_state = State::new(consensus_rx);
        app_state.file_path = Some(options.file_path.clone());
        app_state.color_scheme_manager = ColorSchemeFormatter::new(options.color_scheme);
        let viewport = Viewport::default();
        let main_ui_component = UiComponent;
        let alignment_component = AlignmentComponent;
        let id_view_component = SequenceIdComponent;
        let help_component = HelpComponent;
        let consensus_component = ConsensusComponent::new(consensus_tx);
        let jump_component = JumpComponent::new();

        Self {
            should_quit: false,
            state: AppMode::Alignment,
            app_state,
            viewport,
            ui_component: main_ui_component,
            alignment_component,
            consensus_component,
            sequence_id_component: id_view_component,
            help_component,
            jump_component,
            keybindings,
            options,
            loading_receiver: None,
            sequence_type: None,
        }
    }

    fn parse_alignments(&mut self) {
        self.app_state.loading_state = LoadingState::Loading;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.loading_receiver = Some(rx);

        if let Some(file_path) = self.app_state.file_path.clone() {
            if file_path.as_os_str() == "-" {
                tokio::spawn(async move {
                    let result = parser::parse_fasta_stdin().await;
                    let _ = tx.send(result);
                });
                return;
            } else {
                tokio::spawn(async move {
                    let result = parser::parse_fasta_file(file_path).await;
                    let _ = tx.send(result);
                });
            }
        }
    }

    fn on_parse_success(&mut self, alignments: Vec<Alignment>) {
        self.app_state.load_alignments(alignments);
        let sequence_length = self.app_state.sequence_length;
        let sequence_count = self.app_state.alignments.len();
        // viewport keeps its own state - this is fine as these dont change once loaded
        self.viewport
            .set_sequence_params(sequence_length, sequence_count);
        self.viewport
            .set_initial_position(self.options.initial_position);

        self.sequence_type = Some(parser::detect_sequence_type(&self.app_state.alignments));
    }

    fn switch_app_mode(&mut self, new_state: AppMode) {
        match new_state {
            AppMode::Jump => {
                let alignments = &self.app_state.alignments;
                let sequence_names: Vec<String> =
                    alignments.iter().map(|a| a.id.to_string()).collect();
                self.jump_component.clear(&sequence_names);
            }
            AppMode::Help | AppMode::Alignment => {}
        }
        self.state = new_state;
    }

    fn execute_action(&mut self, action: KeyAction) {
        match action {
            KeyAction::Quit => self.should_quit = true,
            KeyAction::ToggleHelp => match self.state {
                AppMode::Help => self.switch_app_mode(AppMode::Alignment),
                _ => self.switch_app_mode(AppMode::Help),
            },
            KeyAction::ToggleJump => {
                self.switch_app_mode(AppMode::Jump);
            }
            KeyAction::CloseWidget => {
                self.switch_app_mode(AppMode::Alignment);
            }
            KeyAction::RunJump => {
                if let Some(seq_index) = self.jump_component.get_selected_sequence_index() {
                    self.viewport.jump_to_sequence(seq_index);
                }
                if let Some(position) = self.jump_component.get_position() {
                    self.viewport.jump_to_position(position.saturating_sub(1));
                }
                self.switch_app_mode(AppMode::Alignment);
            }
            KeyAction::JumpInputChar(c) => {
                let alignments = &self.app_state.alignments;
                let sequence_names: Vec<String> =
                    alignments.iter().map(|a| a.id.to_string()).collect();
                self.jump_component.add_char(c, &sequence_names);
            }
            KeyAction::JumpInputBackspace => {
                let alignments = &self.app_state.alignments;
                let sequence_names: Vec<String> =
                    alignments.iter().map(|a| a.id.to_string()).collect();
                self.jump_component.backspace(&sequence_names);
            }
            KeyAction::JumpToggleMode => {
                self.jump_component.toggle_mode();
            }
            KeyAction::JumpMoveUp => {
                self.jump_component.move_selection_up();
            }
            KeyAction::JumpMoveDown => {
                self.jump_component.move_selection_down();
            }
            KeyAction::ScrollDown => {
                self.viewport.scroll_down(SCROLL_AMOUNT);
            }
            KeyAction::SkipDown => {
                self.viewport.scroll_down(SKIP_SCROLL_AMOUNT);
            }
            KeyAction::ScrollUp => {
                self.viewport.scroll_up(SCROLL_AMOUNT);
            }
            KeyAction::SkipUp => {
                self.viewport.scroll_up(SKIP_SCROLL_AMOUNT);
            }
            KeyAction::ScrollLeft => {
                self.viewport.scroll_left(SCROLL_AMOUNT);
            }
            KeyAction::ScrollRight => {
                self.viewport.scroll_right(SCROLL_AMOUNT);
            }
            KeyAction::SkipLeft => {
                self.viewport.scroll_left(SKIP_SCROLL_AMOUNT);
            }
            KeyAction::SkipRight => {
                self.viewport.scroll_right(SKIP_SCROLL_AMOUNT);
            }
            KeyAction::JumpToStart => {
                self.viewport.jump_to_start();
            }
            KeyAction::JumpToEnd => {
                self.viewport.jump_to_end();
            }
            KeyAction::CycleColorScheme => {
                self.app_state.cycle_color_scheme();
            }
        }
    }

    fn handle_event(&mut self, event: &Event) {
        if let Some(key) = event.as_key_press_event() {
            if let Some(binding) =
                self.keybindings
                    .loookup_binding_context(key.code, key.modifiers, self.state)
            {
                self.execute_action(binding.action);
                return;
            }

            if let KeyCode::Char(c) = key.code {
                self.execute_action(KeyAction::JumpInputChar(c));
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let layout = AppLayout::new(frame.area());

        let visible_width = layout.sequence_area.width.saturating_sub(2) as usize;
        let visible_height = layout.sequence_area.height.saturating_sub(4) as usize;
        self.viewport
            .update_dimensions(visible_width, visible_height);

        let viewport = self.viewport.clone();
        self.ui_component
            .render(frame, frame.area(), &self.app_state, &viewport);

        self.sequence_id_component
            .render(frame, &layout, &self.app_state, &viewport);

        self.alignment_component
            .render(frame, &layout, &self.app_state, &viewport);

        self.consensus_component
            .render(frame, &layout, &mut self.app_state, &viewport);

        match self.state {
            AppMode::Help => {
                self.help_component
                    .render(frame, frame.area(), &self.keybindings);
            }
            AppMode::Jump => {
                self.jump_component
                    .render(frame, frame.area(), &self.app_state);
            }
            AppMode::Alignment => {}
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.parse_alignments();

        let period = Duration::from_secs_f32(1.0 / self.options.fps);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => { terminal.draw(|frame| { self.render(frame) })?; },
                Some(Ok(event)) = events.next() => self.handle_event(&event),
                result = async {
                    if let Some(receiver) = &mut self.loading_receiver {
                        receiver.await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    self.loading_receiver = None;
                    match result {
                        Ok(Ok(alignments)) => {
                            self.on_parse_success(alignments);
                        }
                        Ok(Err(e)) => {
                            return Err(eyre!("Failed to load alignments: {}", e));
                        }
                        Err(_) => {
                        }
                    }
                },
            }
        }
        Ok(())
    }
}
