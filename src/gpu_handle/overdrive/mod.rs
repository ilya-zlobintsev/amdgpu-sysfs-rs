pub mod vega10;
pub mod vega20;

use crate::{
    error::{Error, ErrorKind},
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufWriter, Write},
    str::{FromStr, SplitWhitespace},
};

pub trait PowerTable: FromStr {
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<()>;

    fn get_max_sclk(&self) -> Option<u32>;

    fn get_max_mclk(&self) -> Option<u32>;
}

/// Representation of `pp_od_clk_voltage`
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "data", rename_all = "snake_case")
)]
pub enum PowerTableGen {
    Vega10(vega10::Table),
    Vega20(vega20::Table),
}

impl FromStr for PowerTableGen {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.contains("VDDC_CURVE") || s.contains("OD_VDDGFX_OFFSET") {
            vega20::Table::from_str(s).map(Self::Vega20)
        } else {
            vega10::Table::from_str(s).map(Self::Vega10)
        }
    }
}

impl PowerTable for PowerTableGen {
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            PowerTableGen::Vega10(table) => table.write_commands(writer),
            PowerTableGen::Vega20(table) => table.write_commands(writer),
        }
    }

    fn get_max_sclk(&self) -> Option<u32> {
        match self {
            PowerTableGen::Vega10(table) => table.get_max_sclk(),
            PowerTableGen::Vega20(table) => table.get_max_sclk(),
        }
    }

    fn get_max_mclk(&self) -> Option<u32> {
        match self {
            PowerTableGen::Vega10(table) => table.get_max_mclk(),
            PowerTableGen::Vega20(table) => table.get_max_mclk(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Range {
    pub min: Option<u32>,
    pub max: Option<u32>,
}

impl Range {
    pub fn full(min: u32, max: u32) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    pub fn min(min: u32) -> Self {
        Self {
            min: Some(min),
            max: None,
        }
    }

    pub fn max(max: u32) -> Self {
        Self {
            min: None,
            max: Some(max),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ClocksLevel {
    /// Clockspeed (in MHz)
    pub clockspeed: u32,
    /// Voltage (in mV)
    pub voltage: u32,
}

fn parse_level_line(line: &str, i: usize) -> Result<(ClocksLevel, usize)> {
    let mut split = line.split_whitespace();
    let num = parse_line_item(&mut split, i, "level number", &[":"])?;
    let clockspeed = parse_line_item(&mut split, i, "clockspeed", &["mhz"])?;
    let voltage = parse_line_item(&mut split, i, "voltage", &["mv"])?;

    Ok((
        ClocksLevel {
            clockspeed,
            voltage,
        },
        num,
    ))
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

pub struct PowerTableHandle {
    writer: BufWriter<File>,
}

impl PowerTableHandle {
    pub(crate) fn new(writer: BufWriter<File>) -> Self {
        Self { writer }
    }
}

impl PowerTableHandle {
    /// Commit the pending changes.
    pub fn commit(mut self) -> Result<()> {
        self.writer.write_all(b"c\n")?;
        self.writer.flush()?;
        Ok(())
    }

    /// Reset the pending changes.
    pub fn reset(mut self) -> Result<()> {
        self.writer.write_all(b"r\n")?;
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_level_line, parse_range_line};

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
}
