mod sysfs;

use amdgpu_sysfs::{
    gpu_handle::{GpuHandle, PerformanceLevel, PowerLevels},
    hw_mon::{HwMon, Temperature},
};
use std::collections::HashMap;

test_with_handle! {
    "vega56",
    pci_ids => {
        GpuHandle::get_pci_id, Some(("1002", "687F")),
        GpuHandle::get_pci_subsys_id, Some(("1043", "0555")),
    },
    driver => {
        GpuHandle::get_driver, "amdgpu"
    },
    busy_percent => {
        GpuHandle::get_busy_percent, Ok(0)
    },
    vram => {
        GpuHandle::get_total_vram, Ok(8176 * 1024 * 1024),
        GpuHandle::get_used_vram, Ok(16224 * 1024),
    },
    vbios => {
        GpuHandle::get_vbios_version, Ok("115-D050PIL-100".to_owned())
    },
    performance_level => {
        GpuHandle::get_power_force_performance_level, Ok(PerformanceLevel::Auto),
    },
    link => {
        GpuHandle::get_current_link_speed, Ok("8.0 GT/s PCIe".to_owned()),
        GpuHandle::get_current_link_width, Ok("16".to_owned()),
        GpuHandle::get_max_link_speed, Ok("8.0 GT/s PCIe".to_owned()),
        GpuHandle::get_max_link_width, Ok("16".to_owned()),
    },
    pp_dpm_sclk => {
        GpuHandle::get_core_clock_levels,
        Ok(PowerLevels {
            levels: vec![
                852,
                991,
                1138,
                1269,
                1312,
                1474,
                1538,
                1590
            ],
            active: Some(0)
        })
    },
    pp_dpm_mclk => {
        GpuHandle::get_memory_clock_levels,
        Ok(PowerLevels {
            levels: vec![
                167,
                500,
                700,
                920,
            ],
            active: Some(0)
        })
    },
    pp_dpm_pcie => {
        GpuHandle::get_pcie_clock_levels,
        Ok(PowerLevels {
            levels: [
                "8.0GT/s, x16",
                "8.0GT/s, x16"
            ].map(str::to_owned).to_vec(),
            active: Some(1)
        })
    }
}

test_with_hw_mon! {
    "vega56",
    fan_info => {
        HwMon::get_fan_pwm, Ok(0),
        HwMon::get_fan_current, Ok(5),
        HwMon::get_fan_target, Ok(5),
        HwMon::get_fan_min, Ok(0),
        HwMon::get_fan_max, Ok(3500),
    },
    temperatures => {
        HwMon::get_temps,
        HashMap::from([
        (
            "edge".to_owned(),
            Temperature {
                current: Some(38.0),
                crit: Some(85.0),
                crit_hyst: Some(-273.15)
            }
        ),
        (
            "junction".to_owned(),
            Temperature {
                current: Some(38.0),
                crit: Some(105.0),
                crit_hyst: Some(-273.15)
            }
        ),
        (
            "mem".to_owned(),
            Temperature {
                current: Some(39.0),
                crit: Some(95.0),
                crit_hyst: Some(-273.15)
            }
        )
        ])
    },
    gpu_voltage => {
        HwMon::get_gpu_voltage, Ok(762)
    },
}
