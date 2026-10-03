//! Detection of special unified-memory (UMA) accelerator platforms.
//!
//! The NVIDIA GB10 "Spark" family (DGX Spark on Linux, RTX Spark on Windows)
//! is a Grace-Blackwell SoC whose GPU shares the system's LPDDR5x memory.
//! Generic enumeration would misreport these as discrete GPUs with a small
//! VRAM carve-out, so we detect them explicitly and report the full system
//! memory as unified.
//!
//! The detection logic is a pure function over [`MachineSignals`] so it can
//! be unit-tested on any host.

use crate::GpuVendor;

/// Raw signals about the current machine, gathered by the platform layer.
#[derive(Debug, Clone, Default)]
pub struct MachineSignals {
    /// System/product model, e.g. `"DGX Spark"` or `"RTX Spark"`.
    pub system_model: Option<String>,
    /// System vendor, e.g. `"NVIDIA"`.
    pub system_vendor: Option<String>,
    /// GPU adapter names as reported by the GPU API (DXGI on Windows,
    /// sysfs/lspci on Linux).
    pub gpu_names: Vec<String>,
}

/// A recognized special platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Human-readable name to use for the GPU entry.
    pub name: String,
    pub vendor: GpuVendor,
    /// Whether the platform uses unified memory (always `true` today).
    pub is_uma: bool,
}

/// Returns a [`DeviceProfile`] if the signals match a known UMA accelerator
/// platform (NVIDIA GB10 "Spark" family).
///
/// Matching is case-insensitive on the substrings `"spark"` and `"gb10"` in
/// the system model, system vendor, or any GPU name. The vendor alone is
/// never sufficient (ordinary NVIDIA machines are not UMA).
pub fn detect_uma_device(signals: &MachineSignals) -> Option<DeviceProfile> {
    let hay = |s: &str| s.to_ascii_lowercase();
    let is_spark = |s: &str| {
        let s = hay(s);
        s.contains("spark") || s.contains("gb10")
    };

    let model_match = signals
        .system_model
        .as_deref()
        .map(is_spark)
        .unwrap_or(false);
    let vendor_match = signals
        .system_vendor
        .as_deref()
        .map(is_spark)
        .unwrap_or(false);
    let gpu_match = signals.gpu_names.iter().any(|n| is_spark(n));

    if model_match || vendor_match || gpu_match {
        // Prefer the GPU name reported by the platform; fall back to the
        // system model; fall back to a generic name.
        let name = signals
            .gpu_names
            .iter()
            .find(|n| is_spark(n))
            .cloned()
            .or_else(|| signals.system_model.clone())
            .unwrap_or_else(|| "NVIDIA GB10 (Spark)".to_string());
        Some(DeviceProfile {
            name,
            vendor: GpuVendor::Nvidia,
            is_uma: true,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(model: Option<&str>, vendor: Option<&str>, gpus: &[&str]) -> MachineSignals {
        MachineSignals {
            system_model: model.map(str::to_string),
            system_vendor: vendor.map(str::to_string),
            gpu_names: gpus.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn detects_dgx_spark_by_model() {
        let s = signals(Some("DGX Spark"), Some("NVIDIA"), &[]);
        let p = detect_uma_device(&s).unwrap();
        assert!(p.is_uma);
        assert_eq!(p.vendor, GpuVendor::Nvidia);
        assert_eq!(p.name, "DGX Spark");
    }

    #[test]
    fn detects_rtx_spark_by_gpu_name() {
        let s = signals(
            Some("Some Laptop"),
            Some("NVIDIA"),
            &["NVIDIA GeForce RTX Spark GB10"],
        );
        let p = detect_uma_device(&s).unwrap();
        assert!(p.is_uma);
        assert_eq!(p.name, "NVIDIA GeForce RTX Spark GB10");
    }

    #[test]
    fn detects_gb10_by_vendor() {
        let s = signals(None, Some("NVIDIA GB10"), &[]);
        assert!(detect_uma_device(&s).is_some());
    }

    #[test]
    fn plain_nvidia_machine_is_not_uma() {
        let s = signals(
            Some("System Product Name"),
            Some("NVIDIA"),
            &["NVIDIA GeForce RTX 4090"],
        );
        assert!(detect_uma_device(&s).is_none());
    }

    #[test]
    fn no_signals_no_match() {
        let s = signals(None, None, &[]);
        assert!(detect_uma_device(&s).is_none());
    }

    #[test]
    fn matching_is_case_insensitive() {
        let s = signals(Some("dgx spark"), None, &[]);
        assert!(detect_uma_device(&s).is_some());
    }
}
