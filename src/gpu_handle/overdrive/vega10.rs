//! The format used by Vega10 and older GPUs.
use super::{parse_range_line, push_level_line, AllowedRanges, ClocksLevel, ClocksTable};
use crate::error::Error;
use crate::error::ErrorKind::ParseError;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{io::Write, str::FromStr};

/// Vega10 clocks table.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Table {
    /// List of core clock levels.
    pub sclk_levels: Vec<ClocksLevel>,
    /// List of memory clock levels.
    pub mclk_levels: Vec<ClocksLevel>,
    /// The allowed ranges for clockspeeds and voltages.
    pub allowed_ranges: AllowedRanges,
}

impl ClocksTable for Table {
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        for (i, level) in self.sclk_levels.iter().enumerate() {
            let command = level_command(*level, i, 's');
            writer.write_all(command.as_bytes())?;
        }

        for (i, level) in self.mclk_levels.iter().enumerate() {
            let command = level_command(*level, i, 'm');
            writer.write_all(command.as_bytes())?;
        }

        Ok(())
    }

    fn get_max_sclk(&self) -> Option<u32> {
        self.sclk_levels.last().map(|level| level.clockspeed)
    }

    fn get_max_mclk(&self) -> Option<u32> {
        self.mclk_levels.last().map(|level| level.clockspeed)
    }
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
        for line in s.lines().map(str::trim).filter(|line| !line.is_empty()) {
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

fn level_command(level: ClocksLevel, i: usize, symbol: char) -> String {
    let ClocksLevel {
        clockspeed,
        voltage,
    } = level;
    format!("{symbol} {i} {clockspeed} {voltage}\n")
}

#[derive(PartialEq)]
enum Section {
    Sclk,
    Mclk,
    Range,
}

#[cfg(test)]
mod tests {
    use super::{ClocksLevel, Table};
    use crate::gpu_handle::overdrive::{arr_commands, AllowedRanges, ClocksTable, Range};
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    const TABLE_RX580: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/rx580/pp_od_clk_voltage"
    ));

    #[test]
    fn parse_full_table() {
        let table = Table::from_str(TABLE_RX580).unwrap();

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
            sclk: Range::full(300, 2000),
            mclk: Some(Range::full(300, 2250)),
            vddc: Some(Range::full(750, 1200)),
        };

        assert_eq!(table.sclk_levels, sclk_levels);
        assert_eq!(table.mclk_levels, mclk_levels);
        assert_eq!(table.allowed_ranges, ranges);
    }

    #[test]
    fn table_into_commands() {
        let table = Table::from_str(TABLE_RX580).unwrap();
        let mut buf = Vec::new();
        table.write_commands(&mut buf).unwrap();
        let commands = String::from_utf8(buf).unwrap();

        let expected_commands = arr_commands([
            "s 0 300 750",
            "s 1 600 769",
            "s 2 900 912",
            "s 3 1145 1125",
            "s 4 1215 1150",
            "s 5 1257 1150",
            "s 6 1300 1150",
            "s 7 1366 1150",
            "m 0 300 750",
            "m 1 1000 825",
            "m 2 1750 975",
        ]);

        assert_eq!(expected_commands, commands);
    }

    #[test]
    fn max_clocks() {
        let table = Table::from_str(TABLE_RX580).unwrap();
        let sclk = table.get_max_sclk().unwrap();
        assert_eq!(sclk, 1366);
        let mclk = table.get_max_mclk().unwrap();
        assert_eq!(mclk, 1750);
    }
}
