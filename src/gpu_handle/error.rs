use std::fmt::{self, Display};

#[derive(Debug)]
pub enum GpuHandleError {
    NotAllowed(String),
    InvalidSysFS,
    ParseError(String),
    IoError(std::io::Error),
}

impl From<std::io::Error> for GpuHandleError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl Display for GpuHandleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&match self {
            GpuHandleError::NotAllowed(info) => format!("not allowed: {info}"),
            GpuHandleError::InvalidSysFS => "invalid SysFS".to_owned(),
            GpuHandleError::ParseError(error) => format!("parse error: {error}"),
            GpuHandleError::IoError(error) => format!("io error: {error}"),
        })
    }
}
