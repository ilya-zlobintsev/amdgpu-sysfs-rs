mod sysfs;

use amdgpu_sysfs::gpu_handle::{fan_control::FanInfo, GpuHandle};

test_with_handle! {
    "rx7800xt",
    get_fan_acoustic_limit => {
        GpuHandle::get_fan_acoustic_limit,
        Ok(FanInfo { current: 2450, min: 500, max: 3100 })
    },
    get_fan_acoustic_target => {
        GpuHandle::get_fan_acoustic_target,
        Ok(FanInfo { current: 2200, min: 500, max: 3100 })
    },
    get_fan_target_temperature => {
        GpuHandle::get_fan_target_temperature,
        Ok(FanInfo { current: 95, min: 25, max: 110 })
    },
    get_fan_minimum_pwm => {
        GpuHandle::get_fan_minimum_pwm,
        Ok(FanInfo { current: 20, min: 20, max: 100 })
    },
}
