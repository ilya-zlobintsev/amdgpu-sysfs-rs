//! `pp-power-profile-mode`
mod basic;
mod full;

use crate::Result;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub use basic::BasicTable;
pub use full::{FullTable, FullTableMode};

/// Table of predefined power profile modes with a list of GPU-specific heuristics

/// https://kernel.org/doc/html/latest/gpu/amdgpu/thermal.html#pp-power-profile-mode
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "data", rename_all = "snake_case")
)]
pub enum PowerProfileModesTable {
    /// The full table format, used by dedicated GPUs
    Full(FullTable),
    /// Basic table format
    Basic(BasicTable),
}

impl PowerProfileModesTable {
    /// Parse the table from a given string
    pub fn parse(s: &str) -> Result<Self> {
        // The basic format starts with a number
        if s.split_whitespace()
            .next()
            .and_then(|item| item.parse::<usize>().ok())
            .is_some()
        {
            BasicTable::parse(s).map(Self::Basic)
        } else {
            FullTable::parse(s).map(Self::Full)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PowerProfileModesTable;
    use insta::assert_yaml_snapshot;

    const TABLE_VEGA56: &str = include_test_data!("vega56/pp_power_profile_mode");
    const TABLE_RX580: &str = include_test_data!("rx580/pp_power_profile_mode");
    const TABLE_4800H: &str = include_test_data!("internal-4800h/pp_power_profile_mode");

    #[test]
    fn parse_full_vega56() {
        let table = PowerProfileModesTable::parse(TABLE_VEGA56).unwrap();
        assert_yaml_snapshot!(table, {
            ".data.modes[].heuristics" => insta::sorted_redaction()
        });
    }

    #[test]
    fn parse_full_rx580() {
        let table = PowerProfileModesTable::parse(TABLE_RX580).unwrap();
        assert_yaml_snapshot!(table, {
            ".data.modes[].heuristics" => insta::sorted_redaction()
        });
    }

    #[test]
    fn parse_full_internal_4800h() {
        let table = PowerProfileModesTable::parse(TABLE_4800H).unwrap();
        assert_yaml_snapshot!(table);
    }
}
