use amdgpu_sysfs::{gpu_handle::GpuHandle, sysfs::SysFS};
use rust_embed::RustEmbed;
use std::fs;
use tempfile::{tempdir, TempDir};

#[derive(RustEmbed)]
#[folder = "tests/data/"]
struct Asset;

pub struct MockSysFs {
    temp_dir: TempDir,
}

impl MockSysFs {
    pub fn new(name: &str) -> Self {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let path = temp_dir.path();

        let path_prefix = format!("{name}/");
        for file_name in Asset::iter() {
            if let Some(stripped_name) = file_name.strip_prefix(&path_prefix) {
                let target_path = path.join(stripped_name);

                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent).expect("Could not create parent dir in temp dir");
                }

                let contents = Asset::get(&file_name)
                    .expect("Could not read file from embedded fs")
                    .data;
                fs::write(target_path, contents).expect("Could not write contents to temp dir");
            }
        }

        Self { temp_dir }
    }
}

impl SysFS for MockSysFs {
    fn get_path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

pub fn create_mock_gpu_handle(name: &str) -> (GpuHandle, MockSysFs) {
    let mockfs = MockSysFs::new(name);

    (
        GpuHandle::new_from_path(mockfs.get_path().to_path_buf())
            .expect("Failed to create GPU handle"),
        mockfs,
    )
}

#[macro_export]
macro_rules! test_with_handle {
    ($sysfs_name:expr, $($test_name:ident => {$($code:expr, $expected:expr),* $(,)?}),* $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                let (handle, _mockfs) = crate::sysfs::create_mock_gpu_handle($sysfs_name);
                $(
                    let value = $code(&handle);
                    pretty_assertions::assert_eq!(value, $expected);
                )*
            }
        )*
    };
}

#[macro_export]
macro_rules! test_with_hw_mon {
    ($sysfs_name:expr, $($test_name:ident => {$($code:expr, $expected:expr),* $(,)?}),* $(,)?) => {
        $(
            #[test]
            fn $test_name() {
                let (handle, _mockfs) = crate::sysfs::create_mock_gpu_handle($sysfs_name);
                let hw_mon = handle.hw_monitors.first().expect("Handle has no hw monitor");
                $(
                    let value = $code(&hw_mon);
                    pretty_assertions::assert_eq!(value, $expected);
                )*
            }
        )*
    };
}
