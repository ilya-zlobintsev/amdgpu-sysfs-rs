//! `pp-power-profile-mode`

use super::trim_sysfs_line;
use crate::{error::Error, Result};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Table of predefined power profile modes

/// https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#pp-power-profile-mode
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PowerProfileModesTable {
    /// List of available modes
    pub modes: BTreeMap<u16, String>,
    /// The currently active mode
    pub active: u16,
}

impl PowerProfileModesTable {
    /// Parse the table from a given string
    pub fn parse(s: &str) -> Result<Self> {
        let mut modes = BTreeMap::new();
        let mut active = None;

        for (line, row) in s.lines().map(trim_sysfs_line).enumerate() {
            let mut parts = row.split_whitespace();

            if let Some(num) = parts.next().and_then(|part| part.parse::<u16>().ok()) {
                let mut name = parts
                    .next()
                    .ok_or_else(|| Error::unexpected_eol("No name after mode number", line))?
                    .trim_matches(':');

                if let Some(stripped_name) = name.strip_suffix('*') {
                    name = stripped_name;
                    active = Some(num);
                }

                modes.insert(num, name.to_owned());

                // To handle cases that look like this:
                // 1 3D_FULL_SCREEN *:
                if active.is_none() {
                    if let Some(part) = parts.next() {
                        if part.starts_with('*') {
                            active = Some(num);
                        }
                    }
                }
            }
        }

        Ok(Self {
            modes,
            active: active.ok_or_else(|| Error::basic_parse_error("No active level found"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::PowerProfileModesTable;
    use insta::assert_yaml_snapshot;

    const TABLE_VEGA56: &str = include_test_data!("vega56/pp_power_profile_mode");
    const TABLE_RX580: &str = include_test_data!("rx580/pp_power_profile_mode");
    const TABLE_4800H: &str = include_test_data!("internal-4800h/pp_power_profile_mode");
    const TABLE_RX6900XT: &str = include_test_data!("rx6900xt/pp_power_profile_mode");

    #[test]
    fn parse_full_vega56() {
        let table = PowerProfileModesTable::parse(TABLE_VEGA56).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn parse_full_rx580() {
        let table = PowerProfileModesTable::parse(TABLE_RX580).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn parse_full_internal_4800h() {
        let table = PowerProfileModesTable::parse(TABLE_4800H).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn parse_full_rx6900xt() {
        let table = PowerProfileModesTable::parse(TABLE_RX6900XT).unwrap();
        assert_yaml_snapshot!(table);
    }
}
