use std::io::Write;
use std::{fs::File, path::PathBuf};

mod gpu_controller;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{gpu_controller::GpuController, *};
    use tempfile::tempdir;

    #[test]
    fn mock() {
        let dir = tempdir().expect("Failed to create temp dir");

        let path = dir.path().to_path_buf();

        create_sysfs_mock(path.as_path());

        let gpu_controller =
            GpuController::new_from_path(path).expect("Failed to create GPU controller");

        assert_eq!(gpu_controller.get_driver(), "mock");

        assert_eq!(gpu_controller.get_busy_percent(), Some(100));

        assert_eq!(gpu_controller.get_total_vram(), Some(512 * 1024 * 1024));

        assert_eq!(gpu_controller.get_used_vram(), Some(256 * 1024 * 1024));

        assert_eq!(
            gpu_controller.get_vbios_version(),
            Some("MOCKFS-VBIOS".to_string())
        );

        dir.close().expect("Failed to close temp dir");
    }

    fn create_sysfs_mock(path: &Path) {
        let path = &path;

        std::fs::create_dir_all(&path).expect("Failed to create mock path");

        let mut uevent = File::create(path.join("uevent")).unwrap();

        writeln!(uevent, "DRIVER=mock").unwrap();
        writeln!(uevent, "PCI_ID=1002:67DF").unwrap();

        let mut gpu_busy_percent = File::create(path.join("gpu_busy_percent")).unwrap();

        writeln!(gpu_busy_percent, "100").unwrap();

        let mut mem_info_vram_total = File::create(path.join("mem_info_vram_total")).unwrap();

        writeln!(mem_info_vram_total, "{}", 512 * 1024 * 1024).unwrap();

        let mut mem_info_vram_used = File::create(path.join("mem_info_vram_used")).unwrap();

        writeln!(mem_info_vram_used, "{}", 256 * 1024 * 1024).unwrap();

        let mut vbios_version = File::create(path.join("vbios_version")).unwrap();

        writeln!(vbios_version, "MOCKFS-VBIOS").unwrap();
    }
}
