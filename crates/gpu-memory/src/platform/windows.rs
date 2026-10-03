//! Windows implementation.
//!
//! Strategy:
//! 1. Enumerate DXGI adapters for names and dedicated memory.
//! 2. Check the GB10 "Spark" detector (RTX Spark on Windows): if it fires,
//!    report a single unified GPU backed by system RAM.
//! 3. Otherwise:
//!    - NVIDIA GPUs come from `nvidia-smi` when available (it reports both
//!      total and free VRAM); DXGI is the fallback (total only).
//!    - Other discrete GPUs (e.g. AMD) use DXGI `DedicatedVideoMemory` for
//!      the total; free VRAM is not available without vendor APIs.
//!    - Adapters with no dedicated memory are integrated (UMA): report
//!      system RAM via `GlobalMemoryStatusEx`.

use windows::core::HSTRING;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory, IDXGIFactory};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    REG_VALUE_TYPE,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use crate::detect::{detect_uma_device, MachineSignals};
use crate::parse::{parse_nvidia_smi_csv, wide_to_string};
use crate::{Error, Gpu, GpuVendor, MemoryInfo, MemoryKind, Result};

/// A DXGI adapter.
struct Adapter {
    name: String,
    /// Dedicated VRAM in bytes (0 for integrated GPUs).
    dedicated: u64,
}

/// Total/available system RAM in bytes.
fn system_memory() -> Result<(u64, u64)> {
    unsafe {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        GlobalMemoryStatusEx(&mut status).map_err(|e| Error::Platform {
            detail: format!("GlobalMemoryStatusEx failed: {e}"),
        })?;
        Ok((status.ullTotalPhys, status.ullAvailPhys))
    }
}

/// Reads a REG_SZ value from `HKLM\<subkey>`.
fn registry_sz(subkey: &str, value_name: &str) -> Option<String> {
    let sub = HSTRING::from(subkey);
    let val = HSTRING::from(value_name);
    let mut hkey = HKEY::default();
    if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, &sub, None, KEY_READ, &mut hkey) }
        != ERROR_SUCCESS
    {
        return None;
    }
    let result = unsafe {
        let mut data_type = REG_VALUE_TYPE(0);
        let mut size = 0u32;
        if RegQueryValueExW(hkey, &val, None, Some(&mut data_type), None, Some(&mut size))
            != ERROR_SUCCESS
        {
            None
        } else if data_type.0 != 1 {
            None // REG_SZ only
        } else {
            let mut buf = vec![0u8; size as usize];
            if RegQueryValueExW(
                hkey,
                &val,
                None,
                Some(&mut data_type),
                Some(buf.as_mut_ptr()),
                Some(&mut size),
            ) != ERROR_SUCCESS
            {
                None
            } else {
                let wide =
                    std::slice::from_raw_parts(buf.as_ptr() as *const u16, (size / 2) as usize);
                Some(wide_to_string(wide))
            }
        }
    };
    let _ = unsafe { RegCloseKey(hkey) };
    result.filter(|s| !s.is_empty())
}

/// Best-effort system model from the BIOS registry block.
fn system_model() -> Option<String> {
    registry_sz(r"HARDWARE\DESCRIPTION\System\BIOS", "SystemFamily")
        .or_else(|| registry_sz(r"HARDWARE\DESCRIPTION\System\BIOS", "BaseBoardProduct"))
        .or_else(|| registry_sz(r"HARDWARE\DESCRIPTION\System\BIOS", "SystemSKU"))
}

/// Enumerates DXGI adapters, skipping software renderers.
fn enumerate_adapters() -> Vec<Adapter> {
    unsafe {
        let factory: IDXGIFactory = match CreateDXGIFactory() {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let mut adapters = Vec::new();
        let mut index = 0u32;
        loop {
            let adapter = match factory.EnumAdapters(index) {
                Ok(a) => a,
                Err(_) => break, // DXGI_ERROR_NOT_FOUND: end of list
            };
            let desc = match adapter.GetDesc() {
                Ok(d) => d,
                Err(_) => {
                    index += 1;
                    continue;
                }
            };
            let name = wide_to_string(&desc.Description);
            if !name.contains("Microsoft Basic Render") && !name.contains("Hyper-V") {
                adapters.push(Adapter {
                    name,
                    dedicated: desc.DedicatedVideoMemory as u64,
                });
            }
            index += 1;
        }
        adapters
    }
}

fn vendor_from_name(name: &str) -> GpuVendor {
    let n = name.to_ascii_lowercase();
    if n.contains("nvidia") || n.contains("geforce") || n.contains("quadro") {
        GpuVendor::Nvidia
    } else if n.contains("amd") || n.contains("radeon") {
        GpuVendor::Amd
    } else if n.contains("intel") || n.contains("iris") {
        GpuVendor::Intel
    } else {
        GpuVendor::Unknown
    }
}

/// Runs `nvidia-smi` and returns per-GPU data, or `None` if unavailable.
fn nvidia_smi() -> Option<Vec<crate::parse::NvidiaSmiGpu>> {
    let out = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,name,pci.bus_id,memory.total,memory.free",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(parse_nvidia_smi_csv(&String::from_utf8_lossy(&out.stdout)))
}

pub fn list_gpus() -> Result<Vec<Gpu>> {
    let adapters = enumerate_adapters();
    let (total_ram, avail_ram) = system_memory()?;

    // --- Special UMA platforms (RTX Spark / GB10) -------------------------
    let signals = MachineSignals {
        system_model: system_model(),
        system_vendor: None,
        gpu_names: adapters.iter().map(|a| a.name.clone()).collect(),
    };
    if let Some(profile) = detect_uma_device(&signals) {
        return Ok(vec![Gpu {
            name: profile.name,
            vendor: profile.vendor,
            memory: MemoryInfo {
                kind: MemoryKind::Unified,
                total_bytes: Some(total_ram),
                available_bytes: Some(avail_ram),
            },
            device_id: None,
        }]);
    }

    let nvidia = nvidia_smi();
    let mut gpus = Vec::new();

    if let Some(list) = &nvidia {
        // nvidia-smi is authoritative for NVIDIA GPUs (total + free VRAM).
        for g in list {
            gpus.push(Gpu {
                name: g.name.clone(),
                vendor: GpuVendor::Nvidia,
                memory: MemoryInfo {
                    kind: MemoryKind::Discrete,
                    total_bytes: g.total_bytes,
                    available_bytes: g.free_bytes,
                },
                device_id: g.pci_bus_id.clone(),
            });
        }
        // Add non-NVIDIA adapters from DXGI.
        for adapter in &adapters {
            if vendor_from_name(&adapter.name) == GpuVendor::Nvidia {
                continue;
            }
            push_adapter(&mut gpus, adapter, total_ram, avail_ram);
        }
    } else {
        // No nvidia-smi: use DXGI for everything (NVIDIA gets total only).
        for adapter in &adapters {
            push_adapter(&mut gpus, adapter, total_ram, avail_ram);
        }
    }

    Ok(gpus)
}

fn push_adapter(gpus: &mut Vec<Gpu>, adapter: &Adapter, total_ram: u64, avail_ram: u64) {
    let vendor = vendor_from_name(&adapter.name);
    if adapter.dedicated == 0 {
        // Integrated GPU: shares system RAM (UMA).
        gpus.push(Gpu {
            name: adapter.name.clone(),
            vendor,
            memory: MemoryInfo {
                kind: MemoryKind::Unified,
                total_bytes: Some(total_ram),
                available_bytes: Some(avail_ram),
            },
            device_id: None,
        });
    } else {
        gpus.push(Gpu {
            name: adapter.name.clone(),
            vendor,
            memory: MemoryInfo {
                kind: MemoryKind::Discrete,
                total_bytes: Some(adapter.dedicated),
                available_bytes: None,
            },
            device_id: None,
        });
    }
}
