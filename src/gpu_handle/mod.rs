//! Handle on a GPU
#[cfg(feature = "overdrive")]
pub mod overdrive;
#[macro_use]
mod power_levels;
pub mod fan_control;
pub mod power_profile_mode;

pub use power_levels::{PowerLevelKind, PowerLevels};

use self::fan_control::{FanCurve, FanCurveRanges, FanInfo};
use crate::{
    error::{Error, ErrorContext, ErrorKind},
    gpu_handle::fan_control::FanCtrlContents,
    hw_mon::HwMon,
    sysfs::SysFS,
    Result,
};
use power_profile_mode::PowerProfileModesTable;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Display, Write as _},
    fs,
    io::Write,
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

        let uevent_raw = fs::read_to_string(sysfs_path.join("uevent"))?.replace(char::from(0), "");

        let mut uevent = HashMap::new();

        for (i, line) in uevent_raw.trim().lines().enumerate() {
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

    fn get_link(&self, file_name: &str) -> Result<String> {
        // Despite being labled NAVI10, newer generations use the same port device ids
        const NAVI10_UPSTREAM_PORT: &str = "0x1478\n";
        const NAVI10_DOWNSTREAM_PORT: &str = "0x1479\n";

        let mut sysfs_path = std::fs::canonicalize(self.get_path())?.join("../"); // pcie port

        for _ in 0..2 {
            let Ok(did) = std::fs::read_to_string(sysfs_path.join("device")) else {
                break;
            };

            if did == NAVI10_UPSTREAM_PORT || did == NAVI10_DOWNSTREAM_PORT {
                sysfs_path.push("../");
            } else {
                break;
            }
        }

        sysfs_path.pop();

        Self {
            sysfs_path,
            hw_monitors: Vec::new(),
            uevent: HashMap::new(),
        }
        .read_file(file_name)
    }

    /// Gets the current PCIe link speed.
    pub fn get_current_link_speed(&self) -> Result<String> {
        self.get_link("current_link_speed")
    }

    /// Gets the current PCIe link width.
    pub fn get_current_link_width(&self) -> Result<String> {
        self.get_link("current_link_width")
    }

    /// Gets the maximum possible PCIe link speed.
    pub fn get_max_link_speed(&self) -> Result<String> {
        self.get_link("max_link_speed")
    }

    /// Gets the maximum possible PCIe link width.
    pub fn get_max_link_width(&self) -> Result<String> {
        self.get_link("max_link_width")
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
            let mut invalid_active = false;

            for mut line in content.trim().split('\n') {
                if let Some(stripped) = line.strip_suffix('*') {
                    line = stripped;

                    if let Some(identifier) = stripped.split(':').next() {
                        if !invalid_active {
                            if active.is_some() {
                                active = None;
                                invalid_active = true;
                            } else {
                                let idx = identifier
                                    .trim()
                                    .parse()
                                    .context("Unexpected power level identifier")?;
                                active = Some(idx);
                            }
                        }
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
    pub fn set_clocks_table(&self, new_table: &ClocksTableGen) -> Result<CommitHandle> {
        let old_table = self.get_clocks_table()?;

        let path = self.sysfs_path.join("pp_od_clk_voltage");
        let mut file = File::create(&path)?;

        new_table.write_commands(&mut file, &old_table)?;

        Ok(CommitHandle::new(path))
    }

    /// Resets the clocks table to the default configuration.
    #[cfg(feature = "overdrive")]
    pub fn reset_clocks_table(&self) -> Result<()> {
        let path = self.sysfs_path.join("pp_od_clk_voltage");
        let mut file = File::create(path)?;
        file.write_all(b"r\n")?;

        Ok(())
    }

    /// Reads the list of predefined power profiles and the relevant heuristics settings for them from `pp_power_profile_mode`
    ///
    /// https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#pp-power-profile-mode
    pub fn get_power_profile_modes(&self) -> Result<PowerProfileModesTable> {
        let contents = self.read_file("pp_power_profile_mode")?;
        PowerProfileModesTable::parse(&contents)
    }

    /// Sets the current power profile mode. You can get the available modes with [`get_power_profile_modes`].
    /// Requires the performance level to be set to "manual" first using [`set_power_force_performance_level`]
    pub fn set_active_power_profile_mode(&self, i: u16) -> Result<()> {
        self.write_file("pp_power_profile_mode", format!("{i}\n"))
    }

    /// Sets a custom power profile mode. You can get the available modes, and the list of heuristic names with [`get_power_profile_modes`].
    /// Requires the performance level to be set to "manual" first using [`set_power_force_performance_level`]
    pub fn set_custom_power_profile_mode_heuristics(
        &self,
        components: &[Vec<Option<i32>>],
    ) -> Result<()> {
        let table = self.get_power_profile_modes()?;
        let (index, current_custom_profile) = table
            .modes
            .iter()
            .find(|(_, profile)| profile.is_custom())
            .ok_or_else(|| {
                ErrorKind::NotAllowed("Could not find a custom power profile".to_owned())
            })?;

        if current_custom_profile.components.len() != components.len() {
            return Err(ErrorKind::NotAllowed(format!(
                "Expected {} power profile components, got {}",
                current_custom_profile.components.len(),
                components.len()
            ))
            .into());
        }

        if current_custom_profile.components.len() == 1 {
            let mut values_command = format!("{index}");
            for heuristic in &components[0] {
                match heuristic {
                    Some(value) => write!(values_command, " {value}").unwrap(),
                    None => write!(values_command, " -").unwrap(),
                }
            }

            values_command.push('\n');
            self.write_file("pp_power_profile_mode", values_command)
        } else {
            for (component_index, heuristics) in components.iter().enumerate() {
                let mut values_command = format!("{index} {component_index}");
                for heuristic in heuristics {
                    match heuristic {
                        Some(value) => write!(values_command, " {value}").unwrap(),
                        None => write!(values_command, " -").unwrap(),
                    }
                }
                values_command.push('\n');

                self.write_file("pp_power_profile_mode", values_command)?;
            }

            Ok(())
        }
    }

    fn read_fan_info(&self, file: &str, section_name: &str, range_name: &str) -> Result<FanInfo> {
        let file_path = self.get_path().join("gpu_od/fan_ctrl").join(file);
        let data = self.read_file(file_path)?;
        let contents = FanCtrlContents::parse(&data, section_name)?;

        let current = contents.contents.parse()?;

        let allowed_range = match contents.od_range.get(range_name) {
            Some((raw_min, raw_max)) => {
                let min = raw_min.parse()?;
                let max = raw_max.parse()?;
                Some((min, max))
            }
            None => None,
        };

        Ok(FanInfo {
            current,
            allowed_range,
        })
    }

    /// Gets the fan acoustic limit. Values are in RPM.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#acoustic-limit-rpm-threshold>
    pub fn get_fan_acoustic_limit(&self) -> Result<FanInfo> {
        self.read_fan_info(
            "acoustic_limit_rpm_threshold",
            "OD_ACOUSTIC_LIMIT",
            "ACOUSTIC_LIMIT",
        )
    }

    /// Gets the fan acoustic target. Values are in RPM.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#acoustic-target-rpm-threshold>
    pub fn get_fan_acoustic_target(&self) -> Result<FanInfo> {
        self.read_fan_info(
            "acoustic_target_rpm_threshold",
            "OD_ACOUSTIC_TARGET",
            "ACOUSTIC_TARGET",
        )
    }

    /// Gets the fan temperature target. Values are in degrees.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#fan-target-temperature>
    pub fn get_fan_target_temperature(&self) -> Result<FanInfo> {
        self.read_fan_info(
            "fan_target_temperature",
            "FAN_TARGET_TEMPERATURE",
            "TARGET_TEMPERATURE",
        )
    }

    /// Gets the fan minimum PWM. Values are in percentages.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#fan-minimum-pwm>
    pub fn get_fan_minimum_pwm(&self) -> Result<FanInfo> {
        self.read_fan_info("fan_minimum_pwm", "FAN_MINIMUM_PWM", "MINIMUM_PWM")
    }

    fn set_fan_value(
        &self,
        file: &str,
        value: u32,
        section_name: &str,
        range_name: &str,
    ) -> Result<CommitHandle> {
        let info = self.read_fan_info(file, section_name, range_name)?;
        match info.allowed_range {
            Some((min, max)) => {
                if !(min..=max).contains(&value) {
                    return Err(Error::not_allowed(format!(
                        "Value {value} is out of range, should be between {min} and {max}"
                    )));
                }

                let file_path = self.sysfs_path.join("gpu_od/fan_ctrl").join(file);
                std::fs::write(&file_path, format!("{value}\n"))?;

                Ok(CommitHandle::new(file_path))
            }
            None => Err(Error::not_allowed(format!(
                "Changes to {range_name} are not allowed"
            ))),
        }
    }

    /// Sets the fan acoustic limit. Value is in RPM.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#acoustic-limit-rpm-threshold>
    pub fn set_fan_acoustic_limit(&self, value: u32) -> Result<CommitHandle> {
        self.set_fan_value(
            "acoustic_limit_rpm_threshold",
            value,
            "OD_ACOUSTIC_LIMIT",
            "ACOUSTIC_LIMIT",
        )
    }

    /// Sets the fan acoustic target. Value is in RPM.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#acoustic-target-rpm-threshold>
    pub fn set_fan_acoustic_target(&self, value: u32) -> Result<CommitHandle> {
        self.set_fan_value(
            "acoustic_target_rpm_threshold",
            value,
            "OD_ACOUSTIC_TARGET",
            "ACOUSTIC_TARGET",
        )
    }

    /// Sets the fan temperature target. Value is in degrees.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#fan-target-temperature>
    pub fn set_fan_target_temperature(&self, value: u32) -> Result<CommitHandle> {
        self.set_fan_value(
            "fan_target_temperature",
            value,
            "FAN_TARGET_TEMPERATURE",
            "TARGET_TEMPERATURE",
        )
    }

    /// Sets the fan minimum PWM. Value is a percentage.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#fan-minimum-pwm>
    pub fn set_fan_minimum_pwm(&self, value: u32) -> Result<CommitHandle> {
        self.set_fan_value("fan_minimum_pwm", value, "FAN_MINIMUM_PWM", "MINIMUM_PWM")
    }

    fn reset_fan_value(&self, file: &str) -> Result<()> {
        let file_path = self.sysfs_path.join("gpu_od/fan_ctrl").join(file);
        let mut file = File::create(file_path)?;
        writeln!(file, "r")?;
        Ok(())
    }

    /// Resets the fan acoustic limit.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#acoustic-limit-rpm-threshold>
    pub fn reset_fan_acoustic_limit(&self) -> Result<()> {
        self.reset_fan_value("acoustic_limit_rpm_threshold")
    }

    /// Resets the fan acoustic target.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#acoustic-target-rpm-threshold>
    pub fn reset_fan_acoustic_target(&self) -> Result<()> {
        self.reset_fan_value("acoustic_target_rpm_threshold")
    }

    /// Resets the fan target temperature.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#fan-target-temperature>
    pub fn reset_fan_target_temperature(&self) -> Result<()> {
        self.reset_fan_value("fan_target_temperature")
    }

    /// Resets the fan minimum pwm.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#fan-minimum-pwm>
    pub fn reset_fan_minimum_pwm(&self) -> Result<()> {
        self.reset_fan_value("fan_minimum_pwm")
    }

    /// Gets the PMFW (power management firmware) fan curve.
    /// Note: if no custom curve is used, all of the curve points may be set to 0.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// Older GPUs do not have a configurable fan curve in firmware, they need custom logic.
    pub fn get_fan_curve(&self) -> Result<FanCurve> {
        let data = self.read_file("gpu_od/fan_ctrl/fan_curve")?;
        let contents = FanCtrlContents::parse(&data, "OD_FAN_CURVE")?;
        let points = contents
            .contents
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let mut split = line.split(' ');
                split.next(); // Discard index

                let raw_temp = split
                    .next()
                    .ok_or_else(|| Error::unexpected_eol("Temperature value", i))?;
                let temp = raw_temp.trim_end_matches('C').parse()?;

                let raw_speed = split
                    .next()
                    .ok_or_else(|| Error::unexpected_eol("Speed value", i))?;
                let speed = raw_speed.trim_end_matches('%').parse()?;

                Ok((temp, speed))
            })
            .collect::<Result<_>>()?;

        let temp_range = contents.od_range.get("FAN_CURVE(hotspot temp)");
        let speed_range = contents.od_range.get("FAN_CURVE(fan speed)");

        let allowed_ranges = if let Some(((min_temp, max_temp), (min_speed, max_speed))) =
            (temp_range).zip(speed_range)
        {
            let min_temp: i32 = min_temp.trim_end_matches('C').parse()?;
            let max_temp: i32 = max_temp.trim_end_matches('C').parse()?;

            let min_speed: u8 = min_speed.trim_end_matches('%').parse()?;
            let max_speed: u8 = max_speed.trim_end_matches('%').parse()?;

            Some(FanCurveRanges {
                temperature_range: min_temp..=max_temp,
                speed_range: min_speed..=max_speed,
            })
        } else {
            None
        };

        Ok(FanCurve {
            points,
            allowed_ranges,
        })
    }

    /// Sets and applies the PMFW fan curve.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#fan-curve>
    pub fn set_fan_curve(&self, new_curve: &FanCurve) -> Result<CommitHandle> {
        let current_curve = self.get_fan_curve()?;
        let allowed_ranges = current_curve.allowed_ranges.ok_or_else(|| {
            Error::not_allowed("Changes to the fan curve are not supported".to_owned())
        })?;

        let file_path = self.sysfs_path.join("gpu_od/fan_ctrl/fan_curve");

        for (i, (temperature, speed)) in new_curve.points.iter().enumerate() {
            if !allowed_ranges.temperature_range.contains(temperature) {
                Err(Error::not_allowed(format!(
                    "Temperature value {temperature} is outside of the allowed range {:?}",
                    allowed_ranges.temperature_range
                )))?;
            }
            if !allowed_ranges.speed_range.contains(speed) {
                Err(Error::not_allowed(format!(
                    "Speed value {speed} is outside of the allowed range {:?}",
                    allowed_ranges.speed_range
                )))?;
            }

            std::fs::write(&file_path, format!("{i} {temperature} {speed}\n"))?;
        }

        Ok(CommitHandle::new(file_path))
    }

    /// Resets the PMFW fan curve.
    ///
    /// Only available on Navi3x (RDNA 3) or newer.
    /// <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#fan-curve>
    pub fn reset_fan_curve(&self) -> Result<()> {
        self.reset_fan_value("fan_curve")
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum PerformanceLevel {
    /// When auto is selected, the driver will attempt to dynamically select the optimal power profile for current conditions in the driver.
    #[default]
    Auto,
    /// When low is selected, the clocks are forced to the lowest power state.
    Low,
    /// When high is selected, the clocks are forced to the highest power state.
    High,
    /// When manual is selected, power states can be manually adjusted via `pp_dpm_*` files ([`GpuHandle::set_enabled_power_levels`]) and `pp_od_clk_voltage` ([`GpuHandle::set_clocks_table`]).
    Manual,
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

/// For some reason files sometimes have random null bytes around lines
fn trim_sysfs_line(line: &str) -> &str {
    line.trim_matches(char::from(0)).trim()
}

/// Handle for committing values which were previusly written
#[must_use]
#[derive(Debug)]
pub struct CommitHandle {
    file_path: PathBuf,
}

impl CommitHandle {
    pub(crate) fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    /// Commit the previously written values
    pub fn commit(self) -> Result<()> {
        std::fs::write(&self.file_path, "c\n").with_context(|| {
            format!(
                "Could not commit values to {:?}",
                self.file_path.file_name().unwrap()
            )
        })
    }
}
