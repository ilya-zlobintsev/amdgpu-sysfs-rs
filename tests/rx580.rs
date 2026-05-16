mod sysfs;
mod utils;

use amdgpu_sysfs::{
    gpu_handle::{GpuHandle, PerformanceLevel, PowerLevels, PowerLevelsActiveId},
    hw_mon::{HwMon, Temperature},
};
use std::collections::HashMap;
use utils::p_level;

test_with_handle! {
    "rx580",
    pci_ids => {
        GpuHandle::get_pci_id, Some(("1002", "67DF")),
        GpuHandle::get_pci_subsys_id, Some(("1DA2", "E387")),
    },
    driver => {
        GpuHandle::get_driver, "amdgpu"
    },
    busy_percent => {
        GpuHandle::get_busy_percent, Ok(11)
    },
    vram => {
        GpuHandle::get_total_vram, Ok(4096 * 1024 * 1024),
        GpuHandle::get_used_vram, Ok(512 * 1024 * 1024),
    },
    vbios => {
        GpuHandle::get_vbios_version, Ok("113-1E3871U-O4C".to_owned())
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
                p_level(0, 300),
                p_level(1, 600),
                p_level(2, 900),
                p_level(3, 1145),
                p_level(4, 1215),
                p_level(5, 1257),
                p_level(6, 1300),
                p_level(7, 1366)
            ],
            active: Some(PowerLevelsActiveId::Index(2))
        })
    },
    pp_dpm_mclk => {
        GpuHandle::get_memory_clock_levels,
        Ok(PowerLevels {
            levels: vec![
                p_level(0, 300),
                p_level(1, 1000),
                p_level(2, 1750),
            ],
            active: Some(PowerLevelsActiveId::Index(2))
        })
    },
    pp_dpm_pcie => {
        GpuHandle::get_pcie_clock_levels,
        Ok(PowerLevels {
            levels: [
                p_level(0, "2.5GT/s, x8".to_owned()),
                p_level(1, "8.0GT/s, x16".to_owned())
            ].to_vec(),
            active: Some(PowerLevelsActiveId::Index(1))
        })
    }
}

test_with_hw_mon! {
    "rx580",
    fan_info => {
        HwMon::get_fan_pwm, Ok(35),
        HwMon::get_fan_current, Ok(595),
        HwMon::get_fan_target, Ok(595),
        HwMon::get_fan_min, Ok(0),
        HwMon::get_fan_max, Ok(3200),
    },
    temperatures => {
        HwMon::get_temps,
        HashMap::from([(
            "edge".to_owned(),
            Temperature {
                current: Some(44.0),
                crit: Some(94.0),
                crit_hyst: Some(-273.15)
            }
        )])
    },
    gpu_voltage => {
        HwMon::get_gpu_voltage, Ok(975)
    },
}
