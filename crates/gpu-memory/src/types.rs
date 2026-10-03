//! Core types shared by all platforms.

use serde::{Deserialize, Serialize};

/// Vendor of a GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Unknown,
}

impl GpuVendor {
    /// Stable string form (used in JSON output and fallback names).
    pub fn as_str(self) -> &'static str {
        match self {
            GpuVendor::Nvidia => "nvidia",
            GpuVendor::Amd => "amd",
            GpuVendor::Intel => "intel",
            GpuVendor::Apple => "apple",
            GpuVendor::Unknown => "unknown",
        }
    }
}

/// How a GPU's memory is organized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MemoryKind {
    /// Discrete GPU with dedicated VRAM.
    ///
    /// `total_bytes` is the size of the VRAM and `available_bytes` is the
    /// currently free VRAM.
    Discrete,
    /// Unified Memory Architecture (UMA): the GPU shares the system's main
    /// memory (integrated GPUs, Apple silicon, NVIDIA GB10 "Spark" machines).
    ///
    /// `total_bytes` is the total system RAM (or the platform-reported GPU
    /// share) and `available_bytes` is the currently available system RAM.
    /// Callers must not treat this as dedicated VRAM.
    Unified,
}

impl MemoryKind {
    /// Stable string form (used in JSON output).
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryKind::Discrete => "discrete",
            MemoryKind::Unified => "unified",
        }
    }
}

/// Memory information for a GPU.
///
/// Both byte fields are [`Option`] because not every platform can report
/// every value (for example, Windows DXGI reports total dedicated memory but
/// not free VRAM; free VRAM there comes from `nvidia-smi` when present).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub kind: MemoryKind,
    /// Total memory usable by the GPU, in bytes.
    ///
    /// - [`MemoryKind::Discrete`]: size of the dedicated VRAM.
    /// - [`MemoryKind::Unified`]: total system RAM (or the platform-reported
    ///   GPU share).
    pub total_bytes: Option<u64>,
    /// Memory currently available to the GPU, in bytes.
    ///
    /// - [`MemoryKind::Discrete`]: free VRAM.
    /// - [`MemoryKind::Unified`]: available system RAM.
    pub available_bytes: Option<u64>,
}

/// A single GPU.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gpu {
    /// Human-readable name, e.g. `"NVIDIA GeForce RTX 4090"`.
    pub name: String,
    pub vendor: GpuVendor,
    pub memory: MemoryInfo,
    /// Platform-specific identifier (PCI bus ID on Linux, `None` elsewhere
    /// for now).
    pub device_id: Option<String>,
}
