//! The format used by Vega20 and newer GPUs.
use super::{parse_line_item, parse_range_line, push_level_line, ClocksLevel, ClocksTable, Range};
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
    /// Voltage offset(in mV) applied on target voltage calculation.
    /// This is available for Sienna Cichlid, Navy Flounder and Dimgrey Cavefish.
    pub voltage_offset: Option<u32>,
    /// The allowed ranges for clockspeeds and voltages.
    pub od_range: OdRange,
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
                let line = clockspeed_line(symbol, index, clockspeed);
                writer.write_all(line.as_bytes())?;
            }
        }

        for (i, level) in self.vddc_curve.iter().enumerate() {
            let line = vddc_curve_line(i, level.clockspeed, level.voltage);
            writer.write_all(line.as_bytes())?;
        }

        if let Some(offset) = self.voltage_offset {
            let line = voltage_offset_line(offset);
            writer.write_all(line.as_bytes())?;
        }

        Ok(())
    }

    fn get_max_sclk_range(&self) -> Option<Range> {
        self.od_range
            .voltage_points
            .last()
            .map(|point| point.sclk)
            .or(Some(self.od_range.sclk))
    }

    fn get_max_mclk_range(&self) -> Option<Range> {
        self.od_range.mclk
    }

    fn get_max_voltage_range(&self) -> Option<Range> {
        self.od_range
            .voltage_points
            .last()
            .map(|point| point.voltage)
    }

    fn get_max_sclk(&self) -> Option<u32> {
        self.current_sclk_range.max
    }

    fn set_max_sclk_unchecked(&mut self, clockspeed: u32) -> Result<()> {
        self.current_sclk_range.max = Some(clockspeed);
        if let Some(point) = self.vddc_curve.last_mut() {
            point.clockspeed = clockspeed;
        }
        Ok(())
    }

    fn get_max_mclk(&self) -> Option<u32> {
        self.current_mclk_range.max
    }

    fn set_max_mclk_unchecked(&mut self, clockspeed: u32) -> Result<()> {
        self.current_mclk_range.max = Some(clockspeed);
        Ok(())
    }

    fn set_max_voltage_unchecked(&mut self, voltage: u32) -> Result<()> {
        self.vddc_curve
            .last_mut()
            .ok_or_else(|| {
                Error::not_allowed("The GPU did not report any voltage curve points".to_owned())
            })?
            .voltage = voltage;
        Ok(())
    }

    fn get_max_sclk_voltage(&self) -> Option<u32> {
        self.vddc_curve.last().map(|level| level.voltage)
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

        let mut vddc_curve = Vec::with_capacity(3);
        let mut sclk_range_points = Vec::with_capacity(3);
        let mut volt_range_points = Vec::with_capacity(3);

        let mut voltage_offset = None;

        let mut i = 1;
        for line in s
            .lines()
            .map(|line| line.trim_matches(char::from(0)).trim())
            .filter(|line| !line.is_empty())
        {
            match line {
                "OD_SCLK:" => current_section = Some(Section::Sclk),
                "OD_MCLK:" => current_section = Some(Section::Mclk),
                "OD_RANGE:" => current_section = Some(Section::Range),
                "OD_VDDC_CURVE:" => current_section = Some(Section::VddcCurve),
                "OD_VDDGFX_OFFSET:" => current_section = Some(Section::VddGfxOffset),
                line => match current_section {
                    // Voltage points will overwrite maximum clock info, with the last one taking priority
                    Some(Section::Range) if line.starts_with("VDDC_CURVE_SCLK") => {
                        let (range, _) = parse_range_line(line, i)?;
                        sclk_range_points.push(range);
                    }
                    Some(Section::Range) if line.starts_with("VDDC_CURVE_VOLT") => {
                        let (range, _) = parse_range_line(line, i)?;
                        volt_range_points.push(range);
                    }
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
                    Some(Section::VddGfxOffset) => {
                        let offset = parse_voltage_offset_line(line, i)?;
                        voltage_offset = Some(offset);
                    }
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

        let voltage_points = sclk_range_points
            .into_iter()
            .zip(volt_range_points)
            .map(|(sclk, voltage)| VoltagePointRange { sclk, voltage })
            .collect();

        let od_range = OdRange {
            sclk: allowed_sclk_range.ok_or_else(|| ParseError {
                msg: "No sclk range found".to_owned(),
                line: i,
            })?,
            mclk: Some(allowed_mclk_range.ok_or_else(|| ParseError {
                msg: "No mclk range found".to_owned(),
                line: i,
            })?),
            voltage_points,
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
            current_sclk_range,
            current_mclk_range,
            vddc_curve,
            od_range,
            voltage_offset,
        })
    }
}

impl Table {
    /// Clears the table of all "applicable" values.
    ///
    /// This removes all values except the allowed range.
    /// You can use it to avoid overwriting the table with already present values, as it can be problematic on some cards.
    /// It is intended to be used before calling `set_*` functions and generating commands/writing the table.
    pub fn clear(&mut self) {
        self.current_sclk_range = Range::empty();
        self.current_mclk_range = Range::empty();
        self.vddc_curve.clear();
        self.voltage_offset = None;
    }
}

/// The ranges for overclocking values which the GPU allows to be used.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OdRange {
    /// Clocks range for sclk (in MHz). Should be present on all GPUs.
    pub sclk: Range,
    /// Clocks range for mclk (in MHz). Present on discrete GPUs only.
    pub mclk: Option<Range>,
    /// Ranges available at specific voltage points.
    pub voltage_points: Vec<VoltagePointRange>,
}

/// Range available at specific voltage points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VoltagePointRange {
    /// Core clock range.
    pub sclk: Range,
    /// Voltage range.
    pub voltage: Range,
}

enum Section {
    Sclk,
    Mclk,
    VddcCurve,
    Range,
    VddGfxOffset,
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

fn parse_voltage_offset_line(line: &str, i: usize) -> Result<u32> {
    match line.to_lowercase().strip_suffix("mv") {
        Some(raw_value) => Ok(raw_value.parse()?),
        None => Err(ParseError {
            msg: format!("Could not find expected `mV` suffix in offset line {line}"),
            line: i,
        }
        .into()),
    }
}

fn clockspeed_line(symbol: char, index: usize, clockspeed: u32) -> String {
    format!("{symbol} {index} {clockspeed}\n")
}

fn vddc_curve_line(index: usize, clockspeed: u32, voltage: u32) -> String {
    format!("vc {index} {clockspeed} {voltage}\n")
}

fn voltage_offset_line(offset: u32) -> String {
    format!("vo {offset}\n")
}

#[cfg(test)]
mod tests {
    use super::{OdRange, Table};
    use crate::{
        gpu_handle::overdrive::{
            arr_commands, vega20::VoltagePointRange, ClocksLevel, ClocksTable, Range,
        },
        include_table,
    };
    use insta::assert_yaml_snapshot;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    const TABLE_5700XT: &str = include_table!("rx5700xt");
    const TABLE_6900XT: &str = include_table!("rx6900xt");
    const TABLE_6700XT: &str = include_table!("rx6700xt");

    #[test]
    fn parse_5700xt_full() {
        let table = Table::from_str(TABLE_5700XT).unwrap();

        assert_eq!(table.current_sclk_range, Range::full(800, 2100));
        assert_eq!(table.current_mclk_range, Range::max(875));

        let vddc_curve = [(800, 711), (1450, 801), (2100, 1191)]
            .map(|(clockspeed, voltage)| ClocksLevel::new(clockspeed, voltage));
        assert_eq!(table.vddc_curve, vddc_curve);

        let voltage_points = vec![
            VoltagePointRange {
                sclk: Range::full(800, 2150),
                voltage: Range::full(750, 1200),
            },
            VoltagePointRange {
                sclk: Range::full(800, 2150),
                voltage: Range::full(750, 1200),
            },
            VoltagePointRange {
                sclk: Range::full(800, 2150),
                voltage: Range::full(750, 1200),
            },
        ];

        let od_range = OdRange {
            sclk: Range::full(800, 2150),
            mclk: Some(Range::full(625, 950)),
            voltage_points,
        };
        assert_eq!(table.od_range, od_range);
    }

    #[test]
    fn generic_actions_5700xt() {
        let mut table = Table::from_str(TABLE_5700XT).unwrap();
        assert_eq!(table.get_max_sclk(), Some(2100));
        assert_eq!(table.get_max_mclk(), Some(875));
        assert_eq!(table.get_max_sclk_voltage(), Some(1191));

        table.set_max_sclk(2050).unwrap();
        assert_eq!(table.get_max_sclk(), Some(2050));
        assert_eq!(table.current_sclk_range.max, Some(2050));

        table.set_max_mclk(950).unwrap();
        assert_eq!(table.get_max_mclk(), Some(950));
        assert_eq!(table.current_mclk_range.max, Some(950));

        let sclk_range = table.get_max_sclk_range();
        let mclk_range = table.get_max_mclk_range();
        let voltage_range = table.get_max_voltage_range();
        assert_eq!(sclk_range, Some(Range::full(800, 2150)));
        assert_eq!(mclk_range, Some(Range::full(625, 950)));
        assert_eq!(voltage_range, Some(Range::full(750, 1200)));
    }

    #[test]
    fn write_commands_5700xt() {
        let mut table = Table::from_str(TABLE_5700XT).unwrap();

        table.set_max_sclk(2150).unwrap();
        table.set_max_mclk(950).unwrap();
        table.set_max_voltage(1200).unwrap();

        let mut buf = Vec::new();
        table.write_commands(&mut buf).unwrap();
        let commands = String::from_utf8(buf).unwrap();

        let expected_commands = arr_commands([
            "s 0 800",
            "s 1 2150",
            "m 1 950",
            "vc 0 800 711",
            "vc 1 1450 801",
            "vc 2 2150 1200",
        ]);

        assert_eq!(expected_commands, commands);
    }

    #[test]
    fn write_commands_custom_5700xt() {
        let table = Table {
            current_sclk_range: Range::empty(),
            current_mclk_range: Range::full(500, 1000),
            vddc_curve: vec![ClocksLevel::new(300, 600), ClocksLevel::new(1000, 1000)],
            voltage_offset: None,
            od_range: OdRange {
                sclk: Range::empty(),
                mclk: None,
                voltage_points: Vec::new(),
            },
        };

        let mut buf = Vec::new();
        table.write_commands(&mut buf).unwrap();
        let commands = String::from_utf8(buf).unwrap();

        let expected_commands =
            arr_commands(["m 0 500", "m 1 1000", "vc 0 300 600", "vc 1 1000 1000"]);

        assert_eq!(expected_commands, commands);
    }

    #[test]
    fn parse_6900xt_full() {
        let table = Table::from_str(TABLE_6900XT).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn write_commands_6900xt_default() {
        let table = Table::from_str(TABLE_6900XT).unwrap();
        let commands = table.get_commands().unwrap();

        assert_yaml_snapshot!(commands);
    }

    #[test]
    fn parse_6700xt_full() {
        let table = Table::from_str(TABLE_6700XT).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn generic_actions_6700xt() {
        let table = Table::from_str(TABLE_6700XT).unwrap();

        let max_sclk = table.get_max_sclk().unwrap();
        assert_eq!(max_sclk, 2725);
        let sclk_range = table.get_max_sclk_range().unwrap();
        assert_eq!(sclk_range, Range::full(500, 2800));

        let max_mclk = table.get_max_mclk().unwrap();
        assert_eq!(max_mclk, 1000);
        let mclk_range = table.get_max_mclk_range().unwrap();
        assert_eq!(mclk_range, Range::full(674, 1075));

        assert!(table.get_max_sclk_voltage().is_none());
    }

    #[test]
    fn write_only_max_values_67900xt() {
        let mut table = Table::from_str(TABLE_6700XT).unwrap();

        table.clear();
        table.set_max_sclk(2800).unwrap();
        table.set_max_mclk(1075).unwrap();

        let commands = table.get_commands().unwrap();
        assert_yaml_snapshot!(commands);
    }
}
