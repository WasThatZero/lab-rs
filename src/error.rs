//! error types for parsing `.lab` files

use std::fmt;

/// an error produced while parsing a `.lab` file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-indexed line number the error occurred on
    pub line: usize,
    /// what went wrong on that line
    pub kind: ParseErrorKind,
}

/// the specific kind of parse failure
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// the line is empty or has invalid label syntax
    InvalidLabel(String),
    /// a field that looked like a time could not be parsed as an integer
    InvalidTime(String),
    /// the start time is greater than the end time
    StartAfterEnd {
        /// parsed start time, in 100ns units
        start: u64,
        /// parsed end time, in 100ns units
        end: u64,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: ", self.line)?;
        match &self.kind {
            ParseErrorKind::InvalidLabel(s) => write!(f, "invalid label: {s}"),
            ParseErrorKind::InvalidTime(s) => write!(f, "invalid time value `{s}`"),
            ParseErrorKind::StartAfterEnd { start, end } => {
                write!(f, "start time {start} is after end time {end}")
            }
        }
    }
}

/// an error produced while scaling label timestamps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleError {
    /// the scaling factor is not a finite number
    NonFiniteFactor,
    /// the scaling factor is negative
    NegativeFactor,
}

impl fmt::Display for ScaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScaleError::NonFiniteFactor => f.write_str("scale factor must be finite"),
            ScaleError::NegativeFactor => f.write_str("scale factor must not be negative"),
        }
    }
}

impl std::error::Error for ScaleError {}

impl std::error::Error for ParseError {}

/// an error produced while reading a .lab from a reader or path
#[derive(Debug)]
#[non_exhaustive]
pub enum ReadError {
    /// an io error occurred while reading
    Io(std::io::Error),
    /// the contents were read but could not be parsed
    Parse(ParseError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Io(e) => write!(f, "i/o error: {e}"),
            ReadError::Parse(e) => write!(f, "parse error: {e}"),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReadError::Io(e) => Some(e),
            ReadError::Parse(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for ReadError {
    fn from(e: std::io::Error) -> Self {
        ReadError::Io(e)
    }
}

impl From<ParseError> for ReadError {
    fn from(e: ParseError) -> Self {
        ReadError::Parse(e)
    }
}
