use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeError {
    pub row: usize,
    pub expected: usize,
    pub actual: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataError {
    Shape(ShapeError),
    EmptySchemaWithRows,
    MissingRow(usize),
}

impl fmt::Display for ShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "row {} has {} cells, expected {}",
            self.row, self.actual, self.expected
        )
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape(e) => e.fmt(f),
            Self::EmptySchemaWithRows => f.write_str("rows supplied for a schema with no columns"),
            Self::MissingRow(row) => write!(f, "row {row} does not exist"),
        }
    }
}

impl std::error::Error for ShapeError {}
impl std::error::Error for DataError {}

pub fn validate_rectangular<T>(width: usize, rows: &[Vec<T>]) -> Result<(), DataError> {
    if width == 0 && !rows.is_empty() {
        return Err(DataError::EmptySchemaWithRows);
    }
    for (row, cells) in rows.iter().enumerate() {
        if cells.len() != width {
            return Err(DataError::Shape(ShapeError {
                row,
                expected: width,
                actual: cells.len(),
            }));
        }
    }
    Ok(())
}
