use std::path::Path;

pub trait SysFS {
    fn get_path(&self) -> &Path;

    /// Reads the content of a file in the SysFS.
    fn read_file(&self, file: &str) -> Option<String> {
        match std::fs::read_to_string(self.get_path().join(file)) {
            Ok(contents) => Some(contents.trim().to_string()),
            Err(_) => None,
        }
    }

    /// Write to a file in the SysFS.
    fn write_file<C: AsRef<[u8]>>(&self, file: &str, contents: C) -> Result<(), std::io::Error> {
        std::fs::write(self.get_path().join(file), contents)
    }
}
