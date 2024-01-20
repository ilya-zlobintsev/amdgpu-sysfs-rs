#![allow(clippy::redundant_closure_call)]
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
        Ok(FanCurve { points: vec![(0, 0); 5].into_boxed_slice(), allowed_ranges: Some(FanCurveRanges {temperature_range: 25..=100, speed_range: 15..=100 })})
    },
    set_invalid_fan_curve => {
        |gpu_handle: &GpuHandle| {
            let mut curve = gpu_handle.get_fan_curve().unwrap();
            curve.points[0].0 = 5;
            curve.points[0].1 = 0;
            gpu_handle.set_fan_curve(&curve).unwrap_err().to_string()
        },
        "not allowed: Temperature value 5 is outside of the allowed range 25..=100",
    },

    set_valid_fan_curve => {
        |gpu_handle: &GpuHandle| {
            let mut curve = gpu_handle.get_fan_curve().unwrap();
            curve.points[0] = (25, 15);
            curve.points[1] = (40, 30);
            curve.points[2] = (60, 65);
            curve.points[3] = (70, 80);
            curve.points[4] = (85, 100);
            gpu_handle.set_fan_curve(&curve)
        },
        Ok(())
    }
}
