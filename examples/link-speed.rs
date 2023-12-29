use amdgpu_sysfs::gpu_handle::GpuHandle;
use std::path::PathBuf;

fn main() {
    let sysfs_path = PathBuf::from("/sys/class/drm/card0/device");
    let gpu_handle = GpuHandle::new_from_path(sysfs_path).unwrap();

    println!("Cur: {}x{}",
        gpu_handle.get_current_link_speed().unwrap(),
        gpu_handle.get_current_link_width().unwrap(),
    );
    println!("Max: {}x{}",
        gpu_handle.get_max_link_speed().unwrap(),
        gpu_handle.get_max_link_width().unwrap(),
    );
}
