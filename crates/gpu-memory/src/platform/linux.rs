//! Linux implementation.
//!
//! Strategy:
//! 1. Enumerate `/sys/class/drm/card*` devices.
//! 2. Classify each by PCI vendor:
//!    - **NVIDIA** (discrete): sysfs exposes no VRAM counters, so query
//!      `nvidia-smi` and join on PCI bus ID.
//!    - **AMD** (discrete): read `device/mem_info_vram_total` /
//!      `mem_info_vram_used`.
//!    - **Intel**: if `device/tile0/lmem_total_bytes` exists it is a discrete
//!      Arc with local memory; otherwise it is an integrated GPU (UMA) and
//!      memory is reported from `/proc/meminfo`.
//! 3. Before all of that, check the GB10 "Spark" detector: on a DGX Spark the
//!   GPU is part of a unified-memory SoC, so report a single unified GPU
//!    backed by system RAM.

use std::fs;
use std::path::{Path, PathBuf};

use crate::detect::{detect_uma_device, MachineSignals};
use crate::parse::{
    parse_lspci_mm, parse_meminfo, parse_nvidia_smi_csv, pci_match_key, MemInfo,
};
use crate::{Error, Gpu, GpuVendor, MemoryInfo, MemoryKind, Result};

/// Reads a file and trims trailing whitespace/NULs. Returns `None` if the
/// file does not exist or cannot be read.
fn read_trim(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().trim_end_matches('\0').trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Reads a sysfs file that contains a single unsigned integer (bytes).
fn read_sysfs_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

fn vendor_from_pci_id(id: &str) -> GpuVendor {
    match id.trim().to_ascii_lowercase().as_str() {
        "0x10de" => GpuVendor::Nvidia,
        "0x1002" => GpuVendor::Amd,
        "0x8086" => GpuVendor::Intel,
        _ => GpuVendor::Unknown,
    }
}

/// A DRM card discovered in sysfs.
struct Card {
    /// e.g. `"card0"`.
    name: String,
    /// PCI vendor ID, e.g. `"0x10de"`.
    vendor_id: String,
    /// PCI bus ID, e.g. `"0000:01:00.0"`.
    pci_bus_id: Option<String>,
    /// Path to the card's sysfs directory.
    path: PathBuf,
}

fn enumerate_cards() -> Result<Vec<Card>> {
    let mut cards = Vec::new();
    let entries = match fs::read_dir("/sys/class/drm") {
        Ok(e) => e,
        Err(e) => return Err(Error::Io(e)),
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(card_name) = file_name.strip_prefix("card") else {
            continue; // skip connectors like card0-DP-1
        };
        if card_name.is_empty() {
            continue;
        }
        let path = entry.path();
        let vendor_id = read_trim(path.join("device/vendor").to_str().unwrap_or(""))
            .unwrap_or_default();
        let pci_bus_id = read_trim(path.join("device").join("uevent").to_str().unwrap_or(""))
            .and_then(|uevent| {
                uevent
                    .lines()
                    .find_map(|l| l.strip_prefix("PCI_SLOT_NAME="))
                    .map(str::to_string)
            });
        cards.push(Card {
            name: file_name,
            vendor_id,
            pci_bus_id,
            path,
        });
    }
    Ok(cards)
}

/// Runs `nvidia-smi` and returns per-GPU data, or `None` if the tool is not
/// available or fails.
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

/// Runs `lspci -mm` and returns a slot -> name map, or an empty map if the
/// tool is not available.
fn lspci_names() -> std::collections::HashMap<String, String> {
    let out = match std::process::Command::new("lspci").arg("-mm").output() {
        Ok(o) if o.status.success() => o,
        _ => return std::collections::HashMap::new(),
    };
    parse_lspci_mm(&String::from_utf8_lossy(&out.stdout))
}

/// Reads `/proc/meminfo` for system RAM (used for UMA reporting).
fn system_memory() -> Result<MemInfo> {
    let content = fs::read_to_string("/proc/meminfo").map_err(Error::Io)?;
    parse_meminfo(&content)
        .ok_or_else(|| Error::Parse {
            source: "/proc/meminfo".to_string(),
            detail: "MemTotal or MemAvailable missing".to_string(),
        })
}

fn gpu_name(card: &Card, lspci: &std::collections::HashMap<String, String>) -> String {
    if let Some(slot) = &card.pci_bus_id {
        if let Some(name) = lspci.get(&pci_match_key(slot)) {
            return name.clone();
        }
    }
    format!("{} GPU (vendor {})", card.name, card.vendor_id)
}

pub fn list_gpus() -> Result<Vec<Gpu>> {
    let cards = enumerate_cards()?;
    let lspci = lspci_names();
    let meminfo = system_memory().ok();

    // --- Special UMA platforms (DGX Spark / GB10) -------------------------
    // Collect GPU names first so the detector can also match on them.
    let gpu_names: Vec<String> = cards
        .iter()
        .map(|c| gpu_name(c, &lspci))
        .collect();
    let signals = MachineSignals {
        system_model: read_trim("/sys/class/dmi/id/product_name")
            .or_else(|| read_trim("/proc/device-tree/model")),
        system_vendor: read_trim("/sys/class/dmi/id/sys_vendor"),
        gpu_names,
    };
    if let Some(profile) = detect_uma_device(&signals) {
        let mem = meminfo.ok_or_else(|| Error::Parse {
            source: "/proc/meminfo".to_string(),
            detail: "needed for unified memory reporting".to_string(),
        })?;
        return Ok(vec![Gpu {
            name: profile.name,
            vendor: profile.vendor,
            memory: MemoryInfo {
                kind: MemoryKind::Unified,
                total_bytes: Some(mem.total),
                available_bytes: Some(mem.available),
            },
            device_id: None,
        }]);
    }

    // --- Regular enumeration ----------------------------------------------
    let nvidia = nvidia_smi();
    let mut gpus = Vec::new();

    for card in &cards {
        let vendor = vendor_from_pci_id(&card.vendor_id);
        let name = gpu_name(card, &lspci);

        match vendor {
            GpuVendor::Nvidia => {
                // Join nvidia-smi data on PCI bus ID; fall back to index
                // order if bus IDs are unavailable.
                let smi = nvidia.as_ref().and_then(|g| {
                    card.pci_bus_id.as_deref().and_then(|slot| {
                        let key = pci_match_key(slot);
                        g.iter().find(|x| {
                            x.pci_bus_id
                                .as_deref()
                                .map(|b| pci_match_key(b) == key)
                                .unwrap_or(false)
                        })
                    })
                });
                let (total, free) = match smi {
                    Some(g) => (g.total_bytes, g.free_bytes),
                    None => (None, None),
                };
                gpus.push(Gpu {
                    name,
                    vendor,
                    memory: MemoryInfo {
                        kind: MemoryKind::Discrete,
                        total_bytes: total,
                        available_bytes: free,
                    },
                    device_id: card.pci_bus_id.clone(),
                });
            }
            GpuVendor::Amd => {
                let total = read_sysfs_u64(card.path.join("device/mem_info_vram_total").as_path());
                let used = read_sysfs_u64(card.path.join("device/mem_info_vram_used").as_path());
                let available = total.zip(used).map(|(t, u)| t.saturating_sub(u));
                gpus.push(Gpu {
                    name,
                    vendor,
                    memory: MemoryInfo {
                        kind: MemoryKind::Discrete,
                        total_bytes: total,
                        available_bytes: available,
                    },
                    device_id: card.pci_bus_id.clone(),
                });
            }
            GpuVendor::Intel => {
                // Discrete Arc exposes local memory under tile0.
                let lmem_total = read_sysfs_u64(
                    card.path.join("device/tile0/lmem_total_bytes").as_path(),
                );
                if let Some(total) = lmem_total {
                    let used = read_sysfs_u64(
                        card.path.join("device/tile0/lmem_used_bytes").as_path(),
                    );
                    let available = used.map(|u| total.saturating_sub(u));
                    gpus.push(Gpu {
                        name,
                        vendor,
                        memory: MemoryInfo {
                            kind: MemoryKind::Discrete,
                            total_bytes: Some(total),
                            available_bytes: available,
                        },
                        device_id: card.pci_bus_id.clone(),
                    });
                } else if let Some(mem) = &meminfo {
                    // Integrated GPU: shares system RAM (UMA).
                    gpus.push(Gpu {
                        name,
                        vendor,
                        memory: MemoryInfo {
                            kind: MemoryKind::Unified,
                            total_bytes: Some(mem.total),
                            available_bytes: Some(mem.available),
                        },
                        device_id: card.pci_bus_id.clone(),
                    });
                } else {
                    gpus.push(Gpu {
                        name,
                        vendor,
                        memory: MemoryInfo {
                            kind: MemoryKind::Unified,
                            total_bytes: None,
                            available_bytes: None,
                        },
                        device_id: card.pci_bus_id.clone(),
                    });
                }
            }
            // Unknown vendor (and any future variants): report the card
            // without memory details.
            _ => {
                gpus.push(Gpu {
                    name,
                    vendor,
                    memory: MemoryInfo {
                        kind: MemoryKind::Discrete,
                        total_bytes: None,
                        available_bytes: None,
                    },
                    device_id: card.pci_bus_id.clone(),
                });
            }
        }
    }

    Ok(gpus)
}
