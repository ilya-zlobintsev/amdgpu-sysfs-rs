mod sysfs;

use amdgpu_sysfs::gpu_handle::{
    fan_control::{FanCurve, FanCurveRanges, FanInfo},
    GpuHandle,
};

test_with_handle! {
    "rx7900xt",
    get_fan_acoustic_limit => {
        GpuHandle::get_fan_acoustic_limit,
        Ok(FanInfo { current: 3200, allowed_range: Some((500,  3200)) })
    },
    get_fan_acoustic_target => {
        GpuHandle::get_fan_acoustic_target,
        Ok(FanInfo { current: 1450, allowed_range: Some((500,  3200)) })
    },
    get_fan_target_temperature => {
        GpuHandle::get_fan_target_temperature,
        Ok(FanInfo { current: 83, allowed_range: Some((25,  105)) })
    },
    get_fan_minimum_pwm => {
        GpuHandle::get_fan_minimum_pwm,
        Ok(FanInfo { current: 15, allowed_range: Some((15,  100)) })
    },
    get_fan_curve => {
        GpuHandle::get_fan_curve,
        Ok(FanCurve { points: vec![(0, 0); 5], allowed_ranges: Some(FanCurveRanges {temperature_range: (25, 100), speed_range: (15, 100) })})
    }
}
