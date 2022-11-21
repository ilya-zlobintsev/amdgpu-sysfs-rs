# amdgpu-syfs-rs
[![Crates.io](https://img.shields.io/crates/v/amdgpu-sysfs)](https://crates.io/crates/amdgpu-sysfs)
[![Docs.rs](https://docs.rs/amdgpu-sysfs/badge.svg)](https://docs.rs/amdgpu-sysfs/)

This library allows you to interact with the Linux Kernel SysFS interface for GPUs (mainly targeted at the AMDGPU driver). 

Basic usage:

```rust,no_run
use amdgpu_sysfs::gpu_handle::GpuHandle;
# use std::path::PathBuf;

let sysfs_path = PathBuf::from("/sys/class/drm/card0/device");

let gpu_handle = GpuHandle::new_from_path(sysfs_path).unwrap();
    
let gpu_usage = gpu_handle.get_busy_percent().unwrap();
    
let total_vram = gpu_handle.get_total_vram().unwrap(); 
```
