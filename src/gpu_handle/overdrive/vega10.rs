//! The format used by Vega10 and older GPUs.
use super::{parse_range_line, push_level_line, ClocksLevel, ClocksTable, Range};
use crate::{
    error::{Error, ErrorKind::ParseError},
    Result,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{cmp, io::Write, str::FromStr};

/// Vega10 clocks table.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Table {
    /// List of core clock levels.
    pub sclk_levels: Vec<ClocksLevel>,
    /// List of memory clock levels.
    pub mclk_levels: Vec<ClocksLevel>,
    /// The allowed ranges for clockspeeds and voltages.
    pub od_range: OdRange,
}

impl ClocksTable for Table {
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<()> {
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

    fn get_max_sclk_range(&self) -> Option<Range> {
        Some(self.od_range.sclk)
    }

    fn get_min_sclk_range(&self) -> Option<Range> {
        Some(self.od_range.sclk)
    }

    fn get_max_mclk_range(&self) -> Option<Range> {
        self.od_range.mclk
    }

    fn get_min_mclk_range(&self) -> Option<Range> {
        self.od_range.mclk
    }

    fn get_max_voltage_range(&self) -> Option<Range> {
        self.od_range.vddc
    }

    fn get_min_voltage_range(&self) -> Option<Range> {
        self.od_range.vddc
    }

    fn get_current_voltage_range(&self) -> Option<Range> {
        let min = self.sclk_levels.first().map(|level| level.voltage);
        let max = self.sclk_levels.last().map(|level| level.voltage);
        Some(Range { min, max })
    }

    fn get_current_sclk_range(&self) -> Range {
        let min = self.sclk_levels.first().map(|level| level.clockspeed);
        let max = self.sclk_levels.last().map(|level| level.clockspeed);
        Range { min, max }
    }

    fn get_current_mclk_range(&self) -> Range {
        let min = self.mclk_levels.first().map(|level| level.clockspeed);
        let max = self.mclk_levels.last().map(|level| level.clockspeed);
        Range { min, max }
    }

    fn set_max_sclk_unchecked(&mut self, clockspeed: i32) -> Result<()> {
        let len = self.sclk_levels.len();
        if len == 0 {
            return Ok(());
        }

        self.sclk_levels[len - 1].clockspeed = clockspeed;
        for clock_level in &mut self.sclk_levels[0..len - 1] {
            clock_level.clockspeed = cmp::min(clock_level.clockspeed, clockspeed);
        }

        Ok(())
    }

    fn set_min_sclk_unchecked(&mut self, clockspeed: i32) -> Result<()> {
        let len = self.sclk_levels.len();
        if len == 0 {
            return Ok(());
        }

        self.sclk_levels[0].clockspeed = clockspeed;
        for clock_level in &mut self.sclk_levels[1..len] {
            clock_level.clockspeed = cmp::max(clock_level.clockspeed, clockspeed);
        }

        Ok(())
    }

    fn set_max_mclk_unchecked(&mut self, clockspeed: i32) -> Result<()> {
        let len = self.mclk_levels.len();
        if len == 0 {
            return Ok(());
        }

        self.mclk_levels[len - 1].clockspeed = clockspeed;
        for clock_level in &mut self.mclk_levels[0..len - 1] {
            clock_level.clockspeed = cmp::min(clock_level.clockspeed, clockspeed);
        }

        Ok(())
    }

    fn set_min_mclk_unchecked(&mut self, clockspeed: i32) -> Result<()> {
        let len = self.mclk_levels.len();
        if len == 0 {
            return Ok(());
        }

        self.mclk_levels[0].clockspeed = clockspeed;
        for clock_level in &mut self.mclk_levels[1..len] {
            clock_level.clockspeed = cmp::max(clock_level.clockspeed, clockspeed);
        }

        Ok(())
    }

    fn set_max_voltage_unchecked(&mut self, voltage: i32) -> Result<()> {
        let len = self.sclk_levels.len();
        if len == 0 {
            return Ok(());
        }

        self.sclk_levels[len - 1].voltage = voltage;
        for clock_level in &mut self.sclk_levels[0..len - 1] {
            clock_level.voltage = cmp::min(clock_level.voltage, voltage);
        }

        Ok(())
    }

    fn set_min_voltage_unchecked(&mut self, voltage: i32) -> Result<()> {
        let len = self.sclk_levels.len();
        if len == 0 {
            return Ok(());
        }

        self.sclk_levels[0].voltage = voltage;
        for clock_level in &mut self.sclk_levels[1..len - 1] {
            clock_level.voltage = cmp::max(clock_level.voltage, voltage);
        }

        Ok(())
    }

    fn get_max_sclk_voltage(&self) -> Option<i32> {
        self.sclk_levels.last().map(|level| level.voltage)
    }
}

/// The ranges for overclocking values which the GPU allows to be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OdRange {
    /// Clocks range for sclk (in MHz). Should be present on all GPUs.
    pub sclk: Range,
    /// Clocks range for mclk (in MHz). Present on discrete GPUs only.
    pub mclk: Option<Range>,
    /// Voltage range (in mV). Present on Vega10 and older GPUs only.
    pub vddc: Option<Range>,
}

impl FromStr for Table {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
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

        let od_range = OdRange {
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
            od_range,
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
    use crate::{
        gpu_handle::overdrive::{arr_commands, vega10::OdRange, ClocksTable, Range},
        include_table,
    };
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    const TABLE_RX580: &str = include_table!("rx580");

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
        let ranges = OdRange {
            sclk: Range::full(300, 2000),
            mclk: Some(Range::full(300, 2250)),
            vddc: Some(Range::full(750, 1200)),
        };

        assert_eq!(table.sclk_levels, sclk_levels);
        assert_eq!(table.mclk_levels, mclk_levels);
        assert_eq!(table.od_range, ranges);
    }

    #[test]
    fn table_into_commands() {
        let mut table = Table::from_str(TABLE_RX580).unwrap();

        table.set_max_sclk(1500).unwrap();
        table.set_max_mclk(2250).unwrap();

        table.set_min_sclk(350).unwrap();
        table.set_min_mclk(360).unwrap();

        table.set_min_voltage(800).unwrap();
        table.set_max_voltage(1200).unwrap();

        let mut buf = Vec::new();
        table.write_commands(&mut buf).unwrap();
        let commands = String::from_utf8(buf).unwrap();

        let expected_commands = arr_commands([
            "s 0 350 800",
            "s 1 600 800",
            "s 2 900 912",
            "s 3 1145 1125",
            "s 4 1215 1150",
            "s 5 1257 1150",
            "s 6 1300 1150",
            "s 7 1500 1200",
            "m 0 360 750",
            "m 1 1000 825",
            "m 2 2250 975",
        ]);

        assert_eq!(expected_commands, commands);
    }

    #[test]
    fn generic_actions() {
        let mut table = Table::from_str(TABLE_RX580).unwrap();
        let sclk = table.get_max_sclk().unwrap();
        assert_eq!(sclk, 1366);
        let mclk = table.get_max_mclk().unwrap();
        assert_eq!(mclk, 1750);
        let voltage = table.get_max_sclk_voltage().unwrap();
        assert_eq!(voltage, 1150);

        table.set_max_sclk(1400).unwrap();
        let sclk = table.get_max_sclk().unwrap();
        assert_eq!(sclk, 1400);
        assert_eq!(table.sclk_levels[7].clockspeed, 1400);

        table.set_max_mclk(1800).unwrap();
        let mclk = table.get_max_mclk().unwrap();
        assert_eq!(mclk, 1800);
        assert_eq!(table.mclk_levels[2].clockspeed, 1800);

        let sclk_range = table.get_max_sclk_range();
        let mclk_range = table.get_max_mclk_range();
        let voltage_range = table.get_max_voltage_range();
        assert_eq!(sclk_range, Some(Range::full(300, 2000)));
        assert_eq!(mclk_range, Some(Range::full(300, 2250)));
        assert_eq!(voltage_range, Some(Range::full(750, 1200)));
    }

    #[test]
    fn undervolt_normalize() {
        let mut table = Table::from_str(TABLE_RX580).unwrap();
        table.set_max_voltage(1100).unwrap();
        assert!(table.sclk_levels.iter().all(|level| level.voltage <= 1100));
    }

    #[test]
    fn underclock_normalize() {
        let mut table = Table::from_str(TABLE_RX580).unwrap();
        table.set_max_sclk(1200).unwrap();
        assert!(table
            .sclk_levels
            .iter()
            .all(|level| level.clockspeed <= 1200));
    }

    #[test]
    fn underclock_memory_normalize() {
        let mut table = Table::from_str(TABLE_RX580).unwrap();
        table.set_max_mclk(900).unwrap();
        assert!(table
            .mclk_levels
            .iter()
            .all(|level| level.clockspeed <= 900));
    }

    #[test]
    fn min_voltage_normalize() {
        let mut table = Table::from_str(TABLE_RX580).unwrap();
        table.set_min_voltage(800).unwrap();
        assert!(table.sclk_levels.iter().all(|level| level.voltage >= 800));
    }

    #[test]
    fn min_clockspeed_normalize() {
        let mut table = Table::from_str(TABLE_RX580).unwrap();
        table.set_min_sclk(750).unwrap();
        assert!(table
            .sclk_levels
            .iter()
            .all(|level| level.clockspeed >= 750));
    }

    #[test]
    fn min_memory_clockspeed_normalize() {
        let mut table = Table::from_str(TABLE_RX580).unwrap();
        table.set_min_mclk(1100).unwrap();
        assert!(table
            .mclk_levels
            .iter()
            .all(|level| level.clockspeed >= 1100));
    }
}
