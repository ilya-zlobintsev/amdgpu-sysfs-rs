mod context;

pub(crate) use context::ErrorContext;

use std::{
    fmt::Display,
    num::{ParseFloatError, ParseIntError},
};

#[derive(Debug)]
pub struct Error {
    pub context: Option<String>,
    pub kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    NotAllowed(String),
    Unsupported(String),
    InvalidSysFS,
    ParseError { msg: String, line: usize },
    IoError(std::io::Error),
}

impl Error {
    pub fn unexpected_eol<T: Display>(expected_item: T, line: usize) -> Self {
        Self {
            context: None,
            kind: ErrorKind::ParseError {
                msg: format!("Unexpected EOL, expected {expected_item}"),
                line,
            },
        }
    }

    pub fn basic_parse_error(msg: String) -> Self {
        Self {
            context: None,
            kind: ErrorKind::ParseError { msg, line: 1 },
        }
    }

    pub fn is_not_found(&self) -> bool {
        matches!(&self.kind, ErrorKind::IoError(io_err) if io_err.kind() == std::io::ErrorKind::NotFound)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::NotAllowed(info) => write!(f, "not allowed: {info}")?,
            ErrorKind::InvalidSysFS => write!(f, "invalid SysFS")?,
            ErrorKind::ParseError { msg, line } => write!(f, "parse error: {msg} at line {line}")?,
            ErrorKind::IoError(error) => write!(f, "io error: {error}")?,
            ErrorKind::Unsupported(err) => write!(f, "unsupported: {err}")?,
        }

        if let Some(ctx) = &self.context {
            write!(f, "\n{ctx}")?;
        }

        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self {
            context: None,
            kind,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self {
            context: None,
            kind: ErrorKind::IoError(err),
        }
    }
}

impl From<ParseIntError> for Error {
    fn from(err: ParseIntError) -> Self {
        Self::basic_parse_error(err.to_string())
    }
}

impl From<ParseFloatError> for Error {
    fn from(err: ParseFloatError) -> Self {
        Self::basic_parse_error(err.to_string())
    }
}
