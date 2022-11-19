use crate::{error::ErrorContext, Result};
use std::{fs, path::Path};

pub trait SysFS {
    fn get_path(&self) -> &Path;

    /// Reads the content of a file in the SysFS.
    fn read_file(&self, file: &str) -> Result<String> {
        Ok(fs::read_to_string(self.get_path().join(file))
            .with_context(|| format!("Could not read file {file}"))?
            .trim()
            .to_owned())
    }

    /// Write to a file in the SysFS.
    fn write_file<C: AsRef<[u8]> + Send>(&self, file: &str, contents: C) -> Result<()> {
        Ok(fs::write(self.get_path().join(file), contents)?)
    }
}
