//! The format used by Vega20 and newer GPUs.
use super::{
    parse_line_item, parse_range_line, push_level_line, AllowedRanges, ClocksLevel, ClocksTable,
    Range,
};
use crate::{
    error::{Error, ErrorKind::ParseError},
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{io::Write, str::FromStr};

/// Vega20 clocks table.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Table {
    /// The current core clock range.
    pub current_sclk_range: Range,
    /// The current memory clock range.
    pub current_mclk_range: Range,
    /// The current voltage curve. May be empty if the GPU does not support it.
    pub vddc_curve: Vec<ClocksLevel>,
    /// The allowed ranges for clockspeeds.
    pub allowed_ranges: AllowedRanges,
}

impl ClocksTable for Table {
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<()> {
        let clockspeeds = [
            (self.current_sclk_range.min, 's', 0),
            (self.current_sclk_range.max, 's', 1),
            (self.current_mclk_range.min, 'm', 0),
            (self.current_mclk_range.max, 'm', 1),
        ];

        for (maybe_clockspeed, symbol, index) in clockspeeds {
            if let Some(clockspeed) = maybe_clockspeed {
                write_clockspeed_line(writer, symbol, index, clockspeed)?;
            }
        }

        Ok(())
    }

    fn get_max_sclk(&self) -> Option<u32> {
        self.current_sclk_range.max
    }

    fn get_max_mclk(&self) -> Option<u32> {
        self.current_mclk_range.max
    }
}

impl FromStr for Table {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut current_section = None;

        let mut current_sclk_range = None;
        let mut current_mclk_range = None;
        let mut allowed_sclk_range = None;
        let mut allowed_mclk_range = None;
        let mut vddc_curve = Vec::new();

        let mut i = 1;
        for line in s.lines().map(str::trim).filter(|line| !line.is_empty()) {
            match line {
                "OD_SCLK:" => current_section = Some(Section::Sclk),
                "OD_MCLK:" => current_section = Some(Section::Mclk),
                "OD_RANGE:" => current_section = Some(Section::Range),
                "OD_VDDC_CURVE:" => current_section = Some(Section::VddcCurve),
                _ if line.starts_with("VDDC_CURVE_") => {
                    continue; // TODO
                }
                line => match current_section {
                    Some(Section::Range) => {
                        let (range, name) = parse_range_line(line, i)?;
                        match name {
                            "SCLK" => allowed_sclk_range = Some(range),
                            "MCLK" => allowed_mclk_range = Some(range),
                            other => {
                                return Err(ParseError {
                                    msg: format!("Unexpected range item: {other}"),
                                    line: i,
                                }
                                .into())
                            }
                        }
                    }
                    Some(Section::Sclk) => parse_min_max_line(line, i, &mut current_sclk_range)?,
                    Some(Section::Mclk) => parse_min_max_line(line, i, &mut current_mclk_range)?,
                    Some(Section::VddcCurve) => push_level_line(line, &mut vddc_curve, i)?,
                    None => {
                        return Err(ParseError {
                            msg: "Unexpected line without section".to_owned(),
                            line: i,
                        }
                        .into())
                    }
                },
            }
            i += 1;
        }

        let allowed_ranges = AllowedRanges {
            sclk: allowed_sclk_range.ok_or_else(|| ParseError {
                msg: "No sclk range found".to_owned(),
                line: i,
            })?,
            mclk: Some(allowed_mclk_range.ok_or_else(|| ParseError {
                msg: "No mclk range found".to_owned(),
                line: i,
            })?),
            vddc: None,
        };
        let current_sclk_range = current_sclk_range.ok_or_else(|| ParseError {
            msg: "No current sclk range found".to_owned(),
            line: i,
        })?;
        let current_mclk_range = current_mclk_range.ok_or_else(|| ParseError {
            msg: "No current mclk range found".to_owned(),
            line: i,
        })?;

        Ok(Self {
            allowed_ranges,
            current_sclk_range,
            current_mclk_range,
            vddc_curve,
        })
    }
}

enum Section {
    Sclk,
    Mclk,
    VddcCurve,
    Range,
}

fn parse_clockspeed_line(line: &str, i: usize) -> Result<(u32, usize)> {
    let mut split = line.split_whitespace();
    let num = parse_line_item(&mut split, i, "level number", &[":"])?;
    let clockspeed = parse_line_item(&mut split, i, "clockspeed", &["mhz"])?;

    Ok((clockspeed, num))
}

fn parse_min_max_line(line: &str, i: usize, range: &mut Option<Range>) -> Result<()> {
    let (clockspeed, num) = parse_clockspeed_line(line, i)?;
    match num {
        0 => {
            *range = Some(Range::min(clockspeed));
            Ok(())
        }
        1 => {
            if let Some(range) = range {
                range.max = Some(clockspeed);
            } else {
                *range = Some(Range::max(clockspeed));
            }
            Ok(())
        }
        _ => Err(ParseError {
            msg: format!("Unexpected range number {num}"),
            line: i,
        }
        .into()),
    }
}

fn write_clockspeed_line<W: Write>(
    writer: &mut W,
    symbol: char,
    index: u8,
    clockspeed: u32,
) -> Result<()> {
    writeln!(writer, "{symbol} {index} {clockspeed}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Table;
    use crate::gpu_handle::overdrive::{AllowedRanges, ClocksLevel, ClocksTable, Range};
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    const TABLE_5700XT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/rx5700xt/pp_od_clk_voltage"
    ));

    #[test]
    fn parse_5700xt_full() {
        let table = Table::from_str(TABLE_5700XT).unwrap();

        assert_eq!(table.current_sclk_range, Range::full(800, 2100));
        assert_eq!(table.current_mclk_range, Range::max(875));

        let vddc_curve =
            [(800, 711), (1450, 801), (2100, 1191)].map(|(clockspeed, voltage)| ClocksLevel {
                clockspeed,
                voltage,
            });
        assert_eq!(table.vddc_curve, vddc_curve);

        let allowed_ranges = AllowedRanges {
            sclk: Range::full(800, 2150),
            mclk: Some(Range::full(625, 950)),
            vddc: None,
        };
        assert_eq!(table.allowed_ranges, allowed_ranges);
    }

    #[test]
    fn max_clocks_5700xt() {
        let table = Table::from_str(TABLE_5700XT).unwrap();
        assert_eq!(table.get_max_sclk(), Some(2100));
        assert_eq!(table.get_max_mclk(), Some(875));
    }

    #[test]
    fn write_commands_5700xt() {
        let table = Table::from_str(TABLE_5700XT).unwrap();
        let mut buf = Vec::new();
        table.write_commands(&mut buf).unwrap();
        let commands = String::from_utf8(buf).unwrap();

        let mut expected_commands = ["s 0 800", "s 1 2100", "m 1 875"].join("\n");
        expected_commands.push('\n');

        assert_eq!(commands, expected_commands);
    }
}
