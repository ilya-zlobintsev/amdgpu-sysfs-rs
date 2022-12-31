#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// List of power levels.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PowerLevels<T> {
    /// List of possible levels.
    pub levels: Vec<T>,
    /// The currently active level.
    pub active: Option<usize>,
}

impl<T> PowerLevels<T> {
    /// Gets the currently active level value.
    pub fn active_level(&self) -> Option<&T> {
        self.active.and_then(|active| self.levels.get(active))
    }
}

macro_rules! impl_get_clocks_levels {
    ($name:ident, $level:expr, $out:ty) => {
        /// Gets clocks levels.
        pub fn $name(&self) -> Result<PowerLevels<$out>> {
            self.get_clock_levels($level)
        }
    };
}

/// Type of a power level.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PowerLevelKind {
    CoreClock,
    MemoryClock,
    SOCClock,
    FabricClock,
    DCEFClock,
    PcieSpeed,
}

impl PowerLevelKind {
    /// Gets the filename of a given power level kind.
    pub fn filename(&self) -> &str {
        use PowerLevelKind::*;
        match self {
            CoreClock => "pp_dpm_sclk",
            MemoryClock => "pp_dpm_mclk",
            SOCClock => "pp_dpm_socclk",
            FabricClock => "pp_dpm_fclk",
            DCEFClock => "pp_dpm_dcefclk",
            PcieSpeed => "pp_dpm_pcie",
        }
    }

    /// Suffix of the power level value
    pub fn value_suffix(&self) -> Option<&str> {
        use PowerLevelKind::*;
        match self {
            CoreClock | MemoryClock | SOCClock | FabricClock | DCEFClock => Some("mhz"),
            PcieSpeed => None,
        }
    }
}
