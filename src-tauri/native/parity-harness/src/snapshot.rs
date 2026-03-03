use godly_protocol::types::{RichGridCell, RichGridData, RichGridRow};

#[derive(Debug)]
pub struct GridDiffResult {
    pub mismatches: Vec<CellMismatch>,
}

impl GridDiffResult {
    pub fn is_identical(&self) -> bool {
        self.mismatches.is_empty()
    }
}

#[derive(Debug)]
pub struct CellMismatch {
    pub row: usize,
    pub col: usize,
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

/// Compare two `RichGridData` snapshots cell-by-cell.
pub struct GridSnapshotComparator;

impl GridSnapshotComparator {
    pub fn compare(expected: &RichGridData, actual: &RichGridData) -> GridDiffResult {
        let mut mismatches = Vec::new();

        if expected.dimensions.rows != actual.dimensions.rows
            || expected.dimensions.cols != actual.dimensions.cols
        {
            mismatches.push(CellMismatch {
                row: 0,
                col: 0,
                field: "dimensions",
                expected: format!("{}x{}", expected.dimensions.rows, expected.dimensions.cols),
                actual: format!("{}x{}", actual.dimensions.rows, actual.dimensions.cols),
            });
            return GridDiffResult { mismatches };
        }

        if expected.cursor.row != actual.cursor.row || expected.cursor.col != actual.cursor.col {
            mismatches.push(CellMismatch {
                row: expected.cursor.row as usize,
                col: expected.cursor.col as usize,
                field: "cursor",
                expected: format!("({}, {})", expected.cursor.row, expected.cursor.col),
                actual: format!("({}, {})", actual.cursor.row, actual.cursor.col),
            });
        }

        let row_count = expected.rows.len().min(actual.rows.len());
        for row_idx in 0..row_count {
            Self::compare_rows(row_idx, &expected.rows[row_idx], &actual.rows[row_idx], &mut mismatches);
        }

        if expected.rows.len() != actual.rows.len() {
            mismatches.push(CellMismatch {
                row: row_count,
                col: 0,
                field: "row_count",
                expected: expected.rows.len().to_string(),
                actual: actual.rows.len().to_string(),
            });
        }

        GridDiffResult { mismatches }
    }

    fn compare_rows(row_idx: usize, expected: &RichGridRow, actual: &RichGridRow, mismatches: &mut Vec<CellMismatch>) {
        let col_count = expected.cells.len().min(actual.cells.len());
        for col_idx in 0..col_count {
            Self::compare_cells(row_idx, col_idx, &expected.cells[col_idx], &actual.cells[col_idx], mismatches);
        }
        if expected.cells.len() != actual.cells.len() {
            mismatches.push(CellMismatch {
                row: row_idx,
                col: col_count,
                field: "col_count",
                expected: expected.cells.len().to_string(),
                actual: actual.cells.len().to_string(),
            });
        }
    }

    fn compare_cells(row: usize, col: usize, expected: &RichGridCell, actual: &RichGridCell, mismatches: &mut Vec<CellMismatch>) {
        macro_rules! check {
            ($field:ident) => {
                if expected.$field != actual.$field {
                    mismatches.push(CellMismatch {
                        row, col,
                        field: stringify!($field),
                        expected: format!("{:?}", expected.$field),
                        actual: format!("{:?}", actual.$field),
                    });
                }
            };
        }
        check!(content);
        check!(fg);
        check!(bg);
        check!(bold);
        check!(dim);
        check!(italic);
        check!(underline);
        check!(inverse);
        check!(wide);
        check!(wide_continuation);
    }
}
