use crate::parser::Alignment;
use crate::state::State;
use crate::viewport::Viewport;
use ratatui::Frame;
use ratatui::layout::Rect;

const BUFFER_SIZE: usize = 500;
const RECALC_THRESHOLD: usize = 25;

#[derive(Debug)]
pub struct ConsensusComponent {
    current_window: Option<(usize, usize)>,
    consensus_sender: tokio::sync::watch::Sender<Vec<u8>>,
}

impl ConsensusComponent {
    pub fn new(consensus_sender: tokio::sync::watch::Sender<Vec<u8>>) -> Self {
        Self {
            current_window: None,
            consensus_sender,
        }
    }

    pub fn update(&mut self, viewport: &Viewport, app_state: &mut State) {
        let sequence_length = app_state.sequence_length;
        if sequence_length == 0 || app_state.alignments.is_empty() {
            return;
        }

        let viewport_range = viewport.horizontal_range();
        let window_start = viewport_range.start.saturating_sub(BUFFER_SIZE);
        let window_end = (viewport_range.end + BUFFER_SIZE).min(sequence_length);
        let needed_window = (window_start, window_end);

        let should_update = match self.current_window {
            None => true,
            Some((current_start, current_end)) => {
                let viewport_start = viewport_range.start;
                let viewport_end = viewport_range.end;
                viewport_start < (current_start + RECALC_THRESHOLD)
                    || viewport_end > (current_end.saturating_sub(RECALC_THRESHOLD))
            }
        };

        if should_update {
            let existing_consensus = app_state.consensus.clone();
            let positions_to_calculate = Self::subset_missing_positions(
                needed_window.0,
                needed_window.1,
                existing_consensus.as_ref(),
            );

            if !positions_to_calculate.is_empty() {
                Self::calculate_positions(
                    positions_to_calculate,
                    app_state,
                    self.consensus_sender.clone(),
                );
            }

            self.current_window = Some(needed_window);
        }
    }

    fn subset_missing_positions(
        start: usize,
        end: usize,
        existing: Option<&Vec<u8>>,
    ) -> Vec<usize> {
        let mut missing = Vec::new();

        for pos in start..end {
            let is_calculated = existing
                .is_some_and(|consensus| consensus.get(pos).copied().unwrap_or(b' ') != b' ');

            if !is_calculated {
                missing.push(pos);
            }
        }

        missing
    }

    fn calculate_positions(
        positions: Vec<usize>,
        app_state: &State,
        sender: tokio::sync::watch::Sender<Vec<u8>>,
    ) {
        let alignments = app_state.alignments.clone();
        let existing_consensus = app_state.consensus.clone();
        let sequence_length = app_state.sequence_length;

        tokio::spawn(async move {
            let consensus = ConsensusComponent::calculate_consensus_at_position(
                &alignments,
                existing_consensus,
                sequence_length,
                positions,
            );
            let _ = sender.send(consensus);
        });
    }

    fn calculate_consensus_at_position(
        alignments: &[Alignment],
        existing: Option<Vec<u8>>,
        seq_length: usize,
        positions: Vec<usize>,
    ) -> Vec<u8> {
        let mut consensus = existing.unwrap_or_else(|| vec![b' '; seq_length]);

        for pos in positions {
            if pos >= seq_length {
                continue;
            }

            let mut counts = [0u32; 256];
            for alignment in alignments {
                if let Some(&nuc) = alignment.sequence.get(pos) {
                    counts[nuc as usize] += 1;
                }
            }

            let consensus_nuc = counts
                .iter()
                .enumerate()
                .filter(|(nuc, count)| *nuc != b'-' as usize && **count > 0)
                .max_by_key(|(_, count)| *count)
                .and_then(|(nuc, _)| u8::try_from(nuc).ok())
                .unwrap_or(b'*');

            consensus[pos] = consensus_nuc;
        }

        consensus
    }

    fn render_consensus_id(area: Rect, f: &mut Frame) {
        use ratatui::text::Line;
        use ratatui::widgets::{Block, Paragraph};

        let block = Block::bordered();
        let inner_area = block.inner(area);
        f.render_widget(block, area);

        let consensus_id_paragraph = Paragraph::new(vec![
            Line::from("Consensus Sequence:"),
            Line::from("Amino Acid translation:"),
        ]);
        f.render_widget(consensus_id_paragraph, inner_area);
    }

    fn render_consensus_sequence(
        area: Rect,
        app_state: &State,
        viewport: &Viewport,
        f: &mut Frame,
    ) {
        use ratatui::style::{Color, Style, Stylize};
        use ratatui::text::Line;
        use ratatui::widgets::{Block, Paragraph};

        let block = Block::bordered();
        let inner_area = block.inner(area);
        f.render_widget(block, area);

        let consensus = &app_state.consensus;
        let color_scheme = app_state.color_scheme;
        let _sequence_length = app_state.sequence_length;
        let horizontal_range = viewport.horizontal_range();

        let consensus_paragraph = if let Some(consensus_data) = &consensus {
            let seq_slice = &consensus_data
                [horizontal_range.start..horizontal_range.end.min(consensus_data.len())];
            let spans = crate::config::schemes::format_sequence_bytes(seq_slice, color_scheme);
            Paragraph::new(Line::from(spans))
        } else {
            let loading_message = Line::from("Calculating consensus...")
                .style(Style::default().fg(Color::DarkGray).italic());
            Paragraph::new(loading_message).alignment(ratatui::layout::Alignment::Center)
        };

        f.render_widget(consensus_paragraph, inner_area);
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        layout: &crate::layout::AppLayout,
        app_state: &mut State,
        viewport: &Viewport,
    ) {
        app_state.check_consensus_updates();
        self.update(viewport, app_state);

        Self::render_consensus_id(layout.consensus_id_area, f);

        Self::render_consensus_sequence(layout.consensus_sequence_area, app_state, viewport, f);
    }
}
