//! The format used by Vega20 and newer GPUs.
use super::{
    parse_line_item, parse_range_line, push_level_line, ClocksLevel, ClocksTable, ClocksTableGen,
    Range,
};
use crate::{
    error::{Error, ErrorContext, ErrorKind::ParseError},
    gpu_handle::trim_sysfs_line,
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{cmp, io::Write, str::FromStr};

/// Vega20 clocks table.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Table {
    /// The current core clock range.
    pub current_sclk_range: Range,
    /// The current memory clock range. Empty on iGPUs.
    pub current_mclk_range: Range,
    /// The current voltage curve. May be empty if the GPU does not support it.
    pub vddc_curve: Vec<ClocksLevel>,
    /// Voltage offset(in mV) applied on target voltage calculation.
    /// This is available for Sienna Cichlid, Navy Flounder and Dimgrey Cavefish.
    ///
    /// Note: editing this value directly does not check if it's in the allowed range!
    pub voltage_offset: Option<i32>,
    /// The allowed ranges for clockspeeds and voltages.
    pub od_range: OdRange,
}

impl ClocksTable for Table {
    fn write_commands<W: Write>(
        &self,
        writer: &mut W,
        previous_table: &ClocksTableGen,
    ) -> Result<()> {
        let ClocksTableGen::Vega20(previous_table) = previous_table else {
            return Err(Error::not_allowed(
                "Mismatched clocks table format".to_owned(),
            ));
        };

        let mut clocks_commands = Vec::with_capacity(4);

        // If the new minimum clockspeed is higher than the previous maximum clockspeed,
        // we need to first write the new maximum value to avoid an error on RDNA3
        if let (Some(current_sclk_min), Some(old_sclk_max)) = (
            self.current_sclk_range.min,
            previous_table.current_sclk_range.max,
        ) {
            if current_sclk_min > old_sclk_max {
                clocks_commands.push((self.current_sclk_range.max, 's', 1));
            }
        }

        clocks_commands.extend([
            (self.current_sclk_range.min, 's', 0),
            (self.current_sclk_range.max, 's', 1),
        ]);

        if let (Some(current_mclk_min), Some(old_mclk_max)) = (
            self.current_mclk_range.min,
            previous_table.current_mclk_range.max,
        ) {
            if current_mclk_min > old_mclk_max {
                clocks_commands.push((self.current_mclk_range.max, 'm', 1));
            }
        }

        clocks_commands.extend([
            (self.current_mclk_range.min, 'm', 0),
            (self.current_mclk_range.max, 'm', 1),
        ]);

        for (maybe_clockspeed, symbol, index) in clocks_commands {
            if let Some(clockspeed) = maybe_clockspeed {
                let line = clockspeed_line(symbol, index, clockspeed);
                writer
                    .write_all(line.as_bytes())
                    .with_context(|| format!("Error when writing clockspeed line `{line}`"))?;
            }
        }

        for (i, level) in self.vddc_curve.iter().enumerate() {
            let line = vddc_curve_line(i, level.clockspeed, level.voltage);
            writer
                .write_all(line.as_bytes())
                .with_context(|| format!("Error when writing VDDC line `{line}`"))?;
        }

        if let Some(offset) = self.voltage_offset {
            let line = voltage_offset_line(offset);
            writer
                .write_all(line.as_bytes())
                .with_context(|| format!("Error when writing voltage offset `{line}`"))?;
        }

        Ok(())
    }

    fn get_max_sclk_range(&self) -> Option<Range> {
        self.od_range
            .curve_sclk_points
            .last()
            .copied()
            .or(Some(self.od_range.sclk))
    }

    fn get_min_sclk_range(&self) -> Option<Range> {
        self.od_range
            .curve_sclk_points
            .first()
            .copied()
            .or(Some(self.od_range.sclk))
    }

    fn get_max_mclk_range(&self) -> Option<Range> {
        self.od_range.mclk
    }

    fn get_min_mclk_range(&self) -> Option<Range> {
        self.od_range.mclk
    }

    fn get_max_voltage_range(&self) -> Option<Range> {
        self.od_range.curve_voltage_points.last().copied()
    }

    fn get_min_voltage_range(&self) -> Option<Range> {
        self.od_range.curve_voltage_points.first().copied()
    }

    fn get_current_voltage_range(&self) -> Option<Range> {
        let min = self.vddc_curve.first().map(|level| level.voltage)?;
        let max = self.vddc_curve.last().map(|level| level.voltage)?;
        Some(Range::full(min, max))
    }

    fn get_current_sclk_range(&self) -> Range {
        self.current_sclk_range
    }

    fn get_current_mclk_range(&self) -> Range {
        self.current_mclk_range
    }

    fn set_max_sclk_unchecked(&mut self, clockspeed: i32) -> Result<()> {
        self.current_sclk_range.max = Some(clockspeed);
        if let Some(point) = self.vddc_curve.last_mut() {
            point.clockspeed = clockspeed;
        }
        Ok(())
    }

    fn set_min_sclk_unchecked(&mut self, clockspeed: i32) -> Result<()> {
        self.current_sclk_range.min = Some(clockspeed);
        if let Some(point) = self.vddc_curve.first_mut() {
            point.clockspeed = clockspeed;
        }
        Ok(())
    }

    fn set_max_mclk_unchecked(&mut self, clockspeed: i32) -> Result<()> {
        self.current_mclk_range.max = Some(clockspeed);
        Ok(())
    }

    fn set_min_mclk_unchecked(&mut self, clockspeed: i32) -> Result<()> {
        self.current_mclk_range.min = Some(clockspeed);
        Ok(())
    }

    fn set_max_voltage_unchecked(&mut self, voltage: i32) -> Result<()> {
        self.vddc_curve
            .last_mut()
            .ok_or_else(|| {
                Error::not_allowed("The GPU did not report any voltage curve points".to_owned())
            })?
            .voltage = voltage;
        Ok(())
    }

    fn set_min_voltage_unchecked(&mut self, voltage: i32) -> Result<()> {
        self.vddc_curve
            .first_mut()
            .ok_or_else(|| {
                Error::not_allowed("The GPU did not report any voltage curve points".to_owned())
            })?
            .voltage = voltage;
        Ok(())
    }

    fn get_max_sclk_voltage(&self) -> Option<i32> {
        self.vddc_curve.last().map(|level| level.voltage)
    }
}

impl Table {
    /// Sets the voltage offset, checking if it's in range if the GPU provided one
    ///
    /// Note: RDNA2 GPUs use a voltage offset but do not provide a range
    pub fn set_voltage_offset(&mut self, offset: i32) -> Result<()> {
        if let Some(offset_range) = self.od_range.voltage_offset {
            if let Some((min, max)) = offset_range.into_full() {
                if !(min..=max).contains(&offset) {
                    return Err(Error::not_allowed(format!("Provided voltage offset {offset} is out of range, should be between {min} and {max}")));
                }
            }
        }

        self.voltage_offset = Some(offset);
        Ok(())
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
        let mut curve_sclk_points = Vec::with_capacity(3);
        let mut curve_voltage_points = Vec::with_capacity(3);

        let mut voltage_offset = None;
        let mut voltage_offset_range = None;

        let mut i = 1;
        for line in s
            .lines()
            .map(trim_sysfs_line)
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
                        curve_sclk_points.push(range);
                    }
                    Some(Section::Range)
                        if line.starts_with("VDDC_CURVE_VOLT")
                            || (line.starts_with("VDDC_CURVE:") && line.contains("mv")) =>
                    {
                        let (range, _) = parse_range_line(line, i)?;
                        curve_voltage_points.push(range);
                    }
                    Some(Section::Range) => {
                        let (range, name) = parse_range_line(line, i)?;
                        match name {
                            "SCLK" => allowed_sclk_range = Some(range),
                            "MCLK" => allowed_mclk_range = Some(range),
                            "VDDGFX_OFFSET" => voltage_offset_range = Some(range),
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
                    Some(Section::VddcCurve) => {
                        let _ = push_level_line(line, &mut vddc_curve, i);
                    }
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

        let od_range = OdRange {
            sclk: allowed_sclk_range.ok_or_else(|| ParseError {
                msg: "No sclk range found".to_owned(),
                line: i,
            })?,
            mclk: allowed_mclk_range,
            curve_sclk_points,
            curve_voltage_points,
            voltage_offset: voltage_offset_range,
        };
        let current_sclk_range = current_sclk_range.ok_or_else(|| ParseError {
            msg: "No current sclk range found".to_owned(),
            line: i,
        })?;

        Ok(Self {
            current_sclk_range,
            current_mclk_range: current_mclk_range.unwrap_or_else(Range::empty),
            vddc_curve,
            od_range,
            voltage_offset,
        })
    }
}

impl Table {
    /// Clears the table of all "applicable" values.
    ///
    /// This removes all values except the allowed range and voltage curve.
    /// You can use it to avoid overwriting the table with already present values, as it can be problematic on some cards.
    /// It is intended to be used before calling `set_*` functions and generating commands/writing the table.
    pub fn clear(&mut self) {
        self.current_sclk_range = Range::empty();
        self.current_mclk_range = Range::empty();
        self.voltage_offset = None;
    }

    /// Normalizes the VDDC curve making sure all of the values are within the allowed range.
    /// This is needed as some GPUs have default values outside of the allowed range.
    pub fn normalize_vddc_curve(&mut self) {
        for (i, point) in self.vddc_curve.iter_mut().enumerate() {
            if let Some(sclk_range) = self.od_range.curve_sclk_points.get(i) {
                let normalized_clockspeed = normalize_value(point.clockspeed, *sclk_range);
                point.clockspeed = normalized_clockspeed;
            }

            if let Some(voltage_range) = self.od_range.curve_voltage_points.get(i) {
                let normalized_voltage = normalize_value(point.voltage, *voltage_range);
                point.voltage = normalized_voltage;
            }
        }
    }
}

fn normalize_value(mut value: i32, range: Range) -> i32 {
    if let Some(min_allowed) = range.min {
        value = cmp::max(min_allowed, value);
    }
    if let Some(max_allowed) = range.max {
        value = cmp::min(max_allowed, value);
    }

    value
}

/// The ranges for overclocking values which the GPU allows to be used.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OdRange {
    /// Clocks range for sclk (in MHz). Should be present on all GPUs.
    pub sclk: Range,
    /// Clocks range for mclk (in MHz). Present on discrete GPUs only.
    pub mclk: Option<Range>,
    /// Frequencies available at specific levels.
    pub curve_sclk_points: Vec<Range>,
    /// Ranges available at specific levels.
    pub curve_voltage_points: Vec<Range>,
    /// Allowed voltage offset range. Present on RDNA3+.
    pub voltage_offset: Option<Range>,
}

#[derive(Debug)]
enum Section {
    Sclk,
    Mclk,
    VddcCurve,
    Range,
    VddGfxOffset,
}

fn parse_clockspeed_line(line: &str, i: usize) -> Result<(i32, usize)> {
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

fn parse_voltage_offset_line(line: &str, i: usize) -> Result<i32> {
    match line.to_lowercase().strip_suffix("mv") {
        Some(raw_value) => Ok(raw_value.parse()?),
        None => Err(ParseError {
            msg: format!("Could not find expected `mV` suffix in offset line {line}"),
            line: i,
        }
        .into()),
    }
}

fn clockspeed_line(symbol: char, index: usize, clockspeed: i32) -> String {
    format!("{symbol} {index} {clockspeed}\n")
}

fn vddc_curve_line(index: usize, clockspeed: i32, voltage: i32) -> String {
    format!("vc {index} {clockspeed} {voltage}\n")
}

fn voltage_offset_line(offset: i32) -> String {
    format!("vo {offset}\n")
}

#[cfg(test)]
mod tests {
    use super::{OdRange, Table};
    use crate::{
        gpu_handle::overdrive::{arr_commands, ClocksLevel, ClocksTable, Range},
        include_table,
    };
    use insta::assert_yaml_snapshot;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    const TABLE_5500XT: &str = include_table!("rx5500xt");
    const TABLE_5700XT: &str = include_table!("rx5700xt");
    const TABLE_6900XT: &str = include_table!("rx6900xt");
    const TABLE_6700XT: &str = include_table!("rx6700xt");
    const TABLE_6800: &str = include_table!("rx6800");
    const TABLE_7900XTX: &str = include_table!("rx7900xtx");
    const TABLE_7900XT: &str = include_table!("rx7900xt");
    const TABLE_7800XT: &str = include_table!("rx7800xt");
    const TABLE_PHOENIX: &str = include_table!("internal-7840u");

    #[test]
    fn parse_5700xt_full() {
        let table = Table::from_str(TABLE_5700XT).unwrap();

        assert_eq!(table.current_sclk_range, Range::full(800, 2100));
        assert_eq!(table.current_mclk_range, Range::max(875));

        let vddc_curve = [(800, 711), (1450, 801), (2100, 1191)]
            .map(|(clockspeed, voltage)| ClocksLevel::new(clockspeed, voltage));
        assert_eq!(table.vddc_curve, vddc_curve);

        let curve_sclk_points = vec![
            Range::full(800, 2150),
            Range::full(800, 2150),
            Range::full(800, 2150),
        ];
        let curve_voltage_points = vec![
            Range::full(750, 1200),
            Range::full(750, 1200),
            Range::full(750, 1200),
        ];

        let od_range = OdRange {
            sclk: Range::full(800, 2150),
            mclk: Some(Range::full(625, 950)),
            curve_sclk_points,
            curve_voltage_points,
            voltage_offset: None,
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

        table.set_max_voltage(1150).unwrap();
        assert_eq!(table.vddc_curve[2].voltage, 1150);

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
        table.set_min_sclk(850).unwrap();
        table.set_max_mclk(950).unwrap();
        table.set_max_voltage(1200).unwrap();

        let mut buf = Vec::new();
        table
            .write_commands(&mut buf, &table.clone().into())
            .unwrap();
        let commands = String::from_utf8(buf).unwrap();

        let expected_commands = arr_commands([
            "s 0 850",
            "s 1 2150",
            "m 1 950",
            "vc 0 850 711",
            "vc 1 1450 801",
            "vc 2 2150 1200",
        ]);

        assert_eq!(expected_commands, commands);
    }

    #[test]
    fn normalize_vddc_curve_5700xt() {
        let mut table = Table::from_str(TABLE_5700XT).unwrap();
        let voltage_range = table.od_range.curve_voltage_points[0];
        // The table has points outside of the allowed range by default
        assert!(table.vddc_curve.iter().any(|level| {
            level.voltage < voltage_range.min.unwrap() || level.voltage > voltage_range.max.unwrap()
        }));

        table.normalize_vddc_curve();

        // The table does not have any points outside of the allowed range after normalization
        assert!(!table.vddc_curve.iter().any(|level| {
            level.voltage < voltage_range.min.unwrap() || level.voltage > voltage_range.max.unwrap()
        }));
        assert_eq!(750, table.vddc_curve[0].voltage);
    }

    #[test]
    fn write_commands_5500xt() {
        let mut table = Table::from_str(TABLE_5500XT).unwrap();
        table.clear();
        table.set_max_sclk(1900).unwrap();
        table.set_max_voltage(1140).unwrap();

        let commands = table.get_commands(&table.clone().into()).unwrap();
        let expected_commands = vec![
            "s 1 1900",
            "vc 0 500 710",
            "vc 1 1162 794",
            "vc 2 1900 1140",
        ];
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
                curve_sclk_points: Vec::new(),
                curve_voltage_points: Vec::new(),
                voltage_offset: None,
            },
        };

        let mut buf = Vec::new();
        table
            .write_commands(&mut buf, &table.clone().into())
            .unwrap();
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
        let commands = table.get_commands(&table.clone().into()).unwrap();

        assert_yaml_snapshot!(commands);
    }

    #[test]
    fn write_commands_6900xt_custom() {
        let mut table = Table::from_str(TABLE_6900XT).unwrap();
        table.clear();

        table.set_min_sclk(800).unwrap();
        table.set_max_sclk(2400).unwrap();
        table.set_max_mclk(900).unwrap();
        assert!(table.set_min_voltage(1000).is_err());

        let commands = table.get_commands(&table.clone().into()).unwrap();
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

        let current_sclk_range = table.get_current_sclk_range();
        assert_eq!(current_sclk_range, Range::full(500, 2725));

        let current_mclk_range = table.get_current_mclk_range();
        assert_eq!(current_mclk_range, Range::full(97, 1000));
    }

    #[test]
    fn write_only_max_values_6700xt() {
        let mut table = Table::from_str(TABLE_6700XT).unwrap();

        table.clear();
        table.set_max_sclk(2800).unwrap();
        table.set_max_mclk(1075).unwrap();

        let commands = table.get_commands(&table.clone().into()).unwrap();
        assert_yaml_snapshot!(commands);
    }

    #[test]
    fn write_new_min_over_old_max_7900xt() {
        let original_table = Table::from_str(TABLE_7900XT).unwrap();

        let mut new_table = original_table.clone();
        new_table.clear();

        new_table.set_min_mclk(1350).unwrap();
        new_table.set_max_mclk(1350).unwrap();

        new_table.set_min_sclk(3000).unwrap();
        new_table.set_max_sclk(3000).unwrap();

        let commands = new_table.get_commands(&original_table.into()).unwrap();
        assert_yaml_snapshot!(commands);
    }

    #[test]
    fn parse_6800_full() {
        let table = Table::from_str(TABLE_6800).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn set_max_values_6800() {
        let mut table = Table::from_str(TABLE_6800).unwrap();

        table.clear();
        table.set_max_sclk(2400).unwrap();
        assert!(table.set_max_sclk(2700).is_err());
        table.set_max_mclk(1050).unwrap();
        table.voltage_offset = Some(10);

        assert_yaml_snapshot!(table.get_commands(&table.clone().into()).unwrap());
    }

    #[test]
    fn parse_7900xtx_full() {
        let table = Table::from_str(TABLE_7900XTX).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn parse_7900xt_full() {
        let table = Table::from_str(TABLE_7900XT).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn parse_7800xt_full() {
        let table = Table::from_str(TABLE_7800XT).unwrap();
        assert_yaml_snapshot!(table);
    }

    #[test]
    fn set_7800xt_voltage() {
        let mut table = Table::from_str(TABLE_7800XT).unwrap();
        table.set_voltage_offset(-300).unwrap();
        table.set_voltage_offset(100).unwrap_err();
    }

    #[test]
    fn parse_phoenix_full() {
        let table = Table::from_str(TABLE_PHOENIX).unwrap();
        assert_yaml_snapshot!(table);
    }
}
