//! Types for working with the dedicated fan control interface.
//! Only for Navi 3x (RDNA 3) and newer. Older GPUs have to use the HwMon interface.
use crate::{
    error::{Error, ErrorKind},
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Write};

/// Information about fan characteristics.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanInfo {
    /// Current value
    pub current: u32,
    /// Minimum and maximum allowed values.
    /// This is empty if changes to the value are not supported.
    pub allowed_range: Option<(u32, u32)>,
}

/// Custom fan curve
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanCurve {
    /// Fan curve points in the (temperature, speed) format
    pub points: Vec<(u32, u8)>,
    /// Allowed value ranges.
    /// Empty when changes to the fan curve are not supported.
    pub allowed_ranges: Option<FanCurveRanges>,
}

/// Range of values allowed to be used within fan curve points
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanCurveRanges {
    /// Temperature range allowed in curve points
    pub temperature_range: (u32, u32),
    /// Fan speed range allowed in curve points
    pub speed_range: (u8, u8),
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) struct FanCtrlContents {
    pub contents: String,
    pub od_range: HashMap<String, (String, String)>,
}

impl FanCtrlContents {
    pub(crate) fn parse(data: &str, expected_section_name: &str) -> Result<Self> {
        let mut lines = data.lines().enumerate();
        let (_, section_line) = lines
            .next()
            .ok_or_else(|| Error::unexpected_eol("Section name", 1))?;

        let section_name = section_line.strip_suffix(':').ok_or_else(|| {
            Error::basic_parse_error(format!("Section \"{section_line}\" should end with \":\""))
        })?;

        if section_name != expected_section_name {
            return Err(Error::basic_parse_error(format!(
                "Found section {section_name}, expected {expected_section_name}"
            )));
        }

        let mut contents = String::new();
        for (_, line) in &mut lines {
            if line == "OD_RANGE:" {
                break;
            }
            writeln!(contents, "{line}").unwrap();
        }
        contents.pop(); // Remove newline symbol

        let mut od_range = HashMap::new();
        for (i, range_line) in lines {
            let (name, value) =
                range_line
                    .split_once(": ")
                    .ok_or_else(|| ErrorKind::ParseError {
                        msg: format!("Range line \"{range_line}\" does not have a separator"),
                        line: i + 1,
                    })?;
            let (min, max) = value.split_once(' ').ok_or_else(|| ErrorKind::ParseError {
                msg: format!(
                    "Range line \"{range_line}\" does not have a separator between the values"
                ),
                line: i + 1,
            })?;

            od_range.insert(name.to_owned(), (min.to_owned(), max.to_owned()));
        }

        Ok(Self { contents, od_range })
    }
}

#[cfg(test)]
mod tests {
    use super::FanCtrlContents;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_od_acoustic_limit() {
        let data = "\
OD_ACOUSTIC_LIMIT:
2450
OD_RANGE:
ACOUSTIC_LIMIT: 500 3100";
        let contents = FanCtrlContents::parse(data, "OD_ACOUSTIC_LIMIT").unwrap();
        let expected_contents = FanCtrlContents {
            contents: "2450".to_owned(),
            od_range: [(
                "ACOUSTIC_LIMIT".to_owned(),
                ("500".to_owned(), "3100".to_owned()),
            )]
            .into_iter()
            .collect(),
        };
        assert_eq!(expected_contents, contents);
    }

    #[test]
    fn parse_fan_curve() {
        let data = "\
OD_FAN_CURVE:
0: 0C 0%
1: 0C 0%
2: 0C 0%
3: 0C 0%
4: 0C 0%
OD_RANGE:
FAN_CURVE(hotspot temp): 25C 100C
FAN_CURVE(fan speed): 20% 100%";
        let contents = FanCtrlContents::parse(data, "OD_FAN_CURVE").unwrap();
        let expected_contents = FanCtrlContents {
            contents: "\
0: 0C 0%
1: 0C 0%
2: 0C 0%
3: 0C 0%
4: 0C 0%"
                .to_owned(),
            od_range: [
                (
                    "FAN_CURVE(hotspot temp)".to_owned(),
                    ("25C".to_owned(), "100C".to_owned()),
                ),
                (
                    "FAN_CURVE(fan speed)".to_owned(),
                    ("20%".to_owned(), "100%".to_owned()),
                ),
            ]
            .into_iter()
            .collect(),
        };
        assert_eq!(expected_contents, contents);
    }
}
