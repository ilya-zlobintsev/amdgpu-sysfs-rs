//! Hardware monitoring
use crate::{
    error::{ErrorContext, ErrorKind},
    sysfs::SysFS,
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// Represents a hardware monitor.
/// Hardware monitors are used to report real-time information about the device, such as temperatures and power usage.
#[derive(Clone, Debug)]
pub struct HwMon {
    path: PathBuf,
}

impl HwMon {
    /// Most of the time you may want to access `HwMon`s through the
    /// [GpuHandle](../gpu_handle/struct.GpuHandle.html) they're bound to.
    pub fn new_from_path(path: PathBuf) -> Result<Self> {
        let hw_mon = Self { path };
        hw_mon.read_file("name")?;
        Ok(hw_mon)
    }

    fn read_temp(&self, file: &str) -> Result<f32> {
        let temp_str = self.read_file(file)?;
        Ok(temp_str
            .trim()
            .parse::<f32>()
            .context("Invalid temperature value (driver bug?)")?
            / 1000.0)
    }

    /// Returns a HashMap of temperatures(in degress celsius), indexed by the labels (example: "edge").
    pub fn get_temps(&self) -> HashMap<String, Temperature> {
        let mut temps = HashMap::new();

        let mut i = 1;

        while let Ok(current) = self.read_temp(&format!("temp{i}_input")) {
            let temperature = Temperature {
                current: Some(current),
                crit: self.read_temp(&format!("temp{i}_crit")).ok(),
                crit_hyst: self.read_temp(&format!("temp{i}_crit_hyst")).ok(),
            };

            match self.read_file(format!("temp{i}_label")) {
                Ok(label) => {
                    temps.insert(label, temperature);
                }
                Err(_) => {
                    temps.insert(i.to_string(), temperature);
                    break;
                }
            }

            i += 1;
        }

        temps
    }

    fn read_clockspeed(&self, file: &str) -> Result<u64> {
        let raw_clockspeed = self.read_file(file)?;
        Ok(raw_clockspeed
            .parse::<u64>()
            .context("Unexpected GPU clockspeed (driver bug?)")?
            / 1000000)
    }

    /// Gets the current GFX/compute clockspeed in MHz.
    pub fn get_gpu_clockspeed(&self) -> Result<u64> {
        self.read_clockspeed("freq1_input")
    }

    /// Gets the current memory clockspeed in MHz.
    pub fn get_vram_clockspeed(&self) -> Result<u64> {
        self.read_clockspeed("freq2_input")
    }

    fn read_power(&self, file: &str) -> Result<f64> {
        let raw_power = self.read_file(file)?;
        Ok(raw_power
            .parse::<f64>()
            .context("Unexpected power value (driver bug?)")?
            / 1000000.0)
    }

    /// Gets the average power (currently) used by the GPU in watts.
    pub fn get_power_average(&self) -> Result<f64> {
        self.read_power("power1_average")
    }

    /// Gets the instantaneous power (currently) used by the GPU in watts.
    pub fn get_power_input(&self) -> Result<f64> {
        self.read_power("power1_input")
    }

    /// Gets the current power cap of the GPU in watts.
    pub fn get_power_cap(&self) -> Result<f64> {
        self.read_power("power1_cap")
    }

    /// Sets the current power cap of the GPU in watts.
    pub fn set_power_cap(&self, cap: f64) -> Result<()> {
        let value = (cap * 1000000.0).round() as i64;
        self.write_file("power1_cap", value.to_string())
    }

    /// Gets the maximum possible power cap for the GPU in watts. If overclocking is disabled, this is probably the same as the default cap.
    pub fn get_power_cap_max(&self) -> Result<f64> {
        self.read_power("power1_cap_max")
    }

    /// Gets the minimum possible power cap for the GPU in watts.
    pub fn get_power_cap_min(&self) -> Result<f64> {
        self.read_power("power1_cap_min")
    }

    /// Gets the default power cap for the GPU in watts.
    pub fn get_power_cap_default(&self) -> Result<f64> {
        self.read_power("power1_cap_default")
    }

    /// Gets the pulse width modulation fan level.
    pub fn get_fan_pwm(&self) -> Result<u8> {
        let pwm = self.read_file("pwm1")?;
        pwm.parse().context("Unexpected PWM (driver bug?)")
    }

    /// Gets the minimum pulse width modulation fan level.
    pub fn get_fan_min_pwm(&self) -> Result<u8> {
        let pwm = self.read_file("pwm1_min")?;
        pwm.parse().context("Unexpected PWM (driver bug?)")
    }

    /// Gets the maximum pulse width modulation fan level.
    pub fn get_fan_max_pwm(&self) -> Result<u8> {
        let pwm = self.read_file("pwm1_max")?;
        pwm.parse().context("Unexpected PWM (driver bug?)")
    }

    /// Sets the pulse width modulation fan level.
    pub fn set_fan_pwm(&self, pwm: u8) -> Result<()> {
        self.write_file("pwm1", pwm.to_string())
    }

    /// Gets the current fan speed in RPM.
    pub fn get_fan_current(&self) -> Result<u32> {
        let s = self.read_file("fan1_input")?;
        s.parse().context("Unexpected fan1_input (driver bug?)")
    }

    /// Gets the maximum possible fan speed in RPM.
    pub fn get_fan_max(&self) -> Result<u32> {
        let s = self.read_file("fan1_max")?;
        s.parse().context("Unexpected fan1_max (driver bug?)")
    }

    /// Gets the minimum possible fan speed in RPM.
    pub fn get_fan_min(&self) -> Result<u32> {
        let s = self.read_file("fan1_min")?;
        s.parse().context("Unexpected fan1_min (driver bug?)")
    }

    /// Gets the currently desired fan speed in RPM.
    pub fn get_fan_target(&self) -> Result<u32> {
        self.read_file("fan1_target")
            .map(|s| s.parse().expect("Unexpected fan1_target (driver bug?)"))
    }

    /// Sets the desired fan speed in RPM.
    pub fn set_fan_target(&self, target: u32) -> Result<()> {
        self.write_file("fan1_target", target.to_string())?;
        Ok(())
    }

    /// Gets the pulse width modulation control method.
    pub fn get_fan_control_method(&self) -> Result<FanControlMethod> {
        self.read_file("pwm1_enable").and_then(|pwm1_enable| {
            let repr = pwm1_enable
                .parse()
                .context("Unexpected pwm1_enable (driver bug?)")?;
            FanControlMethod::from_repr(repr).ok_or_else(|| {
                ErrorKind::Unsupported(
                    "Unexpected pwm1_enable (driver bug or unsupported?)".to_owned(),
                )
                .into()
            })
        })
    }

    /// Sets the fan control method (`pwm1_enable`).
    pub fn set_fan_control_method(&self, method: FanControlMethod) -> Result<()> {
        let repr = method as u32;
        self.write_file("pwm1_enable", repr.to_string())
    }

    /// Gets the GPU voltage in millivolts.
    pub fn get_gpu_voltage(&self) -> Result<u64> {
        self.read_file_parsed("in0_input")
    }

    /// Gets the north bridge voltage in millivolts.
    pub fn get_northbridge_voltage(&self) -> Result<u64> {
        self.read_file_parsed("in1_input")
    }
}

impl SysFS for HwMon {
    fn get_path(&self) -> &Path {
        &self.path
    }
}

/// Temperature reported by the GPU.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Temperature {
    /// The current temperature.
    pub current: Option<f32>,
    /// The maximum allowed temperature.
    pub crit: Option<f32>,
    /// The minimum allowed temperature.
    pub crit_hyst: Option<f32>,
}

/// The way the fan speed is controlled.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum FanControlMethod {
    /// No fan speed control.
    None = 0,
    /// Manual fan speed control via the PWM interface.
    Manual = 1,
    /// Automatic fan speed control (by the kernel).
    Auto = 2,
}

impl FanControlMethod {
    /// Create [FanControlMethod] from a digit in the SysFS.
    pub fn from_repr(repr: u32) -> Option<Self> {
        match repr {
            0 => Some(Self::None),
            1 => Some(Self::Manual),
            2 => Some(Self::Auto),
            _ => None,
        }
    }
}
