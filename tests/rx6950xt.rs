mod sysfs;

use amdgpu_sysfs::gpu_handle::{GpuHandle, PowerLevels};

test_with_handle! {
    "rx6950xt",
    invalid_dpm_sclk => {
        GpuHandle::get_core_clock_levels,
        Ok(PowerLevels {
            levels: vec![
                0, 0
            ],
            active: None,
        })
    },
}
