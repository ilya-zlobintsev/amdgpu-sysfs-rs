pub mod gen1;

use crate::error::Error;
use std::str::FromStr;

/// Representation of `pp_od_clk_voltage`
#[derive(Debug, Clone)]
pub enum PowerTable {
    Gen1(gen1::Table),
}

impl FromStr for PowerTable {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: type detection
        gen1::Table::from_str(s).map(PowerTable::Gen1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowedRanges {
    /// Clocks range for sclk (in MHz). Should be present on all GPUs.
    pub sclk: Range,
    /// Clocks range for mclk (in MHz). Present on discrete GPUs only.
    pub mclk: Option<Range>,
    /// Voltage range (in mV). Present on Vega10 and older GPUs only.
    pub vddc: Option<Range>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub min: u32,
    pub max: u32,
}
