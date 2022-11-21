pub mod overdrive;

use crate::{
    error::{Error, ErrorContext, ErrorKind},
    hw_mon::HwMon,
    sysfs::SysFS,
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, fs, path::PathBuf, str::FromStr};

/// A `GpuHandle` represents a handle over a single GPU device, as exposed in the Linux SysFS.
#[derive(Clone, Debug)]
pub struct GpuHandle {
    sysfs_path: PathBuf,
    /// A collection of all [HwMon](../hw_mon/struct.HwMon.html)s bound to this GPU. They are used to expose real-time data.
    pub hw_monitors: Vec<HwMon>,
    uevent: HashMap<String, String>,
}

impl GpuHandle {
    /// Initializes a new `GpuHandle` from a given SysFS device path.
    ///
    /// Normally, the path should look akin to `/sys/class/drm/card0/device`,
    /// and it needs to at least contain a `uevent` file.
    pub fn new_from_path(sysfs_path: PathBuf) -> Result<Self> {
        let mut hw_monitors = Vec::new();

        if let Ok(hw_mons_iter) = std::fs::read_dir(sysfs_path.join("hwmon")) {
            for hw_mon_dir in hw_mons_iter.flatten() {
                if let Ok(hw_mon) = HwMon::new_from_path(hw_mon_dir.path()) {
                    hw_monitors.push(hw_mon);
                }
            }
        }

        let uevent_raw = fs::read_to_string(sysfs_path.join("uevent"))?;

        let mut uevent = HashMap::new();

        for (i, line) in uevent_raw.trim().split('\n').enumerate() {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| Error::unexpected_eol("=", i))?;

            uevent.insert(key.to_owned(), value.to_owned());
        }

        match uevent.get("DRIVER") {
            Some(_) => Ok(Self {
                sysfs_path,
                hw_monitors,
                uevent,
            }),
            None => Err(ErrorKind::InvalidSysFS.into()),
        }
    }

    /// Gets the kernel driver used.
    pub fn get_driver(&self) -> &str {
        self.uevent.get("DRIVER").unwrap()
    }

    /// Gets the **GPU's** PCI vendor and ID. This is the ID of your GPU chip, e.g. AMD Radeon RX 580.
    pub fn get_pci_id(&self) -> Option<(&str, &str)> {
        match self.uevent.get("PCI_ID") {
            Some(pci_str) => pci_str.split_once(':'),
            None => None,
        }
    }

    /// Gets the **Card's** PCI vendor and ID. This is the ID of your card model, e.g. Sapphire RX 580 Pulse.
    pub fn get_pci_subsys_id(&self) -> Option<(&str, &str)> {
        match self.uevent.get("PCI_SUBSYS_ID") {
            Some(pci_str) => pci_str.split_once(':'),
            None => None,
        }
    }

    pub fn get_pci_slot_name(&self) -> Option<&str> {
        self.uevent.get("PCI_SLOT_NAME").map(|s| s.as_str())
    }

    pub fn get_current_link_speed(&self) -> Result<String> {
        self.read_file("current_link_speed")
    }

    pub fn get_current_link_width(&self) -> Result<String> {
        self.read_file("current_link_width")
    }

    pub fn get_max_link_speed(&self) -> Result<String> {
        self.read_file("max_link_speed")
    }
    pub fn get_max_link_width(&self) -> Result<String> {
        self.read_file("max_link_width")
    }

    fn read_vram_file(&self, file: &str) -> Result<u64> {
        let raw_vram = self.read_file(file)?;
        Ok(raw_vram.parse()?)
    }

    /// Gets total VRAM size in bytes. May not be reported on some devices, such as integrated GPUs.
    pub fn get_total_vram(&self) -> Result<u64> {
        self.read_vram_file("mem_info_vram_total")
    }

    /// Gets how much VRAM is currently used, in bytes. May not be reported on some devices, such as integrated GPUs.
    pub fn get_used_vram(&self) -> Result<u64> {
        self.read_vram_file("mem_info_vram_used")
    }

    /// Returns the GPU busy percentage.
    pub fn get_busy_percent(&self) -> Result<u8> {
        let raw_busy = self.read_file("gpu_busy_percent")?;
        Ok(raw_busy.parse()?)
    }

    /// Returns the GPU VBIOS version. Empty if the GPU doesn't report one.
    pub fn get_vbios_version(&self) -> Result<String> {
        self.read_file("vbios_version")
    }

    /// Returns the currently forced performance level.
    pub fn get_power_force_performance_level(&self) -> Result<PerformanceLevel> {
        let raw_level = self.read_file("power_dpm_force_performance_level")?;
        PerformanceLevel::from_str(&raw_level)
    }

    /// Forces a given performance level.
    pub fn set_power_force_performance_level(&self, level: PerformanceLevel) -> Result<()> {
        self.write_file("power_dpm_force_performance_level", level.to_string())
    }

    /// Retuns the list of power levels and index of the currently active level for a given kind of power state.
    pub fn get_power_levels(&self, kind: PowerStateKind) -> Result<(Vec<String>, u8)> {
        self.read_file(kind.to_filename()).and_then(|content| {
            let mut power_levels = Vec::new();
            let mut active = 0;

            for mut line in content.trim().split('\n') {
                if let Some(stripped) = line.strip_suffix('*') {
                    line = stripped;

                    if let Some(identifier) = stripped.split(':').next() {
                        active = identifier
                            .trim()
                            .parse()
                            .context("Unexpected power level identifier")?;
                    }
                }
                if let Some(s) = line.split(':').last() {
                    power_levels.push(s.trim().to_string());
                }
            }

            Ok((power_levels, active))
        })
    }

    /// Sets the enabled power levels for a power state kind to a given list of levels. This means that only the given power levels will be allowed.
    ///
    /// Can only be used if `power_force_performance_level` is set to `manual`.
    pub fn set_enabled_power_levels(&self, kind: PowerStateKind, levels: &[u8]) -> Result<()> {
        match self.get_power_force_performance_level()? {
            PerformanceLevel::Manual => {
                let mut s = String::new();

                for l in levels {
                    s.push(char::from_digit((*l).into(), 10).unwrap());
                    s.push(' ');
                }

                Ok(self.write_file(kind.to_filename(), s)?)
            }
            _ => Err(ErrorKind::NotAllowed(
                "power_force_performance level needs to be set to 'manual' to adjust power levels"
                    .to_string(),
            )
            .into()),
        }
    }
}

impl SysFS for GpuHandle {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

impl FromStr for PerformanceLevel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "auto" | "Automatic" => Ok(PerformanceLevel::Auto),
            "high" | "Highest Clocks" => Ok(PerformanceLevel::High),
            "low" | "Lowest Clocks" => Ok(PerformanceLevel::Low),
            "manual" | "Manual" => Ok(PerformanceLevel::Manual),
            _ => Err(ErrorKind::ParseError {
                msg: "unrecognized GPU power profile".to_string(),
                line: 1,
            }
            .into()),
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
