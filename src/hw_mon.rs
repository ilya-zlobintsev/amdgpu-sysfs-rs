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
