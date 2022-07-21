use crate::gpu_handle::{GpuHandle, PerformanceLevel};
use crate::sysfs::SysFS;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::tempdir;

fn create_mock_gpu_handle() -> (GpuHandle, MockSysFS) {
    let mockfs = MockSysFS::init();

    (
        GpuHandle::new_from_path(mockfs.get_path().to_path_buf())
            .expect("Failed to create GPU handle"),
        mockfs,
    )
}

#[test]
fn get_ids() {
    let (gpu_handle, _mockfs) = create_mock_gpu_handle();

    assert_eq!(gpu_handle.get_driver(), "mock");
    assert_eq!(gpu_handle.get_pci_id(), Some(("1002", "67DF")));
    assert_eq!(gpu_handle.get_pci_subsys_id(), Some(("1DA2", "E387")));
}

#[test]
fn get_usage() {
    let (gpu_handle, _mockfs) = create_mock_gpu_handle();

    assert_eq!(gpu_handle.get_busy_percent(), Some(100));
    assert_eq!(gpu_handle.get_total_vram(), Some(512 * 1024 * 1024));
    assert_eq!(gpu_handle.get_used_vram(), Some(256 * 1024 * 1024));
}

#[test]
fn get_bios() {
    let (gpu_handle, _mockfs) = create_mock_gpu_handle();

    assert_eq!(
        gpu_handle.get_vbios_version(),
        Some("MOCKFS-VBIOS".to_string())
    );
}

#[test]
fn get_performance_level() {
    let (gpu_handle, _mockfs) = create_mock_gpu_handle();

    assert_eq!(
        gpu_handle.get_power_force_performance_level(),
        Some(PerformanceLevel::Auto)
    );
}

#[test]
fn get_link() {
    let (gpu_handle, _mockfs) = create_mock_gpu_handle();

    assert_eq!(
        gpu_handle.get_current_link_speed(),
        Some("8.0 GT/s PCIe".to_string())
    );
    assert_eq!(gpu_handle.get_max_link_width(), Some("16".to_string()));
}

#[test]
fn get_fan_info() {
    let (gpu_handle, _mockfs) = create_mock_gpu_handle();
    let hw_mon = gpu_handle.hw_monitors.first().unwrap();

    assert_eq!(hw_mon.get_fan_pwm(), Some(255));

    assert_eq!(hw_mon.get_fan_current(), Some(1600));
    assert_eq!(hw_mon.get_fan_target(), Some(1600));

    assert_eq!(hw_mon.get_fan_max(), Some(3200));
    assert_eq!(hw_mon.get_fan_min(), Some(0));
}

#[test]
fn get_temperatures() {
    let (gpu_handle, _mockfs) = create_mock_gpu_handle();
    let hw_mon = gpu_handle.hw_monitors.first().unwrap();
    let temperatures = hw_mon.get_temps();

    assert_eq!(temperatures["edge"].current, Some(44.0));
    assert_eq!(temperatures["edge"].crit, Some(94.0));
    assert_eq!(temperatures["edge"].crit_hyst, Some(-273.150));
}

#[derive(Debug)]
struct MockSysFS {
    temp_dir: tempfile::TempDir,
}

impl SysFS for MockSysFS {
    fn get_path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

impl MockSysFS {
    pub fn init() -> Self {
        let temp_dir = tempdir().expect("Failed to create temp dir");

        let path = temp_dir.path().to_path_buf();

        std::fs::create_dir_all(&path).expect("Failed to create mock path");

        let mock = Self { temp_dir };

        mock.write_file(
            "uevent",
            "DRIVER=mock\nPCI_ID=1002:67DF\nPCI_SUBSYS_ID=1DA2:E387",
        )
        .unwrap();

        mock.write_file("gpu_busy_percent", "100").unwrap();

        mock.write_file("mem_info_vram_total", (512 * 1024 * 1024).to_string())
            .unwrap();

        mock.write_file("mem_info_vram_used", (256 * 1024 * 1024).to_string())
            .unwrap();

        mock.write_file("vbios_version", "MOCKFS-VBIOS").unwrap();

        mock.write_file("power_dpm_force_performance_level", "auto")
            .unwrap();

        mock.write_file("current_link_speed", "8.0 GT/s PCIe")
            .unwrap();

        mock.write_file("max_link_speed", "8.0 GT/s PCIe").unwrap();

        mock.write_file("current_link_width", "16").unwrap();

        mock.write_file("max_link_width", "16").unwrap();

        let hw_mon_path = path.join("hwmon/hwmon1");

        fs::create_dir_all(hw_mon_path).unwrap();

        mock.write_file("hwmon/hwmon1/name", "mock").unwrap();

        mock.write_file("hwmon/hwmon1/pwm1", "255").unwrap();

        mock.write_file("hwmon/hwmon1/fan1_max", "3200").unwrap();
        mock.write_file("hwmon/hwmon1/fan1_min", "0").unwrap();

        mock.write_file("hwmon/hwmon1/fan1_input", "1600").unwrap();
        mock.write_file("hwmon/hwmon1/fan1_target", "1600").unwrap();

        mock.write_file("hwmon/hwmon1/temp1_label", "edge").unwrap();

        mock.write_file("hwmon/hwmon1/temp1_input", "44000")
            .unwrap();
        mock.write_file("hwmon/hwmon1/temp1_crit", "94000").unwrap();
        mock.write_file("hwmon/hwmon1/temp1_crit_hyst", "-273150")
            .unwrap();

        mock
    }
}
