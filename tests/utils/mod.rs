use amdgpu_sysfs::gpu_handle::{PowerLevel, PowerLevelsActiveId};

pub fn p_level<T>(id: u8, value: T) -> PowerLevel<T> {
    PowerLevel {
        id: PowerLevelsActiveId::Index(id),
        value,
    }
}
