use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    NotAllowed(String),
    InvalidSysFS,
    ParseError { msg: String, line: usize },
    IoError(std::io::Error),
}

impl Error {
    pub fn unexpected_eol<T: Display>(expected_item: T, line: usize) -> Self {
        Self::ParseError {
            msg: format!("Unexpected EOL, expected {expected_item}"),
            line,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotAllowed(info) => write!(f, "not allowed: {info}"),
            Error::InvalidSysFS => write!(f, "invalid SysFS"),
            Error::ParseError { msg, line } => write!(f, "parse error: {msg} at line {line}"),
            Error::IoError(error) => write!(f, "io error: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}
