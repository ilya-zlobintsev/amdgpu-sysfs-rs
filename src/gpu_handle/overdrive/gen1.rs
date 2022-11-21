//! The format used by Vega10 and older GPUs.
use super::{AllowedRanges, Range};
use crate::error::Error;
use crate::error::ErrorKind::ParseError;
use std::str::{FromStr, SplitWhitespace};

#[derive(Debug, Clone)]
pub struct Table {
    pub sclk_levels: Vec<ClocksLevel>,
    pub mclk_levels: Vec<ClocksLevel>,
    pub allowed_ranges: AllowedRanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClocksLevel {
    /// Clockspeed (in MHz)
    pub clockspeed: u32,
    /// Voltage (in mV)
    pub voltage: u32,
}

impl FromStr for Table {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut sclk_levels = Vec::with_capacity(7);
        let mut mclk_levels = Vec::with_capacity(2);
        let mut sclk_range = None;
        let mut mclk_range = None;
        let mut vddc_range = None;

        let mut current_section = None;

        let mut i = 1;
        for line in s
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
        {
            match line {
                "OD_SCLK:" => current_section = Some(Section::Sclk),
                "OD_MCLK:" => current_section = Some(Section::Mclk),
                "OD_RANGE:" => current_section = Some(Section::Range),
                line => match current_section {
                    Some(Section::Sclk) => {
                        push_level_line(line, &mut sclk_levels, i)?;
                    }
                    Some(Section::Mclk) => {
                        push_level_line(line, &mut mclk_levels, i)?;
                    }
                    Some(Section::Range) => {
                        let (range, name) = parse_range_line(line, i)?;
                        match name {
                            "SCLK" => sclk_range = Some(range),
                            "MCLK" => mclk_range = Some(range),
                            "VDDC" => vddc_range = Some(range),
                            other => {
                                return Err(ParseError {
                                    msg: format!("Unexpected range item: {other}"),
                                    line: i,
                                }
                                .into())
                            }
                        }
                    }
                    None => {
                        return Err(ParseError {
                            msg: "Could not find section".to_owned(),
                            line: i,
                        }
                        .into())
                    }
                },
            }
            i += 1;
        }

        sclk_levels.shrink_to_fit();
        mclk_levels.shrink_to_fit();

        let allowed_ranges = AllowedRanges {
            sclk: sclk_range.ok_or_else(|| ParseError {
                msg: "No sclk range found".to_owned(),
                line: i,
            })?,
            mclk: mclk_range,
            vddc: vddc_range,
        };

        Ok(Self {
            sclk_levels,
            mclk_levels,
            allowed_ranges,
        })
    }
}

fn push_level_line(line: &str, levels: &mut Vec<ClocksLevel>, i: usize) -> Result<(), Error> {
    let (level, num) = parse_level_line(line, i)?;

    let len = levels.len();
    if num != len {
        return Err(ParseError {
            msg: format!("Unexpected level num: expected {len}, got {num}"),
            line: i,
        }
        .into());
    }

    levels.push(level);
    Ok(())
}

fn parse_level_line(line: &str, i: usize) -> Result<(ClocksLevel, usize), Error> {
    let mut split = line.split_whitespace();
    let num = parse_line_item(&mut split, i, "level number", &[":"])?;
    let clockspeed = parse_line_item(&mut split, i, "clockspeed", &["MHz"])?;
    let voltage = parse_line_item(&mut split, i, "voltage", &["mV"])?;

    Ok((
        ClocksLevel {
            clockspeed,
            voltage,
        },
        num,
    ))
}

fn parse_range_line(line: &str, i: usize) -> Result<(Range, &str), Error> {
    let mut split = line.split_whitespace();
    let name = split
        .next()
        .ok_or_else(|| Error::unexpected_eol("range name", i))?
        .trim_end_matches(':');
    let min = parse_line_item(&mut split, i, "range minimum", &["MHz", "mV"])?;
    let max = parse_line_item(&mut split, i, "range maximum", &["MHz", "mV"])?;

    Ok((Range { min, max }, name))
}

fn parse_line_item<T>(
    split: &mut SplitWhitespace,
    i: usize,
    item: &str,
    suffixes: &[&str],
) -> Result<T, Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    let mut text = split.next().ok_or_else(|| Error::unexpected_eol(item, i))?;

    for suffix in suffixes {
        text = text.trim_end_matches(suffix);
    }

    text.parse().map_err(|err| {
        ParseError {
            msg: format!("Could not parse {item}: {err}"),
            line: i,
        }
        .into()
    })
}

#[derive(PartialEq)]
enum Section {
    Sclk,
    Mclk,
    Range,
}

#[cfg(test)]
mod tests {
    use crate::gpu_handle::overdrive::{AllowedRanges, Range};

    use super::{parse_level_line, parse_range_line, ClocksLevel, Table};
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    #[test]
    fn parse_level_line_basic() {
        let line = "0:        300MHz        750mV";
        let (level, i) = parse_level_line(line, 50).unwrap();
        assert_eq!(i, 0);
        assert_eq!(level.clockspeed, 300);
        assert_eq!(level.voltage, 750);
    }

    #[test]
    fn parse_range_line_sclk() {
        let line = "SCLK:     300MHz       2000MHz";
        let (level, name) = parse_range_line(line, 50).unwrap();
        assert_eq!(name, "SCLK");
        assert_eq!(level.min, 300);
        assert_eq!(level.max, 2000);
    }

    #[test]
    fn parse_full_table() {
        let data = r#"
            OD_SCLK:
            0:        300MHz        750mV
            1:        600MHz        769mV
            2:        900MHz        912mV
            3:       1145MHz       1125mV
            4:       1215MHz       1150mV
            5:       1257MHz       1150mV
            6:       1300MHz       1150mV
            7:       1366MHz       1150mV
            OD_MCLK:
            0:        300MHz        750mV
            1:       1000MHz        825mV
            2:       1750MHz        975mV
            OD_RANGE:
            SCLK:     300MHz       2000MHz
            MCLK:     300MHz       2250MHz
            VDDC:     750mV        1200mV
        "#;
        let table = Table::from_str(data).unwrap();

        let sclk_levels = [
            (300, 750),
            (600, 769),
            (900, 912),
            (1145, 1125),
            (1215, 1150),
            (1257, 1150),
            (1300, 1150),
            (1366, 1150),
        ]
        .map(|(clockspeed, voltage)| ClocksLevel {
            clockspeed,
            voltage,
        });
        let mclk_levels =
            [(300, 750), (1000, 825), (1750, 975)].map(|(clockspeed, voltage)| ClocksLevel {
                clockspeed,
                voltage,
            });
        let ranges = AllowedRanges {
            sclk: Range {
                min: 300,
                max: 2000,
            },
            mclk: Some(Range {
                min: 300,
                max: 2250,
            }),
            vddc: Some(Range {
                min: 750,
                max: 1200,
            }),
        };

        assert_eq!(table.sclk_levels, sclk_levels);
        assert_eq!(table.mclk_levels, mclk_levels);
        assert_eq!(table.allowed_ranges, ranges);
    }
}
