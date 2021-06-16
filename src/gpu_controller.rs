use std::{collections::HashMap, path::PathBuf};

pub struct GpuController {
    sysfs_path: PathBuf,
}

impl GpuController {
    pub fn new_from_path(sysfs_path: PathBuf) -> Result<Self, GpuControllerError> {
        let gpu_controller = Self { sysfs_path };

        gpu_controller.get_uevent()?;

        Ok(gpu_controller)
    }

    fn read_file(&self, file: &str) -> Option<String> {
        match std::fs::read_to_string(self.sysfs_path.join(file)) {
            Ok(contents) => Some(contents.trim().to_string()),
            Err(_) => None,
        }
    }

    fn get_uevent(&self) -> Result<HashMap<String, String>, GpuControllerError> {
        let raw = self
            .read_file("uevent")
            .ok_or_else(|| GpuControllerError::InvalidSysFS)?;

        let mut uevent = HashMap::new();

        for line in raw.trim().split('\n') {
            let (key, value) = line
                .split_once("=")
                .ok_or_else(|| GpuControllerError::ParseError("Missing =".to_string()))?;

            uevent.insert(key.to_owned(), value.to_owned());
        }

        Ok(uevent)
    }

    /// Gets the kernel driver used.
    pub fn get_driver(&self) -> String {
        self.get_uevent().unwrap().get("DRIVER").unwrap().clone()
    }

    /// Gets total VRAM size in bytes. May not be reported on some devices, such as integrated GPUs.
    pub fn get_total_vram(&self) -> Option<u64> {
        match self.read_file("mem_info_vram_total") {
            Some(total_vram) => {
                let total_vram = total_vram
                    .trim()
                    .parse()
                    .expect("Unexpected VRAM amount (driver bug?)");

                if total_vram == 0 {
                    None
                } else {
                    Some(total_vram)
                }
            }
            None => todo!(),
        }
    }

    /// Gets how much VRAM is currently used, in bytes. May not be reported on some devices, such as integrated GPUs.
    pub fn get_used_vram(&self) -> Option<u64> {
        match self.read_file("mem_info_vram_used") {
            Some(total_vram) => {
                let used_vram = total_vram
                    .trim()
                    .parse()
                    .expect("Unexpected VRAM amount (driver bug?)");

                if used_vram == 0 {
                    None
                } else {
                    Some(used_vram)
                }
            }
            None => todo!(),
        }
    }

    /// Returns the GPU busy percentage.
    pub fn get_busy_percent(&self) -> Option<u8> {
        self.read_file("gpu_busy_percent").map(|c| {
            c.trim()
                .parse()
                .expect("Unexpected GPU load percentage (driver bug?)")
        })
    }

    /// Returns the GPU VBIOS version. Empty if the GPU doesn't report one.
    pub fn get_vbios_version(&self) -> Option<String> {
        self.read_file("vbios_version")
    }
}

#[derive(Debug)]
pub enum GpuControllerError {
    InvalidSysFS,
    ParseError(String),
    IoError(std::io::Error),
}

impl From<std::io::Error> for GpuControllerError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}
