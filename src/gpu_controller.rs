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

    /// Returns the currently forced performance level.
    pub fn get_power_force_performance_level(&self) -> Option<PerformanceLevel> {
        self.read_file("power_dpm_force_performance_level")
            .map(|power_level| {
                PerformanceLevel::from_str(&power_level)
                    .expect("Unexpected power level (driver bug?)")
            })
    }

    /// Forces a given performance level.
    pub fn set_power_force_performance_level(
        &self,
        level: PerformanceLevel,
    ) -> Result<(), GpuControllerError> {
        Ok(self.write_file("power_dpm_force_performance_level", level.to_string())?)
    }

    /// Retuns the list of power levels and index of the currently active level for a given kind of power state.
    pub fn get_power_levels(&self, kind: PowerStateKind) -> Option<(Vec<String>, u8)> {
        self.read_file(kind.to_filename()).map(|content| {
            let mut power_levels = Vec::new();
            let mut active = 0;

            for mut line in content.trim().split('\n') {
                if let Some(stripped) = line.strip_suffix("*") {
                    line = stripped;

                    if let Some(identifier) = stripped.split(":").next() {
                        active = identifier
                            .trim()
                            .parse()
                            .expect("Unexpected power level identifier");
                    }
                }
                if let Some(s) = line.split(":").last() {
                    power_levels.push(s.trim().to_string());
                }
            }

            (power_levels, active)
        })
    }

    /// Sets the enabled power levels for a power state kind to a given list of levels. This means that only the given power levels will be allowed.
    /// 
    /// Can only be used if `power_force_performance_level` is set to `manual`.
    pub fn set_enabled_power_levels(
        &self,
        kind: PowerStateKind,
        levels: &[u8],
    ) -> Result<(), GpuControllerError> {
        match self.get_power_force_performance_level() {
            Some(PerformanceLevel::Manual) => {
                let mut s = String::new();

                for l in levels {
                    s.push(char::from_digit((*l).into(), 10).unwrap());
                    s.push(' ');
                }

                Ok(self.write_file(kind.to_filename(), s)?)
            }
            _ => Err(GpuControllerError::NotAllowed(
                "power_force_performance level needs to be set to 'manual' to adjust power levels"
                    .to_string(),
            )),
        }
    }
}

impl SysFS for GpuController {
    fn get_path(&self) -> &std::path::Path {
        &self.sysfs_path
    }
}

pub enum PowerStateKind {
    CoreClock,
    MemoryClock,
    SOCClock,
    FabricClock,
    DCEFClock,
    PcieSpeed,
}

impl PowerStateKind {
    pub fn to_filename(&self) -> &str {
        match self {
            PowerStateKind::CoreClock => "pp_dpm_sclk",
            PowerStateKind::MemoryClock => "pp_dpm_mclk",
            PowerStateKind::SOCClock => "pp_dpm_socclk",
            PowerStateKind::FabricClock => "pp_dpm_fclk",
            PowerStateKind::DCEFClock => "pp_dpm_dcefclk",
            PowerStateKind::PcieSpeed => "pp_dpm_pcie",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerformanceLevel {
    Auto,
    Low,
    High,
    Manual,
}

impl Default for PerformanceLevel {
    fn default() -> Self {
        PerformanceLevel::Auto
    }
}

impl PerformanceLevel {
    pub fn from_str(power_level: &str) -> Result<Self, GpuControllerError> {
        match power_level {
            "auto" | "Automatic" => Ok(PerformanceLevel::Auto),
            "high" | "Highest Clocks" => Ok(PerformanceLevel::High),
            "low" | "Lowest Clocks" => Ok(PerformanceLevel::Low),
            "manual" | "Manual" => Ok(PerformanceLevel::Manual),
            _ => Err(GpuControllerError::ParseError(
                "unrecognized GPU power profile".to_string(),
            )),
        }
    }
}

impl fmt::Display for PerformanceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                PerformanceLevel::Auto => "auto",
                PerformanceLevel::High => "high",
                PerformanceLevel::Low => "low",
                PerformanceLevel::Manual => "manual",
            }
        )
    }
}

#[derive(Debug)]
pub enum GpuControllerError {
    NotAllowed(String),
    InvalidSysFS,
    ParseError(String),
    IoError(std::io::Error),
}

impl From<std::io::Error> for GpuControllerError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}
