use std::{collections::HashMap, fmt, path::PathBuf};

use crate::{hw_mon::HwMon, sysfs::SysFS};

/// A `GpuController` represents a handle over a single GPU device, as exposed in the Linux SysFS.
#[derive(Debug)]
pub struct GpuController {
    sysfs_path: PathBuf,
    /// A collection of all [HwMon](../hw_mon/struct.HwMon.html)s bound to this GPU. They are used to expose real-time data.
    pub hw_monitors: Vec<HwMon>,
}

impl GpuController {
    /// Initializes a new `GpuController` from a given SysFS device path.
    ///
    /// Normally, the path should look akin to `/sys/class/drm/card0/device`,
    /// and it needs to at least contain a `uevent` file.
    pub fn new_from_path(sysfs_path: PathBuf) -> Result<Self, GpuControllerError> {
        let mut hw_monitors = Vec::new();

        if let Ok(hw_mons_iter) = std::fs::read_dir(sysfs_path.join("hwmon")) {
            for hw_mon_dir in hw_mons_iter {
                if let Ok(hw_mon_dir) = hw_mon_dir {
                    if let Ok(hw_mon) = HwMon::new_from_path(hw_mon_dir.path()) {
                        hw_monitors.push(hw_mon);
                    }
                }
            }
        }

        let gpu_controller = Self {
            sysfs_path,
            hw_monitors,
        };

        gpu_controller.get_uevent()?;

        Ok(gpu_controller)
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

    /// Returns the current power level. // TODO doc
    pub fn get_power_level(&self) -> Option<PowerLevel> {
        self.read_file("power_dpm_force_performance_level")
            .map(|power_level| {
                PowerLevel::from_str(&power_level).expect("Unexpected power level (driver bug?)")
            })
    }
}

impl SysFS for GpuController {
    fn get_path(&self) -> &std::path::Path {
        &self.sysfs_path
    }
}

pub enum PowerLevel {
    Auto,
    Low,
    High,
}

impl Default for PowerLevel {
    fn default() -> Self {
        PowerLevel::Auto
    }
}

impl PowerLevel {
    pub fn from_str(power_level: &str) -> Result<Self, GpuControllerError> {
        match power_level {
            "auto" | "Automatic" => Ok(PowerLevel::Auto),
            "high" | "Highest Clocks" => Ok(PowerLevel::High),
            "low" | "Lowest Clocks" => Ok(PowerLevel::Low),
            _ => Err(GpuControllerError::ParseError(
                "unrecognized GPU power profile".to_string(),
            )),
        }
    }
}

impl fmt::Display for PowerLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                PowerLevel::Auto => "auto",
                PowerLevel::High => "high",
                PowerLevel::Low => "low",
            }
        )
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
