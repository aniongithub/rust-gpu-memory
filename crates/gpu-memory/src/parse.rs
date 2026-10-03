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

/// Normalizes a PCI bus ID to a canonical `domain:bus:slot.func` key so that
/// the same device reported by different tools compares equal:
/// `"00000000:01:00.0"` (nvidia-smi), `"0000:01:00.0"` (sysfs) and
/// `"01:00.0"` (lspci, which omits the domain) all map to `"0:01:00.0"`.
///
/// The domain is preserved (hex-normalized) so that devices in different PCI
/// domains at the same bus/slot/function remain distinct.
pub fn pci_match_key(s: &str) -> String {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        // `domain:bus:slot.func` — canonicalize the domain to a bare hex
        // value so `00000000` and `0000` compare equal.
        3 => match u32::from_str_radix(parts[0], 16) {
            Ok(domain) => format!("{:x}:{}:{}", domain, parts[1], parts[2]),
            Err(_) => s.to_string(),
        },
        // `bus:slot.func` (lspci) — no domain, so assume domain 0.
        2 => format!("0:{}:{}", parts[0], parts[1]),
        _ => s.to_string(),
    }
}

/// Parses `lspci -mm` (machine-readable) output into a map of PCI slot
/// (e.g. `"01:00.0"`) to device name.
///
/// `lspci -mm` is whitespace-separated with double-quoted fields, e.g.
/// `00:02.0 "VGA compatible controller" "Intel Corporation" "UHD Graphics 770" -r04`.
/// The slot is the first token and the device name is the last quoted field
/// on the line.
pub fn parse_lspci_mm(output: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (slot, rest) = match line.split_once(char::is_whitespace) {
            Some((slot, rest)) => (slot, rest),
            None => continue,
        };
        if slot.is_empty() {
            continue;
        }
        let quoted = quoted_fields(rest);
        let Some(name) = quoted.last() else {
            continue;
        };
        if !name.is_empty() {
            map.insert(slot.to_string(), name.clone());
        }
    }
    map
}

/// Extracts the contents of double-quoted fields from a string, in order.
fn quoted_fields(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if let Some(rel) = s[i + 1..].find('"') {
                let end = i + 1 + rel;
                out.push(s[i + 1..end].to_string());
                i = end + 1;
            } else {
                break;
            }
        } else {
            i += 1;
        }
    }
    out
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
        // nvidia-smi (8-hex domain), sysfs (4-hex domain) and lspci (no
        // domain) all refer to the same device and must compare equal.
        assert_eq!(pci_match_key("00000000:01:00.0"), "0:01:00.0");
        assert_eq!(pci_match_key("0000:01:00.0"), "0:01:00.0");
        assert_eq!(pci_match_key("01:00.0"), "0:01:00.0");
        assert_eq!(pci_match_key("weird"), "weird");
    }

    #[test]
    fn pci_key_keeps_domains_distinct() {
        // Different PCI domains at the same bus/slot/function are different
        // devices and must not collide.
        assert_ne!(pci_match_key("0000:01:00.0"), pci_match_key("0001:01:00.0"));
    }

    #[test]
    fn lspci_mm_parses() {
        // Real `lspci -mm` output: whitespace-separated, quoted fields.
        let out = "00:02.0 \"VGA compatible controller\" \"Intel Corporation\" \"UHD Graphics 770\" -r04\n\
                   01:00.0 \"3D controller\" \"NVIDIA Corporation\" \"GA102 [GeForce RTX 3090]\" -r01\n";
        let map = parse_lspci_mm(out);
        assert_eq!(
            map.get("00:02.0").map(String::as_str),
            Some("UHD Graphics 770")
        );
        assert_eq!(
            map.get("01:00.0").map(String::as_str),
            Some("GA102 [GeForce RTX 3090]")
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
