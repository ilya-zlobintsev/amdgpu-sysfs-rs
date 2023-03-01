use crate::{error::Error, gpu_handle::trim_sysfs_line, Result};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Basic table format, used by internal GPUs (and potentially older desktop ones?)
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BasicTable {
    /// List of available modes
    pub modes: BTreeMap<usize, String>,
    /// The currently active mode
    pub active: usize,
}

impl BasicTable {
    pub(crate) fn parse(s: &str) -> Result<Self> {
        let mut modes = BTreeMap::new();
        let mut active = None;

        for (line, row) in s.lines().map(trim_sysfs_line).enumerate() {
            let mut split = row.split_whitespace();

            let index: usize = split
                .next()
                .ok_or_else(|| Error::unexpected_eol("index", line))?
                .parse()?;

            let mut name = split
                .next()
                .ok_or_else(|| Error::unexpected_eol("name", line))?;

            if let Some(active_name) = name.strip_suffix('*') {
                name = active_name;
                active = Some(index);
            }

            modes.insert(index, name.to_owned());
        }

        Ok(Self {
            modes,
            active: active.ok_or_else(|| Error::basic_parse_error("No active level found"))?,
        })
    }
}
