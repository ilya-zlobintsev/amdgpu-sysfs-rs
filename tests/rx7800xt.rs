mod sysfs;

use amdgpu_sysfs::gpu_handle::{fan_control::AcousticInfo, GpuHandle};

test_with_handle! {
    "rx7800xt",
    get_fan_acoustic_limit => {
        GpuHandle::get_fan_acoustic_limit,
        Ok(AcousticInfo { current: 2450, min: 500, max: 3100 })
    },
    get_fan_acoustic_target => {
        GpuHandle::get_fan_acoustic_target,
        Ok(AcousticInfo { current: 2200, min: 500, max: 3100 })
    },
}
