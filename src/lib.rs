#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

#[cfg(test)]
#[macro_use]
mod tests;
pub mod error;
pub mod gpu_handle;
pub mod hw_mon;
pub mod sysfs;

type Result<T> = std::result::Result<T, error::Error>;
