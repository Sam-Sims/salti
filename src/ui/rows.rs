use crate::config::theme::SequenceTheme;
use crate::core::lookups::{BYTE_TO_CHAR, translate_codon};
use crate::core::parser::SequenceType;
use crate::core::{CONSENSUS_BUFFER_COLS, CoreState};
use ratatui::style::{Styled, Stylize};
use ratatui::text::Span;
use std::ops::Range;

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
fn format_sequence_bytes(
    sequence: &[u8],
    sequence_theme: &SequenceTheme,
    sequence_type: SequenceType,
) -> Vec<Span<'static>> {
    match sequence_type {
        SequenceType::Dna => sequence
            .iter()
            .map(|&sequence_byte| {
                let character = BYTE_TO_CHAR[sequence_byte as usize];
                Span::styled(character, sequence_theme.nucleotide_style(sequence_byte))
            })
            .collect(),
        SequenceType::AminoAcid => sequence
            .iter()
            .map(|&sequence_byte| {
                let character = BYTE_TO_CHAR[sequence_byte as usize];
                Span::styled(character, sequence_theme.amino_acid_style(sequence_byte))
            })
            .collect(),
    }
}

#[inline]
fn format_sequence_bytes_with_reference(
    sequence: &[u8],
    reference: &[u8],
    sequence_theme: &SequenceTheme,
    sequence_type: SequenceType,
) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(sequence.len());

    for (&sequence_byte, &reference_byte) in sequence.iter().zip(reference.iter()) {
        if reference_byte == sequence_byte {
            spans.push(".".fg(sequence_theme.diff_match));
        } else {
            let character = BYTE_TO_CHAR[sequence_byte as usize];
            let style = match sequence_type {
                SequenceType::Dna => sequence_theme.nucleotide_style(sequence_byte),
                SequenceType::AminoAcid => sequence_theme.amino_acid_style(sequence_byte),
            };
            spans.push(character.set_style(style));
        }
    }

    spans
}

fn build_translated_state(
    sequence: &[u8],
    frame: u8,
    window: Range<usize>,
    buffer: usize,
) -> (Vec<Option<u8>>, Vec<bool>) {
    if window.end <= window.start {
        return (Vec::new(), Vec::new());
    }

    let frame = (frame % 3) as usize;
    let sequence_length = sequence.len();
    let window_start = window.start;
    let window_end = window.end.min(sequence_length);
    let window_len = window_end.saturating_sub(window_start);
    let mut amino_acid_for_position = vec![None; window_len];
    let mut is_centre = vec![false; window_len];

    let translation_start = window_start.saturating_sub(buffer);
    let translation_end = (window_end + buffer).min(sequence_length);

    let mut codon_start =
        frame.max(translation_start - ((translation_start.saturating_sub(frame)) % 3));

    while codon_start < translation_end {
        let amino_acid = translate_codon(sequence, codon_start);
        for position in codon_start..(codon_start + 3) {
            if position >= window_start && position < window_end {
                let index = position - window_start;
                amino_acid_for_position[index] = Some(amino_acid);
                if position == codon_start + 1 {
                    is_centre[index] = true;
                }
            }
        }
        codon_start += 3;
    }

    (amino_acid_for_position, is_centre)
}

fn build_translated_spans(
    sequence: &[u8],
    frame: u8,
    window: Range<usize>,
    buffer: usize,
    sequence_theme: &SequenceTheme,
) -> Vec<Span<'static>> {
    let (amino_acid_for_position, is_centre) =
        build_translated_state(sequence, frame, window, buffer);

    amino_acid_for_position
        .into_iter()
        .zip(is_centre)
        .map(|(amino_acid, is_centre)| {
            let Some(amino_acid) = amino_acid else {
                return Span::raw(" ");
            };
            let style = sequence_theme.amino_acid_style(amino_acid);
            let character = if is_centre { amino_acid as char } else { ' ' };
            character.to_string().set_style(style)
        })
        .collect()
}

fn render_translated_row_with_diff(
    sequence: &[u8],
    diff_against: &[u8],
    frame: u8,
    window: Range<usize>,
    buffer: usize,
    sequence_theme: &SequenceTheme,
) -> Vec<Span<'static>> {
    let (amino_acid_for_position, is_centre) =
        build_translated_state(sequence, frame, window.clone(), buffer);
    let (diff_amino_acid_for_position, _) =
        build_translated_state(diff_against, frame, window, buffer);

    amino_acid_for_position
        .into_iter()
        .zip(diff_amino_acid_for_position)
        .zip(is_centre)
        .map(|((amino_acid, diff_amino_acid), is_centre)| {
            let Some(amino_acid) = amino_acid else {
                return Span::raw(" ");
            };

            let matches = diff_amino_acid.is_some_and(|diff| diff == amino_acid);

            if is_centre {
                if matches {
                    return ".".fg(sequence_theme.diff_match);
                }

                let style = sequence_theme.amino_acid_style(amino_acid);
                return (amino_acid as char).to_string().set_style(style);
            }

            if matches {
                Span::raw(" ")
            } else {
                let style = sequence_theme.amino_acid_style(amino_acid);
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
    window: Range<usize>,
) -> Vec<Span<'static>> {
    let end = window.end.min(sequence.len());
    let sequence_slice = &sequence[window.start..end];

    if let Some(reference) = diff_against
        && window.start < reference.len()
    {
        let reference_end = window.end.min(reference.len());
        let reference_slice = &reference[window.start..reference_end];
        return format_sequence_bytes_with_reference(
            sequence_slice,
            reference_slice,
            sequence_theme,
            sequence_type,
        );
    }

    format_sequence_bytes(sequence_slice, sequence_theme, sequence_type)
}

#[must_use]
pub fn format_row_spans(
    sequence: &[u8],
    window: Range<usize>,
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
            window,
        ),
        RowRenderMode::Translate {
            frame,
            diff_against,
        } => match diff_against {
            Some(diff_against) => render_translated_row_with_diff(
                sequence,
                diff_against,
                frame,
                window,
                CONSENSUS_BUFFER_COLS,
                sequence_theme,
            ),
            None => build_translated_spans(
                sequence,
                frame,
                window,
                CONSENSUS_BUFFER_COLS,
                sequence_theme,
            ),
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

    let resolved_sequence_type = core.data.sequence_type.unwrap_or(SequenceType::Dna);

    let translate_nucleotide_to_amino_acid =
        core.translate_nucleotide_to_amino_acid && resolved_sequence_type == SequenceType::Dna;
    let diff_against = if core.show_reference_diff {
        reference_sequence
    } else if core.show_consensus_diff {
        consensus
    } else {
        None
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
