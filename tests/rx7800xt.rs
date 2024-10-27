mod sysfs;

use amdgpu_sysfs::gpu_handle::{
    fan_control::{FanCurve, FanCurveRanges, FanInfo},
    GpuHandle,
};

test_with_handle! {
    "rx7800xt",
    get_fan_acoustic_limit => {
        GpuHandle::get_fan_acoustic_limit,
        Ok(FanInfo { current: 2450, allowed_range: Some((500,  3100)) })
    },
    get_fan_acoustic_target => {
        GpuHandle::get_fan_acoustic_target,
        Ok(FanInfo { current: 2200, allowed_range: Some((500,  3100)) })
    },
    get_fan_target_temperature => {
        GpuHandle::get_fan_target_temperature,
        Ok(FanInfo { current: 95, allowed_range: Some((25,  110)) })
    },
    get_fan_minimum_pwm => {
        GpuHandle::get_fan_minimum_pwm,
        Ok(FanInfo { current: 97, allowed_range: Some((20,  100)) })
    },
    get_fan_curve => {
        GpuHandle::get_fan_curve,
        Ok(FanCurve { points: vec![(0, 0); 5].into_boxed_slice(), allowed_ranges: Some(FanCurveRanges {temperature_range: 25..=100, speed_range: 20..=100 })})
    },
    get_fan_zero_rpm => {
        GpuHandle::get_fan_zero_rpm,
        Ok(false),
    }
}
