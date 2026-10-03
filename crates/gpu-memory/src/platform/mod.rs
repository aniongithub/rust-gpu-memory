//! Platform dispatch. Each OS has its own implementation; the public API is
//! the same everywhere.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::list_gpus;
#[cfg(target_os = "macos")]
pub use macos::list_gpus;
#[cfg(target_os = "windows")]
pub use windows::list_gpus;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn list_gpus() -> crate::Result<Vec<crate::Gpu>> {
    Err(crate::Error::UnsupportedPlatform)
}
