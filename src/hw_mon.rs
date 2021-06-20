use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::sysfs::SysFS;

/// Reprepesents a hardware monitor.
/// Hardware monitors are used to report real-time information about the device, such as temperatures and power usage.
#[derive(Debug)]
pub struct HwMon {
    path: PathBuf,
}

impl HwMon {
    /// Most of the time you may want to access `HwMon`s through the
    /// [GpuController](../gpu_controller/struct.GpuController.html) they're bound to.
    pub fn new_from_path(path: PathBuf) -> Result<Self, HwMonError> {
        let hw_mon = Self { path };

        match hw_mon.read_file("name") {
            Some(_) => Ok(hw_mon),
            None => Err(HwMonError::InvalidSysFS),
        }
    }

    /// Returns a HashMap of temperatures(in degress celsius), indexed by the labels (example: "edge").
    pub fn get_temps(&self) -> HashMap<String, Temperature> {
        let mut temps = HashMap::new();

        let mut i = 1;

        while let Some(label) = self.read_file(&format!("temp{}_label", i)) {
            temps.insert(
                label,
                Temperature {
                    current: self
                        .read_file(&format!("temp{}_input", i))
                        .unwrap_or_default()
                        .parse::<f32>()
                        .expect("Invalid temperature (driver bug?)")
                        / 1000.0,

                    crit: self
                        .read_file(&format!("temp{}_crit", i))
                        .unwrap_or_default()
                        .parse::<f32>()
                        .expect("Invalid temperature (driver bug?)")
                        / 1000.0,

                    crit_hyst: self
                        .read_file(&format!("temp{}_crit_hyst", i))
                        .unwrap_or_default()
                        .parse::<f32>()
                        .expect("Invalid temperature (driver bug?)")
                        / 1000.0,
                },
            );

            i += 1;
        }

        temps
    }

    /// Gets the current GFX/compute clockspeed in MHz.
    pub fn get_gpu_clockspeed(&self) -> Option<u64> {
        self.read_file("freq1_input").map(|f| {
            f.parse::<u64>()
                .expect("Unexpected GPU clockspeed (driver bug?)")
                / 1000000
        })
    }

    /// Gets the current memory clockspeed in MHz.
    pub fn get_vram_clockspeed(&self) -> Option<u64> {
        self.read_file("freq2_input").map(|f| {
            f.parse::<u64>()
                .expect("Unexpected VRAM clockspeed (driver bug?)")
                / 1000000
        })
    }

    /// Gets the average power (currently) used by the GPU in watts.
    pub fn get_power_average(&self) -> Option<f64> {
        self.read_file("power1_average").map(|p| {
            p.parse::<f64>()
                .expect("Unexpected power usage (driver bug?)")
                / 1000000.0
        })
    }

    /// Gets the current power cap of the GPU in watts.
    pub fn get_power_cap(&self) -> Option<f64> {
        self.read_file("power1_cap").map(|p| {
            p.parse::<f64>()
                .expect("Unexpected power usage (driver bug?)")
                / 1000000.0
        })
    }

    /// Gets the maximum possible power cap for the GPU in watts. If overclocking is disabled, this is probably the same as the default cap.
    pub fn get_power_cap_max(&self) -> Option<f64> {
        self.read_file("power1_cap_max").map(|p| {
            p.parse::<f64>()
                .expect("Unexpected power usage (driver bug?)")
                / 1000000.0
        })
    }

    /// Gets the minimum possible power cap for the GPU in watts.
    pub fn get_power_cap_min(&self) -> Option<f64> {
        self.read_file("power1_cap_min").map(|p| {
            p.parse::<f64>()
                .expect("Unexpected power usage (driver bug?)")
                / 1000000.0
        })
    }

    /// Gets the pulse width modulation fan level.
    pub fn get_fan_pwm(&self) -> Option<u8> {
        self.read_file("pwm1")
            .map(|pwm| pwm.parse().expect("Unexpected PWM (driver bug?)"))
    }

    /// Gets the current fan speed in RPM.
    pub fn get_fan_current(&self) -> Option<u32> {
        self.read_file("fan1_input").map(|s| {
            s
                .parse()
                .expect("Unexpected fan1_input (driver bug?)")
        })
    }
    
    /// Gets the maximum possible fan speed in RPM.
    pub fn get_fan_max(&self) -> Option<u32> {
        self.read_file("fan1_max").map(|s| {
            s
                .parse()
                .expect("Unexpected fan1_max (driver bug?)")
        })
    }

    /// Gets the minimum possible fan speed in RPM.
    pub fn get_fan_min(&self) -> Option<u32> {
        self.read_file("fan1_min").map(|s| {
            s
                .parse()
                .expect("Unexpected fan1_min (driver bug?)")
        })
    }
    
    /// Gets the currently desired fan speed in RPM.
    pub fn get_fan_target(&self) -> Option<u32> {
        self.read_file("fan1_target").map(|s| {
            s
                .parse()
                .expect("Unexpected fan1_target (driver bug?)")
        })
    }

    /// Gets the pulse width modulation control method.
    pub fn get_fan_control_method(&self) -> Option<FanControlMethod> {
        self.read_file("pwm1_enable").map(|pwm1_enable| {
            FanControlMethod::from_enable(
                pwm1_enable
                    .parse()
                    .expect("Unexpected pwm1_enable (driver bug?)"),
            )
            .expect("Unexpected pwm1_enable (driver bug or unsupported?)")
        })
    }
}

impl SysFS for HwMon {
    fn get_path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone)]
pub struct Temperature {
    pub current: f32,
    pub crit: f32,
    pub crit_hyst: f32,
}

#[derive(Debug)]
pub enum HwMonError {
    InvalidSysFS,
    InvalidValue,
}

pub enum FanControlMethod {
    None,
    Auto,
    Manual,
}

impl FanControlMethod {
    pub fn from_enable(enable_value: u8) -> Result<Self, HwMonError> {
        match enable_value {
            0 => Ok(Self::None),
            1 => Ok(Self::Manual),
            2 => Ok(Self::Auto),
            _ => Err(HwMonError::InvalidValue),
        }
    }
}
