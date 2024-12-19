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
    /// On RDNA and newer, each profile has multiple components for different clock types.
    /// Older generations have only one set of values.
    pub components: Vec<PowerProfileComponent>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize, Default, Clone))]
pub struct PowerProfileComponent {
    /// Filled on RDNA and newer
    pub clock_type: Option<String>,
    pub values: Vec<Option<i32>>,
}

impl PowerProfileModesTable {
    /// Parse the table from a given string
    pub fn parse(s: &str) -> Result<Self> {
        let mut lines = s.lines().map(|line| line.split_whitespace());

        let mut split = lines
            .next()
            .ok_or_else(|| Error::unexpected_eol("Power profile line", 1))?;
        let start = split
            .next()
            .ok_or_else(|| Error::unexpected_eol("Value description", 1))?;

        match start {
            "NUM" => Self::parse_flat(s),
            "PROFILE_INDEX(NAME)" => Self::parse_nested(s),
            _ if start.parse::<u16>().is_ok() => {
                if lines
                    .next()
                    .and_then(|mut line| line.next())
                    .is_some_and(|term| term.parse::<u16>().is_ok())
                {
                    Self::parse_basic(s)
                } else {
                    Self::parse_rotated(s)
                }
            }
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

        if header_split.next() != Some("NUM") {
            return Err(
                ErrorKind::Unsupported("Expected header to start with 'NUM'".to_owned()).into(),
            );
        }
        if header_split.next() != Some("MODE_NAME") {
            return Err(ErrorKind::Unsupported(
                "Expected header to contain 'MODE_NAME'".to_owned(),
            )
            .into());
        }

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
                    components: vec![PowerProfileComponent {
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

    /// Parse the format used by RDNA and higher
    fn parse_nested(s: &str) -> Result<Self> {
        let mut modes = BTreeMap::new();
        let mut active = None;

        let mut lines = s.lines();

        let header_line = lines
            .next()
            .ok_or_else(|| Error::unexpected_eol("Info header", 1))?;
        let mut header_split = header_line.split_whitespace();

        if header_split.next() != Some("PROFILE_INDEX(NAME)") {
            return Err(ErrorKind::Unsupported(
                "Expected header to start with 'PROFILE_INDEX(NAME)'".to_owned(),
            )
            .into());
        }
        if header_split.next() != Some("CLOCK_TYPE(NAME)") {
            return Err(ErrorKind::Unsupported(
                "Expected header to contain 'CLOCK_TYPE(NAME)'".to_owned(),
            )
            .into());
        }

        let value_names: Vec<String> = header_split.map(str::to_owned).collect();

        let mut lines = lines.map(str::trim).enumerate().peekable();
        while let Some((line, row)) = lines.next() {
            if row.contains('(') {
                return Err(ErrorKind::ParseError {
                    msg: format!("Unexpected mode heuristics line '{row}'"),
                    line: line + 1,
                }
                .into());
            }

            let mut split = row.split_whitespace();
            if let Some(num) = split.next().and_then(|part| part.parse::<u16>().ok()) {
                let name_part = split
                    .next()
                    .ok_or_else(|| Error::unexpected_eol("No name after mode number", line + 1))?
                    .trim_end_matches(':');

                let name = if let Some(name) = name_part.strip_suffix('*') {
                    active = Some(num);
                    name.trim()
                } else {
                    name_part
                };

                let mut components = Vec::new();

                while lines
                    .peek()
                    .is_some_and(|(_, row)| row.contains(['(', ')']))
                {
                    let (line, clock_type_line) = lines.next().unwrap();

                    let name_start = clock_type_line
                        .char_indices()
                        .position(|(_, c)| c == '(')
                        .ok_or_else(|| Error::unexpected_eol('(', line + 1))?;

                    let name_end = clock_type_line
                        .char_indices()
                        .position(|(_, c)| c == ')')
                        .ok_or_else(|| Error::unexpected_eol(')', line + 1))?;

                    let clock_type = clock_type_line[name_start + 1..name_end].trim();

                    let clock_type_values = clock_type_line[name_end + 1..]
                        .split_whitespace()
                        .map(str::trim)
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
                        .collect::<Result<Vec<Option<i32>>>>()?;

                    components.push(PowerProfileComponent {
                        clock_type: Some(clock_type.to_owned()),
                        values: clock_type_values,
                    })
                }

                let power_profile = PowerProfile {
                    name: name.to_owned(),
                    components,
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

    /// Parse "rotated" format (with columns as profiles, and rows as values).
    /// Used at least by RDNA3 laptop GPUs (example data: 7700s)
    fn parse_rotated(s: &str) -> Result<Self> {
        let mut modes = BTreeMap::new();
        let mut active = None;

        let mut lines = s.lines().map(str::trim).enumerate();

        let mut header_split = lines
            .next()
            .ok_or_else(|| Error::basic_parse_error("Missing header"))?
            .1
            .split_whitespace();

        while let Some(raw_index) = header_split.next() {
            let index: u16 = raw_index.parse()?;

            let mut name = header_split
                .next()
                .ok_or_else(|| Error::unexpected_eol("Missing section name", 1))?;

            if let Some(stripped) = name.strip_suffix("*") {
                name = stripped;
                active = Some(index);
            }

            modes.insert(
                index,
                PowerProfile {
                    name: name.to_owned(),
                    components: vec![],
                },
            );
        }

        let mut value_names = vec![];

        for (i, line) in lines {
            let mut split = line.split_whitespace();
            let value_name = split
                .next()
                .ok_or_else(|| Error::unexpected_eol("Value name", i + 1))?;

            value_names.push(value_name.to_owned());

            for (profile_i, raw_value) in split.enumerate() {
                let value = raw_value.parse()?;

                let component = PowerProfileComponent {
                    clock_type: None,
                    values: vec![Some(value)],
                };

                modes
                    .get_mut(&(profile_i as u16))
                    .ok_or_else(|| {
                        Error::basic_parse_error("Could not get profile from header by index")
                    })?
                    .components
                    .push(component);
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
                        components: vec![],
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

impl PowerProfile {
    /// If this is the custom profile (checked by name)
    pub fn is_custom(&self) -> bool {
        self.name.eq_ignore_ascii_case("CUSTOM")
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
    const TABLE_RX7700S: &str = include_test_data!("rx7700s/pp_power_profile_mode");
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
    fn parse_full_rx7700s() {
        let table = PowerProfileModesTable::parse(TABLE_RX7700S).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn parse_full_rx7800xt() {
        let table = PowerProfileModesTable::parse(TABLE_RX7800XT).unwrap();
        assert_yaml_snapshot!(table);
    }
}
