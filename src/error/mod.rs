//! SysFS errors
mod context;

pub(crate) use context::ErrorContext;

use std::{
    fmt::Display,
    num::{ParseFloatError, ParseIntError},
};

#[derive(Debug, PartialEq)]
/// An error that can happen when working with the SysFs
pub struct Error {
    context: Option<String>,
    /// The error kind
    pub kind: ErrorKind,
}

/// Possible types of errors
#[derive(Debug)]
pub enum ErrorKind {
    /// It is not allowed to perform the given action
    NotAllowed(String),
    /// Something is potentially unsupported by this library
    Unsupported(String),
    /// The given path is not a valid SysFs
    InvalidSysFS,
    /// An error that happens during parsing
    ParseError {
        /// What went wrong during parsing
        msg: String,
        /// The line where the error occured
        line: usize,
    },
    /// An IO error
    IoError(std::io::Error),
}

impl Error {
    pub(crate) fn unexpected_eol<T: Display>(expected_item: T, line: usize) -> Self {
        ErrorKind::ParseError {
            msg: format!("Unexpected EOL, expected {expected_item}"),
            line,
        }
        .into()
    }

    pub(crate) fn basic_parse_error(msg: String) -> Self {
        ErrorKind::ParseError { msg, line: 1 }.into()
    }

    pub(crate) fn not_allowed(msg: String) -> Self {
        ErrorKind::NotAllowed(msg).into()
    }

    /// If the error means that the file doesn't exist
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

impl std::error::Error for Error {}

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

impl PartialEq for ErrorKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::IoError(l0), Self::IoError(r0)) => l0.kind() == r0.kind(),
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}
