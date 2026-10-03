//! Pure parsing/formatting helpers.
//!
//! These are deliberately free of platform I/O so they can be unit-tested on
//! any host (including CI runners that are not the target platform).

use std::collections::HashMap;

/// Parsed `/proc/meminfo` values (bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemInfo {
    /// `MemTotal` in bytes.
    pub total: u64,
    /// `MemAvailable` in bytes.
    pub available: u64,
}

/// Parses `/proc/meminfo` content. Values in the file are in kB.
pub fn parse_meminfo(content: &str) -> Option<MemInfo> {
    let mut total = None;
    let mut available = None;
    for line in content.lines() {
        let mut it = line.split_whitespace();
        let (Some(key), Some(value)) = (it.next(), it.next()) else {
            continue;
        };
        let Ok(value) = value.parse::<u64>() else {
            continue;
        };
        match key {
            "MemTotal:" => total = Some(value * 1024),
            "MemAvailable:" => available = Some(value * 1024),
            _ => {}
        }
    }
    Some(MemInfo {
        total: total?,
        available: available?,
    })
}

/// One GPU as reported by `nvidia-smi`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvidiaSmiGpu {
    pub name: String,
    /// PCI bus ID, e.g. `"00000000:01:00.0"` (absent when nvidia-smi
    /// reports `[N/A]`).
    pub pci_bus_id: Option<String>,
    /// `memory.total` in bytes (nvidia-smi reports MiB).
    pub total_bytes: Option<u64>,
    /// `memory.free` in bytes (nvidia-smi reports MiB).
    pub free_bytes: Option<u64>,
}

/// Parses `nvidia-smi --query-gpu=index,name,pci.bus_id,memory.total,memory.free
/// --format=csv,noheader,nounits` output.
///
/// Memory values are in MiB and converted to bytes.
pub fn parse_nvidia_smi_csv(output: &str) -> Vec<NvidiaSmiGpu> {
    output
        .lines()
        .filter_map(|line| {
            let mut f = line.split(',').map(str::trim);
            let (_index, name, bus_id, total, free) =
                (f.next()?, f.next()?, f.next()?, f.next()?, f.next()?);
            let mib_to_bytes = |mib: &str| mib.parse::<u64>().ok().map(|m| m * 1024 * 1024);
            Some(NvidiaSmiGpu {
                name: name.to_string(),
                pci_bus_id: if bus_id.starts_with('[') {
                    None
                } else {
                    Some(bus_id.to_string())
                },
                total_bytes: mib_to_bytes(total),
                free_bytes: mib_to_bytes(free),
            })
        })
        .collect()
}

/// Normalizes a PCI bus ID to its `bus:slot.func` suffix so that
/// `"00000000:01:00.0"` (nvidia-smi) and `"0000:01:00.0"` (sysfs) compare
/// equal.
pub fn pci_match_key(s: &str) -> String {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        format!("{}:{}", parts[1], parts[2])
    } else {
        s.to_string()
    }
}

/// Parses `lspci -mm` (machine-readable, tab-separated) output into a map of
/// PCI slot (e.g. `"01:00.0"`) to device name.
pub fn parse_lspci_mm(output: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in output.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let (Some(slot), Some(_class), Some(_id), Some(name)) = (
            fields.get(0),
            fields.get(1),
            fields.get(2),
            fields.get(3),
        ) else {
            continue;
        };
        let slot = slot.trim();
        let name = name.trim();
        if !slot.is_empty() && !name.is_empty() {
            map.insert(slot.to_string(), name.to_string());
        }
    }
    map
}

/// Converts a NUL-terminated (or full) UTF-16 buffer to a `String`.
pub fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Formats a byte count for humans: `None` becomes `"unknown"`.
pub fn format_bytes(bytes: Option<u64>) -> String {
    match bytes {
        None => "unknown".to_string(),
        Some(0) => "0 B".to_string(),
        Some(v) => {
            const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
            const MIB: f64 = 1024.0 * 1024.0;
            if v >= 1024 * 1024 * 1024 {
                format!("{:.1} GiB", v as f64 / GIB)
            } else if v >= 1024 * 1024 {
                format!("{:.0} MiB", v as f64 / MIB)
            } else {
                format!("{v} B")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_parses_total_and_available() {
        let content = "\
MemTotal:       32760000 kB
MemFree:         1000000 kB
MemAvailable:   20000000 kB
Buffers:          500000 kB
";
        let m = parse_meminfo(content).unwrap();
        assert_eq!(m.total, 32760000 * 1024);
        assert_eq!(m.available, 20000000 * 1024);
    }

    #[test]
    fn meminfo_requires_both_fields() {
        assert!(parse_meminfo("MemTotal: 100 kB\n").is_none());
        assert!(parse_meminfo("").is_none());
    }

    #[test]
    fn nvidia_smi_csv_parses() {
        let out = "0, NVIDIA GeForce RTX 4090, 00000000:01:00.0, 24564, 24000\n\
                   1, NVIDIA A100 80GB PCIe, 00000000:41:00.0, 81920, 81000\n";
        let gpus = parse_nvidia_smi_csv(out);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4090");
        assert_eq!(gpus[0].pci_bus_id.as_deref(), Some("00000000:01:00.0"));
        assert_eq!(gpus[0].total_bytes, Some(24564 * 1024 * 1024));
        assert_eq!(gpus[0].free_bytes, Some(24000 * 1024 * 1024));
    }

    #[test]
    fn nvidia_smi_csv_handles_na_bus_id() {
        let out = "0, NVIDIA T4, [N/A], 15360, 15000\n";
        let gpus = parse_nvidia_smi_csv(out);
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].pci_bus_id, None);
    }

    #[test]
    fn pci_key_normalizes_domain() {
        assert_eq!(pci_match_key("00000000:01:00.0"), "01:00.0");
        assert_eq!(pci_match_key("0000:01:00.0"), "01:00.0");
        assert_eq!(pci_match_key("weird"), "weird");
    }

    #[test]
    fn lspci_mm_parses() {
        let out = "00:02.0\tVGA compatible controller\t[8086:9a49]\tIntel Corporation Raptor Lake-P [UHD Graphics]\n\
                   01:00.0\t3D controller\t[10de:2684]\tNVIDIA Corporation GA102 [GeForce RTX 3090]\t(rev a1)\n";
        let map = parse_lspci_mm(out);
        assert_eq!(
            map.get("00:02.0").map(String::as_str),
            Some("Intel Corporation Raptor Lake-P [UHD Graphics]")
        );
        assert_eq!(
            map.get("01:00.0").map(String::as_str),
            Some("NVIDIA Corporation GA102 [GeForce RTX 3090]")
        );
    }

    #[test]
    fn wide_string_stops_at_nul() {
        let buf: Vec<u16> = "GPU".encode_utf16().chain(std::iter::once(0)).collect();
        assert_eq!(wide_to_string(&buf), "GPU");
    }

    #[test]
    fn bytes_format() {
        assert_eq!(format_bytes(None), "unknown");
        assert_eq!(format_bytes(Some(0)), "0 B");
        assert_eq!(format_bytes(Some(512)), "512 B");
        assert_eq!(format_bytes(Some(5 * 1024 * 1024)), "5 MiB");
        assert_eq!(format_bytes(Some(24564 * 1024 * 1024)), "24.0 GiB");
    }
}
