use amdgpu_sysfs::gpu_handle::{GpuHandle, PowerLevels};

mod sysfs;

test_with_handle! {
    "rx6900xt",
    pp_dpm_sclk => {
        GpuHandle::get_core_clock_levels,
        Ok(PowerLevels {
            levels: vec![
                500,
                2660
            ],
            active: Some(0)
        })
    },
}
