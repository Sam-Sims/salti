use crate::core::CoreState;
use crate::core::parser::SequenceType;
use crate::ui::UiState;
use crate::ui::rows::{RowRenderMode, format_row_spans};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Styled, Stylize};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

const CONSERVATION_SPARK_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn render_consensus_pane(
    f: &mut Frame,
    layout: &crate::ui::layout::AppLayout,
    core: &CoreState,
    ui: &UiState,
) {
    render_consensus_sequence_id_pane(layout.consensus_sequence_id_pane_area, core, ui, f);
    render_consensus_alignment_pane(layout.consensus_alignment_pane_area, core, ui, f);
}

fn render_consensus_sequence_id_pane(area: Rect, _core: &CoreState, ui: &UiState, f: &mut Frame) {
    let theme = &ui.theme_styles;
    let block = Block::bordered()
        .border_style(theme.border)
        .style(theme.base_block)
        .merge_borders(MergeStrategy::Exact);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from("Reference Sequence:".set_style(theme.accent)),
        Line::from("Consensus Sequence:".set_style(theme.accent)),
        Line::from("Conservation:".set_style(theme.accent)),
    ];

    let consensus_id_paragraph = Paragraph::new(lines).style(theme.base_block);
    f.render_widget(consensus_id_paragraph, inner_area);
}

fn render_consensus_alignment_pane(area: Rect, core: &CoreState, ui: &UiState, f: &mut Frame) {
    let theme = &ui.theme_styles;
    let block = Block::bordered()
        .border_style(theme.border)
        .style(theme.base_block)
        .merge_borders(MergeStrategy::Exact);
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let consensus = &core.consensus;
    let conservation = &core.conservation;
    let window = core.viewport.window();
    let horizontal_range = window.col_range;
    let resolved_sequence_type = core.data.sequence_type.unwrap_or(SequenceType::Dna);
    let translate_nucleotide_to_amino_acid =
        core.translate_nucleotide_to_amino_acid && resolved_sequence_type == SequenceType::Dna;
    let render_mode = if translate_nucleotide_to_amino_acid {
        RowRenderMode::Translate {
            frame: core.translation_frame,
            diff_against: None,
        }
    } else {
        RowRenderMode::Raw {
            sequence_type: resolved_sequence_type,
            diff_against: None,
        }
    };

    let consensus_line = if let Some(consensus_data) = consensus {
        let spans = format_row_spans(
            consensus_data,
            horizontal_range.clone(),
            &ui.theme.sequence,
            render_mode,
        );
        Line::from(spans)
    } else {
        Line::from("Calculating consensus...".fg(ui.theme.text_dim).italic())
    };

    let reference_line = core.reference_alignment().map_or_else(
        || Line::from("No reference selected".fg(ui.theme.text_dim).italic()),
        |alignment| {
            let spans = format_row_spans(
                alignment.sequence.as_ref(),
                horizontal_range.clone(),
                &ui.theme.sequence,
                render_mode,
            );
            Line::from(spans)
        },
    );

    let conservation_line = if let Some(conservation_data) = conservation {
        let sparkline: String = horizontal_range
            .clone()
            .map(|position| {
                conservation_data
                    .get(position)
                    .copied()
                    .filter(|value| value.is_finite())
                    .map_or(' ', conservation_to_spark)
            })
            .collect();
        Line::from(sparkline).set_style(theme.accent_alt)
    } else {
        Line::from("Calculating conservation...".fg(ui.theme.text_dim).italic())
    };

    let lines = vec![reference_line, consensus_line, conservation_line];
    let consensus_paragraph = Paragraph::new(lines).style(theme.base_block);

    f.render_widget(consensus_paragraph, inner_area);
}

fn conservation_to_spark(value: f32) -> char {
    let value = value.clamp(0.0, 1.0);
    let max_idx = CONSERVATION_SPARK_CHARS.len() - 1;
    let idx = (value * max_idx as f32).round() as usize;
    CONSERVATION_SPARK_CHARS[idx]
}
