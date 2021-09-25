pub mod gpu_controller;
pub mod hw_mon;
pub mod sysfs;

#[cfg(all(test, feature = "mock"))]
mod tests {
    use crate::gpu_controller::{GpuController, PerformanceLevel};
    use crate::sysfs::SysFS;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn mock() {
        let mockfs = MockSysFS::init().await;

        let gpu_controller = GpuController::new_from_path(mockfs.get_path().to_path_buf())
            .await
            .expect("Failed to create GPU controller");

        assert_eq!(gpu_controller.get_driver().await, "mock");

        assert_eq!(gpu_controller.get_busy_percent().await, Some(100));

        assert_eq!(
            gpu_controller.get_total_vram().await,
            Some(512 * 1024 * 1024)
        );

        assert_eq!(
            gpu_controller.get_used_vram().await,
            Some(256 * 1024 * 1024)
        );

        assert_eq!(
            gpu_controller.get_vbios_version().await,
            Some("MOCKFS-VBIOS".to_string())
        );

        assert_eq!(
            gpu_controller.get_power_force_performance_level().await,
            Some(PerformanceLevel::Auto)
        );

        let hw_mon = gpu_controller.hw_monitors.first().unwrap();

        assert_eq!(hw_mon.get_fan_pwm().await, Some(255));

        assert_eq!(hw_mon.get_fan_current().await, Some(1600));
        assert_eq!(hw_mon.get_fan_target().await, Some(1600));

        assert_eq!(hw_mon.get_fan_max().await, Some(3200));
        assert_eq!(hw_mon.get_fan_min().await, Some(0));

        let temperatures = hw_mon.get_temps().await;

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
        pub async fn init() -> Self {
            let temp_dir = tempdir().expect("Failed to create temp dir");

            let path = temp_dir.path().to_path_buf();

            std::fs::create_dir_all(&path).expect("Failed to create mock path");

            let mock = Self { temp_dir };

            mock.write_file("uevent", "DRIVER=mock\nPCI_ID=1002:67DF")
                .await
                .unwrap();

            mock.write_file("gpu_busy_percent", "100").await.unwrap();

            mock.write_file("mem_info_vram_total", (512 * 1024 * 1024).to_string())
                .await
                .unwrap();

            mock.write_file("mem_info_vram_used", (256 * 1024 * 1024).to_string())
                .await
                .unwrap();

            mock.write_file("vbios_version", "MOCKFS-VBIOS")
                .await
                .unwrap();

            mock.write_file("power_dpm_force_performance_level", "auto")
                .await
                .unwrap();

            let hw_mon_path = path.join("hwmon/hwmon1");

            fs::create_dir_all(hw_mon_path).await.unwrap();

            mock.write_file("hwmon/hwmon1/name", "mock").await.unwrap();

            mock.write_file("hwmon/hwmon1/pwm1", "255").await.unwrap();

            mock.write_file("hwmon/hwmon1/fan1_max", "3200")
                .await
                .unwrap();
            mock.write_file("hwmon/hwmon1/fan1_min", "0").await.unwrap();

            mock.write_file("hwmon/hwmon1/fan1_input", "1600")
                .await
                .unwrap();
            mock.write_file("hwmon/hwmon1/fan1_target", "1600")
                .await
                .unwrap();

            mock.write_file("hwmon/hwmon1/temp1_label", "edge")
                .await
                .unwrap();

            mock.write_file("hwmon/hwmon1/temp1_input", "44000")
                .await
                .unwrap();
            mock.write_file("hwmon/hwmon1/temp1_crit", "94000")
                .await
                .unwrap();
            mock.write_file("hwmon/hwmon1/temp1_crit_hyst", "-273150")
                .await
                .unwrap();

            mock
        }
    }
}
