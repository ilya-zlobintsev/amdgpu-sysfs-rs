//! Utilities for working with SysFS.
use crate::{
    error::{Error, ErrorContext},
    Result,
};
use std::{fmt::Debug, fs, path::Path, str::FromStr};

/// General functionality of a SysFS.
pub trait SysFS {
    /// Gets the path of the current SysFS.
    fn get_path(&self) -> &Path;

    /// Reads the content of a file in the `SysFS`.
    fn read_file(&self, file: impl AsRef<Path> + Debug) -> Result<String> {
        let path = file.as_ref();
        Ok(fs::read_to_string(self.get_path().join(path))
            .with_context(|| format!("Could not read file {file:?}"))?
            .replace(char::from(0), "") // Workaround for random null bytes in SysFS entries
            .trim()
            .to_owned())
    }

    /// Reads the content of a file and then parses it
    fn read_file_parsed<T: FromStr<Err = E>, E: ToString>(&self, file: &str) -> Result<T> {
        fs::read_to_string(self.get_path().join(file))
            .with_context(|| format!("Could not read file {file}"))?
            .trim()
            .parse()
            .map_err(|err: E| Error::basic_parse_error(err.to_string()))
    }

    /// Write to a file in the `SysFS`.
    fn write_file<C: AsRef<[u8]> + Send>(&self, file: &str, contents: C) -> Result<()> {
        Ok(fs::write(self.get_path().join(file), contents)?)
    }
}
