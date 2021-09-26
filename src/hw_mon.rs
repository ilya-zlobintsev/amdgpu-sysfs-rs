use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::sysfs::SysFS;
use serde::{Serialize, Deserialize};

/// Reprepesents a hardware monitor.
/// Hardware monitors are used to report real-time information about the device, such as temperatures and power usage.
#[derive(Clone, Debug)]
pub struct HwMon {
    path: PathBuf,
}

impl HwMon {
    /// Most of the time you may want to access `HwMon`s through the
    /// [GpuController](../gpu_controller/struct.GpuController.html) they're bound to.
    pub async fn new_from_path(path: PathBuf) -> Result<Self, HwMonError> {
        let hw_mon = Self { path };

        match hw_mon.read_file("name").await {
            Some(_) => Ok(hw_mon),
            None => Err(HwMonError::InvalidSysFS),
        }
    }

    async fn read_temp(&self, file: &str) -> Option<f32> {
        self.read_file(file).await.map(|temp_str| {
            temp_str
                .trim()
                .parse::<f32>()
                .expect("Invalid temperature value (driver bug?)")
                / 1000.0
        })
    }

    /// Returns a HashMap of temperatures(in degress celsius), indexed by the labels (example: "edge").
    pub async fn get_temps(&self) -> HashMap<String, Temperature> {
        let mut temps = HashMap::new();

        let mut i = 1;

        while let Some(label) = self.read_file(&format!("temp{}_label", i)).await {
            temps.insert(
                label,
                Temperature {
                    current: self.read_temp(&format!("temp{}_input", i)).await,

                    crit: self.read_temp(&format!("temp{}_crit", i)).await,

                    crit_hyst: self.read_temp(&format!("temp{}_crit_hyst", i)).await,
                },
            );

            i += 1;
        }

        temps
    }

    async fn read_clockspeed(&self, file: &str) -> Option<u64> {
        self.read_file(file).await.map(|f| {
            f.parse::<u64>()
                .expect("Unexpected GPU clockspeed (driver bug?)")
                / 1000000
        })
    }

    /// Gets the current GFX/compute clockspeed in MHz.
    pub async fn get_gpu_clockspeed(&self) -> Option<u64> {
        self.read_clockspeed("freq1_input").await
    }

    /// Gets the current memory clockspeed in MHz.
    pub async fn get_vram_clockspeed(&self) -> Option<u64> {
        self.read_clockspeed("freq2_input").await
    }

    async fn read_power(&self, file: &str) -> Option<f64> {
        self.read_file(file).await.map(|p| {
            p.parse::<f64>()
                .expect("Unexpected power value (driver bug?)")
                / 1000000.0
        })
    }

    /// Gets the average power (currently) used by the GPU in watts.
    pub async fn get_power_average(&self) -> Option<f64> {
        self.read_power("power1_average").await
    }

    /// Gets the current power cap of the GPU in watts.
    pub async fn get_power_cap(&self) -> Option<f64> {
        self.read_power("power1_cap").await
    }

    /// Gets the maximum possible power cap for the GPU in watts. If overclocking is disabled, this is probably the same as the default cap.
    pub async fn get_power_cap_max(&self) -> Option<f64> {
        self.read_power("power1_cap_max").await
    }

    /// Gets the minimum possible power cap for the GPU in watts.
    pub async fn get_power_cap_min(&self) -> Option<f64> {
        self.read_power("power1_cap_min").await
    }

    /// Gets the default power cap for the GPU in watts.
    pub async fn get_power_cap_default(&self) -> Option<f64> {
        self.read_power("power1_cap_default").await
    }

    /// Gets the pulse width modulation fan level.
    pub async fn get_fan_pwm(&self) -> Option<u8> {
        self.read_file("pwm1")
            .await
            .map(|pwm| pwm.parse().expect("Unexpected PWM (driver bug?)"))
    }

    /// Gets the current fan speed in RPM.
    pub async fn get_fan_current(&self) -> Option<u32> {
        self.read_file("fan1_input")
            .await
            .map(|s| s.parse().expect("Unexpected fan1_input (driver bug?)"))
    }

    /// Gets the maximum possible fan speed in RPM.
    pub async fn get_fan_max(&self) -> Option<u32> {
        self.read_file("fan1_max")
            .await
            .map(|s| s.parse().expect("Unexpected fan1_max (driver bug?)"))
    }

    /// Gets the minimum possible fan speed in RPM.
    pub async fn get_fan_min(&self) -> Option<u32> {
        self.read_file("fan1_min")
            .await
            .map(|s| s.parse().expect("Unexpected fan1_min (driver bug?)"))
    }

    /// Gets the currently desired fan speed in RPM.
    pub async fn get_fan_target(&self) -> Option<u32> {
        self.read_file("fan1_target")
            .await
            .map(|s| s.parse().expect("Unexpected fan1_target (driver bug?)"))
    }

    /// Gets the pulse width modulation control method.
    pub async fn get_fan_control_method(&self) -> Option<FanControlMethod> {
        self.read_file("pwm1_enable").await.map(|pwm1_enable| {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Temperature {
    pub current: Option<f32>,
    pub crit: Option<f32>,
    pub crit_hyst: Option<f32>,
}

#[derive(Debug)]
pub enum HwMonError {
    InvalidSysFS,
    InvalidValue,
}

#[derive(Clone, Serialize, Deserialize)]
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
