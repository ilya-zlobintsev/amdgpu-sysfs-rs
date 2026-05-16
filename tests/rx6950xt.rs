mod sysfs;
mod utils;

use amdgpu_sysfs::gpu_handle::{GpuHandle, PowerLevels};
use utils::p_level;

test_with_handle! {
    "rx6950xt",
    invalid_dpm_sclk => {
        GpuHandle::get_core_clock_levels,
        Ok(PowerLevels {
            levels: vec![
                p_level(0, 0), p_level(1, 0)
            ],
            active: None,
        })
    },
}
