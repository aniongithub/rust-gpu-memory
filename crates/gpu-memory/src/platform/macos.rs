//! macOS implementation.
//!
//! Apple silicon (M-series) is always unified memory: the GPU shares the
//! system's RAM, so we report a single [`MemoryKind::Unified`] GPU.
//!
//! - Total RAM: `sysctlbyname("hw.memsize")`.
//! - Available RAM: `vm.page_free_count` + `vm.page_speculative_count` +
//!   `vm.page_purgeable_count`, times the default page size
//!   (`hw.pagesize`; 16 KiB on Apple silicon).
//!
//! All access goes through `libc` — no process spawning, no output parsing.

use crate::{Error, Gpu, GpuVendor, MemoryInfo, MemoryKind, Result};

/// Reads a sysctl that returns an unsigned integer, as `u64`.
///
/// The kernel reports the actual size of the value (4 bytes for `u32`
/// counters, 8 for `u64`); we interpret the returned bytes as a
/// little-endian integer, which covers both.
fn sysctl_u64(name: &str) -> Option<u64> {
    unsafe {
        let c_name = std::ffi::CString::new(name).ok()?;
        let mut buf = [0u8; 16];
        let mut size = buf.len();
        let rc = libc::sysctlbyname(
            c_name.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        );
        if rc != 0 {
            return None;
        }
        let mut value: u64 = 0;
        for (i, &b) in buf[..size].iter().enumerate() {
            value |= (b as u64) << (8 * i);
        }
        Some(value)
    }
}

/// Total physical memory in bytes.
fn total_ram() -> Option<u64> {
    sysctl_u64("hw.memsize")
}

/// Available memory in bytes.
fn available_ram() -> Option<u64> {
    let free = sysctl_u64("vm.page_free_count")?;
    let speculative = sysctl_u64("vm.page_speculative_count").unwrap_or(0);
    let purgeable = sysctl_u64("vm.page_purgeable_count").unwrap_or(0);
    // 16 KiB on Apple silicon, 4 KiB on Intel Macs.
    let page_size = sysctl_u64("hw.pagesize").unwrap_or(16384);
    Some((free + speculative + purgeable) * page_size)
}

pub fn list_gpus() -> Result<Vec<Gpu>> {
    let total = total_ram().ok_or_else(|| Error::Platform {
        detail: "could not read hw.memsize".to_string(),
    })?;
    let available = available_ram();
    Ok(vec![Gpu {
        name: "Apple GPU (unified memory)".to_string(),
        vendor: GpuVendor::Apple,
        memory: MemoryInfo {
            kind: MemoryKind::Unified,
            total_bytes: Some(total),
            available_bytes: available,
        },
        device_id: None,
    }])
}
