use std::collections::HashMap;

use crate::{error::Error, gpu_handle::trim_sysfs_line, Result};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The full table format used by dedicated GPUs
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FullTable {
    /// List of available modes
    pub modes: Vec<FullTableMode>,
    /// Index of the currently active mode
    pub active: usize,
    /// List of available heuristics in the original order
    pub available_heuristics: Vec<String>,
}

/// A speficic power mode
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FullTableMode {
    /// Name of the mode
    pub name: String,
    /// Heuristics defined for this mode
    pub heuristics: HashMap<String, Option<String>>,
}

impl FullTable {
    pub(crate) fn parse(s: &str) -> Result<Self> {
        let mut lines = s.lines().map(trim_sysfs_line).enumerate();
        let (_, header) = lines
            .next()
            .ok_or_else(|| Error::basic_parse_error("Could not read header"))?;

        let available_heuristics = parse_header(header)?;
        let mut modes = Vec::new();
        let mut active = None;

        for (line, row) in lines {
            let mut parts = row.split_whitespace();

            let num = parts
                .next()
                .ok_or_else(|| Error::unexpected_eol("num", line))?;

            // Depending on the specific GPU there may or may not be a space before the active specifier,
            // so this is the most reliable way to check it
            if row.contains("*:") {
                active = Some(num.parse()?);
            }

            let name = parts
                .next()
                .ok_or_else(|| Error::unexpected_eol("mode name", line))?
                .trim_matches(':')
                .trim_matches('*')
                .to_owned();

            let mut heuristics = HashMap::with_capacity(available_heuristics.len());

            let mut i = 0;
            for value in parts {
                // Skip separator items and don't increase index for them
                if matches!(value, "*" | ":*" | "*:" | ":") {
                    continue;
                }

                let heurisitc_name = &available_heuristics[i];
                let value = if value == "-" {
                    None
                } else {
                    Some(value.to_owned())
                };

                heuristics.insert(heurisitc_name.clone(), value);

                i += 1;
            }

            modes.push(FullTableMode { name, heuristics });
        }

        Ok(Self {
            modes,
            active: active
                .ok_or_else(|| Error::basic_parse_error("could not find active state"))?,
            available_heuristics,
        })
    }
}

fn parse_header(header: &str) -> Result<Vec<String>> {
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

    Ok(parts.map(str::to_owned).collect())
}

#[cfg(test)]
mod tests {
    use super::parse_header;
    use pretty_assertions::assert_eq;

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
}
