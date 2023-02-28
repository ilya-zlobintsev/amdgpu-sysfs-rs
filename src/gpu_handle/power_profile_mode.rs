use super::trim_sysfs_line;
use crate::{error::Error, Result};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PowerProfileModesTable<'a> {
    #[serde(borrow)]
    pub modes: Vec<Mode<'a>>,
    pub active: usize,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Mode<'a> {
    pub name: &'a str,
    pub heuristics: HashMap<&'a str, &'a str>,
}

impl<'a> PowerProfileModesTable<'a> {
    pub fn parse(s: &'a str) -> Result<Self> {
        let mut lines = s.lines().map(trim_sysfs_line).enumerate();
        let (_, header) = lines
            .next()
            .ok_or_else(|| Error::basic_parse_error("Could not read header"))?;

        let heuristic_definitions = parse_header(header)?;
        let mut modes = Vec::new();
        let mut active = None;

        for (line, row) in lines {
            let mut parts = row.split_whitespace();

            let num = parts
                .next()
                .ok_or_else(|| Error::unexpected_eol("num", line))?;

            let mut name = parts
                .next()
                .ok_or_else(|| Error::unexpected_eol("mode name", line))?;

            println!("parsing name {name}");

            if let Some(active_name) = name.strip_suffix("*:") {
                name = active_name;
                active = Some(num.parse()?);
            } else {
                parts.next(); // Skip `:` after mode name
            }

            let mut heuristics = HashMap::with_capacity(heuristic_definitions.len());

            for (column, value) in parts.enumerate() {
                let heurisitc_name = heuristic_definitions[column];
                heuristics.insert(heurisitc_name, value);
            }

            modes.push(Mode { name, heuristics });
        }

        Ok(Self {
            modes,
            active: active
                .ok_or_else(|| Error::basic_parse_error("could not find active state"))?,
        })
    }
}

fn parse_header(header: &str) -> Result<Vec<&str>> {
    let mut parts = header.split_whitespace();

    let num_part = parts
        .next()
        .ok_or_else(|| Error::unexpected_eol("NUM column", 1))?;
    if num_part != "NUM" {
        return Err(Error::basic_parse_error(format!(
            "Expected the first column to be NUM, found {num_part}"
        )));
    }

    let name_part = parts
        .next()
        .ok_or_else(|| Error::unexpected_eol("NAME column", 1))?;
    if name_part != "MODE_NAME" {
        return Err(Error::basic_parse_error(format!(
            "Expected the second column to be MODE_NAME, found {num_part}"
        )));
    }

    Ok(parts.collect())
}

#[cfg(test)]
mod tests {
    use super::{parse_header, PowerProfileModesTable};
    use insta::assert_yaml_snapshot;
    use pretty_assertions::assert_eq;

    const TABLE_VEGA56: &str = include_test_data!("vega56/pp_power_profile_mode");

    #[test]
    fn parse_header_vega56() {
        let header = "NUM        MODE_NAME BUSY_SET_POINT FPS USE_RLC_BUSY MIN_ACTIVE_LEVEL";
        let heuristics = parse_header(header).unwrap();
        assert_eq!(
            heuristics,
            ["BUSY_SET_POINT", "FPS", "USE_RLC_BUSY", "MIN_ACTIVE_LEVEL"]
        );
    }

    #[test]
    fn parse_header_rx580() {
        let header = "NUM        MODE_NAME     SCLK_UP_HYST   SCLK_DOWN_HYST SCLK_ACTIVE_LEVEL     MCLK_UP_HYST   MCLK_DOWN_HYST MCLK_ACTIVE_LEVEL";
        let heuristics = parse_header(header).unwrap();
        assert_eq!(
            heuristics,
            [
                "SCLK_UP_HYST",
                "SCLK_DOWN_HYST",
                "SCLK_ACTIVE_LEVEL",
                "MCLK_UP_HYST",
                "MCLK_DOWN_HYST",
                "MCLK_ACTIVE_LEVEL"
            ]
        );
    }

    #[test]
    fn parse_full_vega56() {
        let table = PowerProfileModesTable::parse(TABLE_VEGA56).unwrap();
        assert_yaml_snapshot!(table);
    }
}
