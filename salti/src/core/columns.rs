#[derive(Debug)]
pub struct WindowColumns<'a> {
    pub grid: libmsa::Grid<'a>,
    pub columns: Vec<usize>,
    pub cells: Vec<Option<Cell>>,
    pub summaries: Vec<libmsa::ColumnSummary>,
}

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub index: usize,
    pub centre: bool,
}
