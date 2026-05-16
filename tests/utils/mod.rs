use amdgpu_sysfs::gpu_handle::{PowerLevel, PowerLevelId};

pub fn p_level<T>(id: u8, value: T) -> PowerLevel<T> {
    PowerLevel {
        id: PowerLevelId::Index(id),
        value,
    }
}
