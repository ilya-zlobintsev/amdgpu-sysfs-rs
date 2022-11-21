#![doc = include_str!("../README.md")]
pub mod error;
pub mod gpu_handle;
pub mod hw_mon;
pub mod sysfs;

type Result<T> = std::result::Result<T, error::Error>;
