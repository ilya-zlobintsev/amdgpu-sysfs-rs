use std::fmt::Display;

#[derive(Debug, Clone)]
pub enum Error {
    ParseError { msg: String, line: usize },
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
            Error::ParseError { msg, line } => {
                write!(f, "parse error: {msg} at line {line}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
