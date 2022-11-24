use super::{
    parse_level_line, parse_line_item, parse_range_line, push_level_line, AllowedRanges,
    ClocksLevel, PowerTable, Range,
};
use crate::{
    error::{Error, ErrorKind::ParseError},
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{io::Write, str::FromStr};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Table {
    pub current_sclk_range: Range,
    pub current_mclk_range: Range,
    pub vddc_curve: Vec<ClocksLevel>,
    pub allowed_levels: AllowedRanges,
}

impl PowerTable for Table {
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<()> {
        todo!()
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
                vddc_curve_line if line.starts_with("VDDC_CURVE_") => {
                    todo!()
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

        /*let sclk_range =

        Ok(Self {
            sclk_range,
            mclk_range,
            allowed_levels: todo!(),
        })*/
        todo!()
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
    let clockspeed = parse_line_item(&mut split, i, "clockspeed", &["MHz"])?;

    Ok((clockspeed, num))
}

fn parse_min_max_line(line: &str, i: usize, range: &mut Option<Range>) -> Result<()> {
    let (clockspeed, num) = parse_clockspeed_line(line, i)?;
    match num {
        0 => {
            *range = Some(Range {
                min: Some(clockspeed),
                max: None,
            });
            Ok(())
        }
        1 => match range {
            Some(range) => {
                range.max = Some(clockspeed);
                Ok(())
            }
            None => Err(ParseError {
                msg: "Found range max item with no proceeding min item".to_owned(),
                line: i,
            }
            .into()),
        },
        _ => Err(ParseError {
            msg: format!("Unexpected range number {num}"),
            line: i,
        }
        .into()),
    }
}
