pub mod gpu_controller;
pub mod hw_mon;
pub mod sysfs;

#[cfg(all(test, feature = "mock"))]
mod tests {
    use crate::gpu_controller::{GpuController, PowerLevel};
    use crate::sysfs::SysFS;
    use tempfile::tempdir;

    #[test]
    fn mock() {
        let mockfs = MockSysFS::new();

        let gpu_controller = GpuController::new_from_path(mockfs.get_path().to_path_buf())
            .expect("Failed to create GPU controller");

        assert_eq!(gpu_controller.get_driver(), "mock");

        assert_eq!(gpu_controller.get_busy_percent(), Some(100));

        assert_eq!(gpu_controller.get_total_vram(), Some(512 * 1024 * 1024));

        assert_eq!(gpu_controller.get_used_vram(), Some(256 * 1024 * 1024));

        assert_eq!(
            gpu_controller.get_vbios_version(),
            Some("MOCKFS-VBIOS".to_string())
        );

        assert_eq!(gpu_controller.get_power_level(), Some(PowerLevel::Auto));

        let hw_mon = gpu_controller.hw_monitors.first().unwrap();

        assert_eq!(hw_mon.get_fan_pwm(), Some(255));
        
        assert_eq!(hw_mon.get_fan_current(), Some(1600));
        assert_eq!(hw_mon.get_fan_target(), Some(1600));

        assert_eq!(hw_mon.get_fan_max(), Some(3200));
        assert_eq!(hw_mon.get_fan_min(), Some(0));
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
        pub fn new() -> Self {
            let temp_dir = tempdir().expect("Failed to create temp dir");

            let path = temp_dir.path().to_path_buf();

            std::fs::create_dir_all(&path).expect("Failed to create mock path");

            let mock = Self { temp_dir };

            mock.write_file("uevent", "DRIVER=mock\nPCI_ID=1002:67DF")
                .unwrap();

            mock.write_file("gpu_busy_percent", "100").unwrap();

            mock.write_file("mem_info_vram_total", (512 * 1024 * 1024).to_string())
                .unwrap();

            mock.write_file("mem_info_vram_used", (256 * 1024 * 1024).to_string())
                .unwrap();

            mock.write_file("vbios_version", "MOCKFS-VBIOS").unwrap();

            mock.write_file("power_dpm_force_performance_level", "auto")
                .unwrap();

            let hw_mon_path = path.join("hwmon/hwmon1");

            std::fs::create_dir_all(hw_mon_path).unwrap();

            mock.write_file("hwmon/hwmon1/name", "mock").unwrap();

            mock.write_file("hwmon/hwmon1/pwm1", "255").unwrap();

            mock.write_file("hwmon/hwmon1/fan1_max", "3200").unwrap();
            mock.write_file("hwmon/hwmon1/fan1_min", "0").unwrap();

            mock.write_file("hwmon/hwmon1/fan1_input", "1600").unwrap();
            mock.write_file("hwmon/hwmon1/fan1_target", "1600").unwrap();

            mock
        }
    }
}
