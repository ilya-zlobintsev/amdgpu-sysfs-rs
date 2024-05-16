//! `pp-power-profile-mode`
#![allow(missing_docs)] // temp
use crate::{
    error::{Error, ErrorKind},
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Table of predefined power profile modes

/// https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#pp-power-profile-mode
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PowerProfileModesTable {
    /// List of available modes
    pub modes: BTreeMap<u16, PowerProfile>,
    /// Names for the values in [`PowerProfile`]
    pub value_names: Vec<String>,
    /// The currently active mode
    pub active: u16,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PowerProfile {
    pub name: String,
    pub values: Vec<PowerProfileValues>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PowerProfileValues {
    pub clock_type: Option<String>,
    pub values: Vec<Option<i32>>,
}

impl PowerProfileModesTable {
    /// Parse the table from a given string
    pub fn parse(s: &str) -> Result<Self> {
        let mut split = s.split_whitespace();
        let start = split
            .next()
            .ok_or_else(|| Error::unexpected_eol("Value description", 1))?;

        match start {
            "NUM" => Self::parse_flat(s),
            "PROFILE_INDEX(NAME)" => todo!(),
            _ if start.parse::<u16>().is_ok() => Self::parse_basic(s),
            _ => Err(Error::basic_parse_error(
                "Could not determine the type of power profile mode table",
            )),
        }
    }

    /// Parse the format used by pre-RDNA GPUs
    fn parse_flat(s: &str) -> Result<Self> {
        let mut modes = BTreeMap::new();
        let mut active = None;

        let mut lines = s.lines();

        let header_line = lines
            .next()
            .ok_or_else(|| Error::unexpected_eol("Info header", 1))?;
        let mut header_split = header_line.split_whitespace();
        assert_eq!(Some("NUM"), header_split.next());
        assert_eq!(Some("MODE_NAME"), header_split.next());

        let value_names: Vec<String> = header_split.map(str::to_owned).collect();

        for (line, row) in s.lines().map(str::trim).enumerate() {
            let mut split = row.split_whitespace().peekable();
            if let Some(num) = split.next().and_then(|part| part.parse::<u16>().ok()) {
                let name_part = split
                    .next()
                    .ok_or_else(|| Error::unexpected_eol("Mode name", line + 1))?
                    .trim_end_matches(':');

                // Handle space within the mode name:
                // `3D_FULL_SCREEN *:`
                if let Some(next) = split.peek() {
                    if next.ends_with(':') {
                        if next.starts_with('*') {
                            active = Some(num);
                        }
                        split.next();
                    }
                }

                let name = if let Some(name) = name_part.strip_suffix('*') {
                    active = Some(num);
                    name.trim()
                } else {
                    name_part
                };

                let values = split
                    .map(|value| {
                        if value == "-" {
                            Ok(None)
                        } else {
                            let parsed = value.parse().map_err(|_| {
                                Error::from(ErrorKind::ParseError {
                                    msg: format!("Expected an integer, got '{value}'"),
                                    line: line + 1,
                                })
                            })?;
                            Ok(Some(parsed))
                        }
                    })
                    .collect::<Result<_>>()?;

                let power_profile = PowerProfile {
                    name: name.to_owned(),
                    values: vec![PowerProfileValues {
                        clock_type: None,
                        values,
                    }],
                };
                modes.insert(num, power_profile);
            }
        }

        Ok(Self {
            modes,
            value_names,
            active: active.ok_or_else(|| Error::basic_parse_error("No active level found"))?,
        })
    }

    /// Parse the format used by integrated GPUs
    fn parse_basic(s: &str) -> Result<Self> {
        let mut modes = BTreeMap::new();
        let mut active = None;

        for (line, row) in s.lines().map(str::trim).enumerate() {
            let mut split = row.split_whitespace();
            if let Some(num) = split.next().and_then(|part| part.parse::<u16>().ok()) {
                let name_part = split
                    .next()
                    .ok_or_else(|| Error::unexpected_eol("No name after mode number", line + 1))?;

                let name = if let Some(name) = name_part.strip_suffix('*') {
                    active = Some(num);
                    name
                } else {
                    name_part
                };

                modes.insert(
                    num,
                    PowerProfile {
                        name: name.to_owned(),
                        values: vec![],
                    },
                );
            }
        }

        Ok(Self {
            modes,
            value_names: vec![],
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
    const TABLE_RX7800XT: &str = include_test_data!("rx7800xt/pp_power_profile_mode");

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

    #[test]
    fn parse_full_rx7800xt() {
        let table = PowerProfileModesTable::parse(TABLE_RX7800XT).unwrap();
        assert_yaml_snapshot!(table);
    }
}
