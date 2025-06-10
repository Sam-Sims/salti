use ratatui::layout::{Constraint, Layout, Rect};

const VERTICAL_BOTTOM_LENGTH: u16 = 6;
const HORIZONTAL_ID_PERCENTAGE: u16 = 12;

#[derive(Debug, Clone)]
pub struct AppLayout {
    pub id_area: Rect,
    pub sequence_area: Rect,
    pub consensus_id_area: Rect,
    pub consensus_sequence_area: Rect,
}

impl AppLayout {
    pub fn new(area: Rect) -> Self {
        let content_area = ratatui::widgets::Block::bordered().inner(area);

        let [alignment_area, consensus_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(VERTICAL_BOTTOM_LENGTH),
        ])
        .split(content_area)[..] else {
            panic!("Failed to split")
        };

        let [id_area, sequence_area] = Layout::horizontal([
            Constraint::Percentage(HORIZONTAL_ID_PERCENTAGE),
            Constraint::Fill(1),
        ])
        .split(alignment_area)[..] else {
            panic!("Failed to split")
        };
        let [consensus_id_area, consensus_sequence_area] = Layout::horizontal([
            Constraint::Percentage(HORIZONTAL_ID_PERCENTAGE),
            Constraint::Fill(1),
        ])
        .split(consensus_area)[..] else {
            panic!("Failed to split")
        };

        Self {
            id_area,
            sequence_area,
            consensus_id_area,
            consensus_sequence_area,
        }
    }
}
