use amdgpu_sysfs::gpu_handle::{GpuHandle, PowerLevelId, PowerLevels};
use utils::p_level;

mod sysfs;
mod utils;

test_with_handle! {
    "rx6900xt",
    pp_dpm_sclk => {
        GpuHandle::get_core_clock_levels,
        Ok(PowerLevels {
            levels: vec![
                p_level(0, 500),
                p_level(1, 2660)
            ],
            active: Some(PowerLevelId::Index(0))
        })
    },
}
