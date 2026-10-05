//! # gpu-memory
//!
//! Platform-independent reporting of total and available GPU memory, with
//! correct handling of discrete (dedicated VRAM) vs. unified (UMA) GPUs.
//!
//! ```no_run
//! use gpu_memory::list_gpus;
//!
//! for gpu in list_gpus().expect("no GPUs found") {
//!     println!("{gpu:?}");
//! }
//! ```
//!
//! ## Semantics
//!
//! - [`MemoryKind::Discrete`]: `total_bytes` is the dedicated VRAM size and
//!   `available_bytes` is the currently free VRAM.
//! - [`MemoryKind::Unified`]: the GPU shares system RAM (integrated GPUs,
//!   Apple silicon, NVIDIA GB10 "Spark" machines). `total_bytes` is total
//!   system RAM and `available_bytes` is available system RAM.
//!
//! Fields are `Option<u64>` because not every platform can report every
//! value (e.g. Windows DXGI reports total dedicated memory but not free
//! VRAM; free VRAM there comes from `nvidia-smi` when present).

pub mod detect;
pub mod error;
pub mod parse;
pub mod platform;
pub mod types;

pub use error::{Error, Result};
pub use types::{Gpu, GpuVendor, MemoryInfo, MemoryKind};

/// The version of this crate, as injected at build time from the git tag
/// (see `build.rs`). When built outside a git checkout — e.g. from a crates.io
/// tarball — this is the manifest version instead.
pub fn version() -> &'static str {
    env!("GPU_MEMORY_VERSION")
}

/// Lists all GPUs on this system with total and available memory.
///
/// Returns an empty `Vec` if no GPUs are found.
pub fn list_gpus() -> Result<Vec<Gpu>> {
    platform::list_gpus()
}
