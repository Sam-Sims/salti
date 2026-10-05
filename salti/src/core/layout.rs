use anyhow::Result;

use crate::core::session::ViewState;

#[derive(Debug)]
pub struct Layout {
    rows: Vec<usize>,
    pinned: Vec<usize>,
    cols: Vec<usize>,
}

impl Layout {
    pub fn build(base_alignment: &libmsa::Alignment, state: &ViewState) -> Result<()> {
        // main and only way to build/rebuild layout
        todo!()
    }
}
