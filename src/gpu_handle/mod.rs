//! Handle on a GPU
#[cfg(feature = "overdrive")]
pub mod overdrive;
#[macro_use]
mod power_levels;

pub use power_levels::{PowerLevelKind, PowerLevels};

use crate::{
    error::{Error, ErrorContext, ErrorKind},
    hw_mon::HwMon,
    sysfs::SysFS,
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Display},
    fs,
    path::PathBuf,
    str::FromStr,
};
#[cfg(feature = "overdrive")]
use {
    self::overdrive::{ClocksTable, ClocksTableGen},
    std::fs::File,
};

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

        if let Ok(hw_mons_iter) = fs::read_dir(sysfs_path.join("hwmon")) {
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

    /// Gets the pci slot name of the card.
    pub fn get_pci_slot_name(&self) -> Option<&str> {
        self.uevent.get("PCI_SLOT_NAME").map(|s| s.as_str())
    }

    /// Gets the current PCIe link speed.
    pub fn get_current_link_speed(&self) -> Result<String> {
        self.read_file("current_link_speed")
    }

    /// Gets the current PCIe link width.
    pub fn get_current_link_width(&self) -> Result<String> {
        self.read_file("current_link_width")
    }

    /// Gets the maximum possible PCIe link speed.
    pub fn get_max_link_speed(&self) -> Result<String> {
        self.read_file("max_link_speed")
    }

    /// Gets the maximum possible PCIe link width.
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

    /// Returns the GPU VBIOS version.
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
    /// `T` is the type that values should be deserialized into.
    pub fn get_clock_levels<T>(&self, kind: PowerLevelKind) -> Result<PowerLevels<T>>
    where
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        self.read_file(kind.filename()).and_then(|content| {
            let mut levels = Vec::new();
            let mut active = None;

            for mut line in content.trim().split('\n') {
                if let Some(stripped) = line.strip_suffix('*') {
                    line = stripped;

                    if let Some(identifier) = stripped.split(':').next() {
                        active = Some(
                            identifier
                                .trim()
                                .parse()
                                .context("Unexpected power level identifier")?,
                        );
                    }
                }
                if let Some(s) = line.split(':').last() {
                    let parse_result = if let Some(suffix) = kind.value_suffix() {
                        let raw_value = s.trim().to_lowercase();
                        let value = raw_value.strip_suffix(suffix).ok_or_else(|| {
                            ErrorKind::ParseError {
                                msg: format!("Level did not have the expected suffix {suffix}"),
                                line: levels.len() + 1,
                            }
                        })?;
                        T::from_str(value)
                    } else {
                        let value = s.trim();
                        T::from_str(value)
                    };

                    let parsed_value = parse_result.map_err(|err| ErrorKind::ParseError {
                        msg: format!("Could not deserialize power level value: {err}"),
                        line: levels.len() + 1,
                    })?;
                    levels.push(parsed_value);
                }
            }

            Ok(PowerLevels { levels, active })
        })
    }

    impl_get_clocks_levels!(get_core_clock_levels, PowerLevelKind::CoreClock, u64);
    impl_get_clocks_levels!(get_memory_clock_levels, PowerLevelKind::MemoryClock, u64);
    impl_get_clocks_levels!(get_pcie_clock_levels, PowerLevelKind::PcieSpeed, String);

    /// Sets the enabled power levels for a power state kind to a given list of levels. This means that only the given power levels will be allowed.
    ///
    /// Can only be used if `power_force_performance_level` is set to `manual`.
    pub fn set_enabled_power_levels(&self, kind: PowerLevelKind, levels: &[u8]) -> Result<()> {
        match self.get_power_force_performance_level()? {
            PerformanceLevel::Manual => {
                let mut s = String::new();

                for l in levels {
                    s.push(char::from_digit((*l).into(), 10).unwrap());
                    s.push(' ');
                }

                Ok(self.write_file(kind.filename(), s)?)
            }
            _ => Err(ErrorKind::NotAllowed(
                "power_force_performance level needs to be set to 'manual' to adjust power levels"
                    .to_string(),
            )
            .into()),
        }
    }

    /// Reads the clocks table from `pp_od_clk_voltage`.
    #[cfg(feature = "overdrive")]
    pub fn get_clocks_table(&self) -> Result<ClocksTableGen> {
        self.read_file_parsed("pp_od_clk_voltage")
    }

    /// Writes and commits the given clocks table to `pp_od_clk_voltage`.
    #[cfg(feature = "overdrive")]
    pub fn set_clocks_table(&self, table: &ClocksTableGen) -> Result<()> {
        use std::io::Write;

        let path = self.sysfs_path.join("pp_od_clk_voltage");
        let mut file = File::create(path)?;

        table.write_commands(&mut file)?;
        file.write_all(b"c\n")?;

        Ok(())
    }

    /// Resets the clocks table to the default configuration.
    #[cfg(feature = "overdrive")]
    pub fn reset_clocks_table(&self) -> Result<()> {
        use std::io::Write;

        let path = self.sysfs_path.join("pp_od_clk_voltage");
        let mut file = File::open(path)?;
        file.write_all(b"r\n")?;

        Ok(())
    }
}

impl SysFS for GpuHandle {
    fn get_path(&self) -> &std::path::Path {
        &self.sysfs_path
    }
}

/// Performance level to be used by the GPU.
///
/// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#pp-od-clk-voltage>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum PerformanceLevel {
    /// When auto is selected, the driver will attempt to dynamically select the optimal power profile for current conditions in the driver.
    Auto,
    /// When low is selected, the clocks are forced to the lowest power state.
    Low,
    /// When high is selected, the clocks are forced to the highest power state.
    High,
    /// When manual is selected, power states can be manually adjusted via `pp_dpm_*` files ([`GpuHandle::set_enabled_power_levels`]) and `pp_od_clk_voltage` ([`GpuHandle::set_clocks_table`]).
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
