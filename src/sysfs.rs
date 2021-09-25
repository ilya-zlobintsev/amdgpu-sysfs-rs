use async_trait::async_trait;
use std::path::Path;
use tokio::fs;

#[async_trait]
pub trait SysFS {
    fn get_path(&self) -> &Path;

    /// Reads the content of a file in the SysFS.
    async fn read_file(&self, file: &str) -> Option<String> {
        match fs::read_to_string(self.get_path().join(file)).await {
            Ok(contents) => Some(contents.trim().to_string()),
            Err(_) => None,
        }
    }

    /// Write to a file in the SysFS.
    async fn write_file<C: AsRef<[u8]> + Send>(
        &self,
        file: &str,
        contents: C,
    ) -> Result<(), std::io::Error> {
        fs::write(self.get_path().join(file), contents).await
    }
}
