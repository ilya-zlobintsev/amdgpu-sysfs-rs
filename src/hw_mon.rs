use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::sysfs::SysFS;

#[derive(Debug)]
pub struct HwMon {
    path: PathBuf,
}

impl HwMon {
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
}
