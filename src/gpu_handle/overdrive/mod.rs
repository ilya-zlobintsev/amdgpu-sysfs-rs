//! GPU overdrive (overclocking)
//!
//! <https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#pp-od-clk-voltage>
pub mod vega10;
pub mod vega20;

use crate::{
    error::{Error, ErrorKind},
    Result,
};
use enum_dispatch::enum_dispatch;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{
    convert::TryFrom,
    io::Write,
    str::{FromStr, SplitWhitespace},
};

/// Shared functionality across all table formats.
#[enum_dispatch]
pub trait ClocksTable: FromStr {
    /// Writes commands needed to apply the state that is in the table struct on the GPU.
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<()>;

    /// Get the clockspeed and voltage ranges which the GPU allows.
    fn get_allowed_ranges(&self) -> AllowedRanges;

    /// Gets the current maximum core clock.
    fn get_max_sclk(&self) -> Option<u32>;

    /// Sets the maximum core clock.
    fn set_max_sclk(&mut self, clockspeed: u32) -> Result<()> {
        let allowed_ranges = self.get_allowed_ranges();
        check_clockspeed_in_range(Some(allowed_ranges.sclk), clockspeed)?;
        self.set_max_sclk_unchecked(clockspeed)
    }

    /// Sets the maximum core clock (without checking if it's in the allowed range).
    fn set_max_sclk_unchecked(&mut self, clockspeed: u32) -> Result<()>;

    /// Gets the current maximum memory clock.
    fn get_max_mclk(&self) -> Option<u32>;

    /// Sets the maximum memory clock.
    fn set_max_mclk(&mut self, clockspeed: u32) -> Result<()> {
        let allowed_ranges = self.get_allowed_ranges();
        check_clockspeed_in_range(allowed_ranges.mclk, clockspeed)?;
        self.set_max_mclk_unchecked(clockspeed)
    }

    /// Sets the maximum memory clock (without checking if it's in the allowed range).
    fn set_max_mclk_unchecked(&mut self, clockspeed: u32) -> Result<()>;

    /// Gets the current maximum voltage (used on maximum clockspeed).
    fn get_max_sclk_voltage(&self) -> Option<u32>;
}

fn check_clockspeed_in_range(range: Option<Range>, clockspeed: u32) -> Result<()> {
    if let (Some(min), Some(max)) = range.map_or((None, None), |range| (range.min, range.max)) {
        if (min..=max).contains(&clockspeed) {
            Ok(())
        } else {
            Err(Error::not_allowed(format!(
                "Given clockspeed {clockspeed} is out of the allowed OD range {min} to {max}"
            )))
        }
    } else {
        Err(Error::not_allowed(
            "GPU does not report allowed OD ranges".to_owned(),
        ))
    }
}

/// Representation of `pp_od_clk_voltage`
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "data", rename_all = "snake_case")
)]
#[enum_dispatch(ClocksTable)]
pub enum ClocksTableGen {
    /// Vega10 (and older) format
    Vega10(vega10::Table),
    /// Vega20 (and newer) format
    Vega20(vega20::Table),
}

impl FromStr for ClocksTableGen {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.contains("VDDC_CURVE") || s.contains("OD_VDDGFX_OFFSET") {
            vega20::Table::from_str(s).map(Self::Vega20)
        } else {
            vega10::Table::from_str(s).map(Self::Vega10)
        }
    }
}

fn parse_range_line(line: &str, i: usize) -> Result<(Range, &str)> {
    let mut split = line.split_whitespace();
    let name = split
        .next()
        .ok_or_else(|| Error::unexpected_eol("range name", i))?
        .trim_end_matches(':');
    let min = parse_line_item(&mut split, i, "range minimum", &["mhz", "mv"])?;
    let max = parse_line_item(&mut split, i, "range maximum", &["mhz", "mv"])?;

    Ok((Range::full(min, max), name))
}

/// Takes the next item from a split, strips the given suffixes, an parses it to a type
fn parse_line_item<T>(
    split: &mut SplitWhitespace,
    i: usize,
    item: &str,
    suffixes: &[&str],
) -> Result<T>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    let text = split
        .next()
        .ok_or_else(|| Error::unexpected_eol(item, i))?
        .to_lowercase();
    let mut trimmed_text = text.as_str();

    for suffix in suffixes {
        if cfg!(test) && suffix.chars().any(|ch| ch.is_uppercase()) {
            panic!("Suffixes must be all lowercase");
        }
        trimmed_text = trimmed_text.trim_end_matches(suffix);
    }

    trimmed_text.parse().map_err(|err| {
        ErrorKind::ParseError {
            msg: format!("Could not parse {item} with value {trimmed_text}: {err}"),
            line: i,
        }
        .into()
    })
}

/// The ranges which the GPU allows to be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AllowedRanges {
    /// Clocks range for sclk (in MHz). Should be present on all GPUs.
    pub sclk: Range,
    /// Clocks range for mclk (in MHz). Present on discrete GPUs only.
    pub mclk: Option<Range>,
    /// Voltage range (in mV). Present on Vega10 and older GPUs only.
    pub vddc: Option<Range>,
}

impl Default for AllowedRanges {
    fn default() -> Self {
        Self {
            sclk: Range::empty(),
            mclk: None,
            vddc: None,
        }
    }
}

/// A range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Range {
    /// The lower value of a range.
    pub min: Option<u32>,
    /// The higher value of a range.
    pub max: Option<u32>,
}

impl Range {
    /// Creates a range with both a minimum and a maximum value.
    pub fn full(min: u32, max: u32) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    /// Creates a rage with a minimum value only.
    pub fn min(min: u32) -> Self {
        Self {
            min: Some(min),
            max: None,
        }
    }

    /// Creates a rage with a maximum value only.
    pub fn max(max: u32) -> Self {
        Self {
            min: None,
            max: Some(max),
        }
    }

    /// Creates an empty range.
    pub fn empty() -> Self {
        Self {
            min: None,
            max: None,
        }
    }
}

impl TryFrom<Range> for (u32, u32) {
    type Error = ();

    fn try_from(value: Range) -> std::result::Result<Self, Self::Error> {
        if let (Some(min), Some(max)) = (value.min, value.max) {
            Ok((min, max))
        } else {
            Err(())
        }
    }
}

/// Represents a combination of a clockspeed and voltage. May be used in different context based on the table format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ClocksLevel {
    /// Clockspeed (in MHz)
    pub clockspeed: u32,
    /// Voltage (in mV)
    pub voltage: u32,
}

impl ClocksLevel {
    /// Create a new clocks level.
    pub fn new(clockspeed: u32, voltage: u32) -> Self {
        Self {
            clockspeed,
            voltage,
        }
    }
}

fn parse_level_line(line: &str, i: usize) -> Result<(ClocksLevel, usize)> {
    let mut split = line.split_whitespace();
    let num = parse_line_item(&mut split, i, "level number", &[":"])?;
    let clockspeed = parse_line_item(&mut split, i, "clockspeed", &["mhz"])?;
    let voltage = parse_line_item(&mut split, i, "voltage", &["mv"])?;

    Ok((ClocksLevel::new(clockspeed, voltage), num))
}

fn push_level_line(line: &str, levels: &mut Vec<ClocksLevel>, i: usize) -> Result<()> {
    let (level, num) = parse_level_line(line, i)?;

    let len = levels.len();
    if num != len {
        return Err(ErrorKind::ParseError {
            msg: format!("Unexpected level num: expected {len}, got {num}"),
            line: i,
        }
        .into());
    }

    levels.push(level);
    Ok(())
}

#[cfg(test)]
fn arr_commands<const N: usize>(commands: [&str; N]) -> String {
    let mut output = commands.join("\n");
    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use super::{check_clockspeed_in_range, parse_level_line, parse_range_line, Range};

    #[test]
    fn parse_range_line_sclk() {
        let line = "SCLK:     300MHz       2000MHz";
        let (level, name) = parse_range_line(line, 50).unwrap();
        assert_eq!(name, "SCLK");
        assert_eq!(level.min, Some(300));
        assert_eq!(level.max, Some(2000));
    }

    #[test]
    fn parse_level_line_basic() {
        let line = "0:        300MHz        750mV";
        let (level, i) = parse_level_line(line, 50).unwrap();
        assert_eq!(i, 0);
        assert_eq!(level.clockspeed, 300);
        assert_eq!(level.voltage, 750);
    }

    #[test]
    fn allowed_ranges() {
        let range = Some(Range::full(300, 1000));
        check_clockspeed_in_range(range, 300).unwrap();
        check_clockspeed_in_range(range, 750).unwrap();
        check_clockspeed_in_range(range, 1000).unwrap();
        check_clockspeed_in_range(range, 1001).unwrap_err();
        check_clockspeed_in_range(range, 250).unwrap_err();
    }
}
