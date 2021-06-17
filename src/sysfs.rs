use std::path::Path;

pub trait SysFS {
    fn get_path(&self) -> &Path;

    fn read_file(&self, file: &str) -> Option<String> {
        match std::fs::read_to_string(self.get_path().join(file)) {
            Ok(contents) => Some(contents.trim().to_string()),
            Err(_) => None,
        }
    }
}