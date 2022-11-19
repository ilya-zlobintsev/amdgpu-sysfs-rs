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

/// Reprepesents a hardware monitor.
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

        while let Ok(label) = self.read_file(&format!("temp{}_label", i)) {
            temps.insert(
                label,
                Temperature {
                    current: self.read_temp(&format!("temp{}_input", i)).ok(),
                    crit: self.read_temp(&format!("temp{}_crit", i)).ok(),
                    crit_hyst: self.read_temp(&format!("temp{}_crit_hyst", i)).ok(),
                },
            );

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

    /// Gets the current power cap of the GPU in watts.
    pub fn get_power_cap(&self) -> Result<f64> {
        self.read_power("power1_cap")
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
        self.write_file("fan1_target", &target.to_string())?;
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

    /// Sets the fan control method.
    pub fn set_fan_control_method(&self, method: FanControlMethod) -> Result<()> {
        let repr = method as u32;
        self.write_file("pwm1_enable", &repr.to_string())?;
        Ok(())
    }

    /// Gets the GPU voltage in millivolts.
    pub fn get_gpu_voltage(&self) -> Result<u64> {
        self.read_file_parsed("in0_input")
    }

    /// Gets the north bridge voltage in millivolts.
    pub fn get_northbirdge_voltage(&self) -> Result<u64> {
        self.read_file_parsed("in1_input")
    }
}

impl SysFS for HwMon {
    fn get_path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Temperature {
    pub current: Option<f32>,
    pub crit: Option<f32>,
    pub crit_hyst: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FanControlMethod {
    None = 0,
    Manual = 1,
    Auto = 2,
}

impl FanControlMethod {
    pub fn from_repr(repr: u32) -> Option<Self> {
        match repr {
            0 => Some(Self::None),
            1 => Some(Self::Manual),
            2 => Some(Self::Auto),
            _ => None,
        }
    }
}
