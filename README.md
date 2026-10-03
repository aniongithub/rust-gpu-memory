# gpu-memory

Platform-independent reporting of **total** and **available** GPU memory in
Rust, with correct handling of **discrete** (dedicated VRAM) vs. **unified /
UMA** GPUs.

Ships as:

- **`gpu-memory`** — the library crate (this is what you depend on).
- **`gpu-memory-cli`** — a small example CLI (`gpu-memory`) that prints a
  table or JSON.

## Why this exists

"Available GPU memory" means different things depending on the hardware:

| Kind | What it is | `total_bytes` | `available_bytes` |
| --- | --- | --- | --- |
| **Discrete** | Dedicated VRAM on a discrete GPU (e.g. RTX 4090) | VRAM size | Free VRAM |
| **Unified (UMA)** | GPU shares system RAM (integrated GPUs, Apple silicon, NVIDIA GB10 "Spark") | Total system RAM | Available system RAM |

A naive implementation reports the small VRAM carve-out on a UMA machine and
calls it a day. This crate detects the special cases and reports the full
picture, so callers can tell the two apart.

## Supported platforms

| OS | Targets | Notes |
| --- | --- | --- |
| Linux | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | sysfs + `nvidia-smi` (when present) + `lspci` |
| macOS | `x86_64-apple-darwin`, `aarch64-apple-darwin` | `libc` (IOKit / sysctl), unified memory |
| Windows | `x86_64-pc-windows-msvc` | DXGI + registry + `nvidia-smi` (when present) |

Other targets compile to a stub that returns `UnsupportedPlatform`.

### Special UMA accelerators (NVIDIA GB10 "Spark")

The GB10 Grace-Blackwell SoC is a unified-memory platform. Generic
enumeration would misreport it as a discrete GPU with a small VRAM carve-out,
so it is detected explicitly and reported as **unified** with the full system
memory:

- **DGX Spark** (Linux, aarch64) — detected by system model / vendor.
- **RTX Spark** (Windows) — detected by GPU adapter name / system model.

Detection is a pure, unit-tested function over a small set of machine
signals, so it runs on any host.

## Install

### CLI

Download a prebuilt binary from the [releases](../../releases) page, or build
from source:

```sh
cargo install --path crates/gpu-memory-cli
```

### Library

```toml
[dependencies]
gpu-memory = "0.1"
```

## Usage

### CLI

```console
$ gpu-memory
NAME                        VENDOR  KIND          TOTAL   AVAILABLE
-------------------------------------------------------------------
Apple GPU (unified memory)  apple   unified   128.0 GiB    17.7 GiB

$ gpu-memory --json
[
  {
    "name": "Apple GPU (unified memory)",
    "vendor": "Apple",
    "memory": {
      "kind": "Unified",
      "total_bytes": 137438953472,
      "available_bytes": 18981224448
    },
    "device_id": null
  }
]
```

### Library

```rust
use gpu_memory::{list_gpus, MemoryKind};

for gpu in list_gpus().expect("no GPUs found") {
    // `available_bytes` is `Option<u64>`; `format_bytes` renders `None` as
    // "unknown".
    let free = gpu_memory::parse::format_bytes(gpu.memory.available_bytes);
    match gpu.memory.kind {
        MemoryKind::Discrete => println!("{}: {} free VRAM", gpu.name, free),
        MemoryKind::Unified  => println!("{}: {} free system RAM (shared)", gpu.name, free),
        // `MemoryKind` is `#[non_exhaustive]`, so a wildcard arm is required.
        _ => println!("{}: {} (unrecognized kind)", gpu.name, free),
    }
}
```

The public API is a single entry point, [`list_gpus()`], returning
`Result<Vec<Gpu>>`. Each [`Gpu`] carries a name, vendor, a [`MemoryInfo`]
(kind + total/available bytes), and an optional platform device id.

## Semantics

- **`MemoryKind::Discrete`** — `total_bytes` is the dedicated VRAM size and
  `available_bytes` is the currently free VRAM.
- **`MemoryKind::Unified`** — the GPU shares system RAM. `total_bytes` is
  total system RAM (or the platform-reported GPU share) and
  `available_bytes` is available system RAM. **Do not treat this as
  dedicated VRAM.**

Byte fields are `Option<u64>` because not every platform can report every
value (e.g. Windows DXGI reports total dedicated memory but not free VRAM;
free VRAM there comes from `nvidia-smi` when present).

## Building

```sh
cargo build --release
cargo test
```

Cross-compile checks (no linking required):

```sh
rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin x86_64-apple-darwin
cargo check --target x86_64-pc-windows-msvc
cargo check --target aarch64-apple-darwin
```

## Repository layout

```
crates/
  gpu-memory/        # the library
    src/
      lib.rs         # public API (list_gpus)
      types.rs       # Gpu, GpuVendor, MemoryInfo, MemoryKind
      error.rs       # Error / Result
      detect.rs      # UMA (GB10 "Spark") detection — pure + unit-tested
      parse.rs       # byte formatting + parsers (meminfo, nvidia-smi, lspci)
      platform/      # per-OS impls (linux, macos, windows) + dispatch
  gpu-memory-cli/    # example CLI (bin: gpu-memory)
```

## License

MIT
