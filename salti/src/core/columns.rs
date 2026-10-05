#[derive(Debug)]
pub struct Columns<'a> {
    pub grid: libmsa::Grid<'a>,
    pub cols: Vec<usize>,
    pub cells: Vec<Option<Cell>>,
    pub summaries: Vec<libmsa::ColumnSummary>,
}

#[derive(Debug)]
pub struct Cell {
    pub index: usize,
    pub centre: bool,
}
