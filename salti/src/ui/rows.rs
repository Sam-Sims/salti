use crate::config::theme::SequenceTheme;
use crate::core::CoreState;
use crate::core::command::DiffMode;
use crate::core::lookups::{BYTE_TO_CHAR, translate_codon};
use crate::core::parser::SequenceType;
use ratatui::style::{Styled, Stylize};
use ratatui::text::Span;

#[derive(Debug, Clone, Copy)]
pub enum RowRenderMode<'a> {
    Raw {
        sequence_type: SequenceType,
        diff_against: Option<&'a [u8]>,
    },
    Translate {
        frame: u8,
        diff_against: Option<&'a [u8]>,
    },
}

#[inline]
fn span_for_sequence_byte(
    sequence_byte: u8,
    sequence_theme: &SequenceTheme,
    sequence_type: SequenceType,
) -> Span<'static> {
    let character = BYTE_TO_CHAR[usize::from(sequence_byte)];
    let style = sequence_theme.style_for(sequence_byte, sequence_type);
    Span::styled(character, style)
}

#[inline]
fn format_sequence_bytes(
    sequence: &[u8],
    absolute_columns: &[usize],
    sequence_theme: &SequenceTheme,
    sequence_type: SequenceType,
) -> Vec<Span<'static>> {
    absolute_columns
        .iter()
        .map(|&absolute_col| {
            sequence.get(absolute_col).copied().map_or_else(
                || Span::raw(" "),
                |byte| span_for_sequence_byte(byte, sequence_theme, sequence_type),
            )
        })
        .collect()
}

#[inline]
fn format_sequence_bytes_with_reference(
    sequence: &[u8],
    reference: &[u8],
    absolute_columns: &[usize],
    sequence_theme: &SequenceTheme,
    sequence_type: SequenceType,
) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(absolute_columns.len());

    for &absolute_col in absolute_columns {
        let Some(sequence_byte) = sequence.get(absolute_col).copied() else {
            spans.push(Span::raw(" "));
            continue;
        };

        if reference.get(absolute_col).copied() == Some(sequence_byte) {
            spans.push(".".fg(sequence_theme.diff_match));
        } else {
            spans.push(span_for_sequence_byte(
                sequence_byte,
                sequence_theme,
                sequence_type,
            ));
        }
    }

    spans
}

#[inline]
fn translated_column(sequence: &[u8], frame: u8, absolute_col: usize) -> Option<(u8, bool)> {
    let frame = usize::from(frame % 3);
    if absolute_col < frame {
        return None;
    }

    let phase = (absolute_col - frame) % 3;
    let codon_start = absolute_col - phase;
    let amino_acid = translate_codon(sequence, codon_start);
    Some((amino_acid, phase == 1))
}

fn build_translated_spans(
    sequence: &[u8],
    frame: u8,
    absolute_columns: &[usize],
    sequence_theme: &SequenceTheme,
) -> Vec<Span<'static>> {
    absolute_columns
        .iter()
        .map(|&absolute_col| {
            let Some((amino_acid, is_centre)) = translated_column(sequence, frame, absolute_col)
            else {
                return Span::raw(" ");
            };
            let style = sequence_theme.style_for(amino_acid, SequenceType::AminoAcid);
            let character = if is_centre { amino_acid as char } else { ' ' };
            character.to_string().set_style(style)
        })
        .collect()
}

fn render_translated_row_with_diff(
    sequence: &[u8],
    diff_against: &[u8],
    frame: u8,
    absolute_columns: &[usize],
    sequence_theme: &SequenceTheme,
) -> Vec<Span<'static>> {
    absolute_columns
        .iter()
        .map(|&absolute_col| {
            let Some((amino_acid, is_centre)) = translated_column(sequence, frame, absolute_col)
            else {
                return Span::raw(" ");
            };

            let matches = translated_column(diff_against, frame, absolute_col)
                .is_some_and(|(diff_amino_acid, _)| diff_amino_acid == amino_acid);

            if is_centre {
                if matches {
                    return ".".fg(sequence_theme.diff_match);
                }

                let style = sequence_theme.style_for(amino_acid, SequenceType::AminoAcid);
                return (amino_acid as char).to_string().set_style(style);
            }

            if matches {
                Span::raw(" ")
            } else {
                let style = sequence_theme.style_for(amino_acid, SequenceType::AminoAcid);
                " ".set_style(style)
            }
        })
        .collect()
}

fn render_raw_row(
    sequence: &[u8],
    diff_against: Option<&[u8]>,
    sequence_theme: &SequenceTheme,
    sequence_type: SequenceType,
    absolute_columns: &[usize],
) -> Vec<Span<'static>> {
    match diff_against {
        Some(reference) => format_sequence_bytes_with_reference(
            sequence,
            reference,
            absolute_columns,
            sequence_theme,
            sequence_type,
        ),
        None => format_sequence_bytes(sequence, absolute_columns, sequence_theme, sequence_type),
    }
}

#[must_use]
pub fn format_row_spans(
    sequence: &[u8],
    absolute_columns: &[usize],
    sequence_theme: &SequenceTheme,
    mode: RowRenderMode<'_>,
) -> Vec<Span<'static>> {
    match mode {
        RowRenderMode::Raw {
            sequence_type,
            diff_against,
        } => render_raw_row(
            sequence,
            diff_against,
            sequence_theme,
            sequence_type,
            absolute_columns,
        ),
        RowRenderMode::Translate {
            frame,
            diff_against,
        } => match diff_against {
            Some(diff_against) => render_translated_row_with_diff(
                sequence,
                diff_against,
                frame,
                absolute_columns,
                sequence_theme,
            ),
            None => build_translated_spans(sequence, frame, absolute_columns, sequence_theme),
        },
    }
}

#[must_use]
pub fn select_row_render_mode<'a>(
    core: &'a CoreState,
    consensus: Option<&'a [u8]>,
) -> RowRenderMode<'a> {
    let reference_sequence = core
        .reference_alignment()
        .map(|alignment| alignment.sequence.as_ref());

    let resolved_sequence_type = core.sequence_type();

    let translate_nucleotide_to_amino_acid =
        core.translate_nucleotide_to_amino_acid && resolved_sequence_type == SequenceType::Dna;
    let diff_against = match core.diff_mode {
        DiffMode::Off => None,
        DiffMode::Reference => reference_sequence,
        DiffMode::Consensus => consensus,
    };

    if translate_nucleotide_to_amino_acid {
        RowRenderMode::Translate {
            frame: core.translation_frame,
            diff_against,
        }
    } else {
        RowRenderMode::Raw {
            sequence_type: resolved_sequence_type,
            diff_against,
        }
    }
}
