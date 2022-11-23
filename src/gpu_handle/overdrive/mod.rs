pub mod gen1;

use crate::{error::Error, Result};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufWriter, Write},
    str::FromStr,
};

pub trait PowerTable: FromStr {
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<()>;
}

/// Representation of `pp_od_clk_voltage`
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PowerTableGen {
    Gen1(gen1::Table),
}

impl FromStr for PowerTableGen {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        // TODO: type detection
        gen1::Table::from_str(s).map(Self::Gen1)
    }
}

impl PowerTable for PowerTableGen {
    fn write_commands<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            PowerTableGen::Gen1(table) => table.write_commands(writer),
        }
    }
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
    pub min: u32,
    pub max: u32,
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
