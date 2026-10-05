# gpu-memory

A small, dependency-light Rust library that reports **total** and **available**
GPU memory, correctly distinguishing **discrete** GPUs (dedicated VRAM) from
**unified / UMA** GPUs (shared system memory).

> **Looking to install the command-line tool?** See the
> [CLI README](../gpu-memory-cli/README.md) or the
> [project README](../../README.md) for one-liner installers.

## Why

Getting "how much GPU memory is free" is deceptively platform-specific:

- **Discrete GPUs** (NVIDIA, AMD dGPU, Apple's discrete history) expose a
  dedicated VRAM pool. "Available" means free VRAM.
- **Unified / UMA GPUs** (Apple Silicon, Intel iGPU, AMD APU, NVIDIA GB10
  "Spark") share the system's main memory. There is no separate VRAM pool, so
  "available" means free *system* memory that the GPU can use.

`gpu-memory` normalizes both cases behind one API and labels each device with
a `MemoryKind` so callers can tell the two apart.

## Add it to your project

```toml
[dependencies]
gpu-memory = "0.1"
```

The crate will be published to crates.io as [`gpu-memory`](https://crates.io/crates/gpu-memory);
until then, depend on it by path or git:

```toml
[dependencies]
gpu-memory = { git = "https://github.com/aniongithub/rust-gpu-memory.git", tag = "v0.1.0" }
```

## Usage

```rust
use gpu_memory::{list_gpus, MemoryKind};

for gpu in list_gpus().expect("no GPUs found") {
    // Byte fields are `Option<u64>`; not every platform can report every
    // value. `format_bytes` renders `None` as "unknown".
    let total = gpu_memory::parse::format_bytes(gpu.memory.total_bytes);
    let avail = gpu_memory::parse::format_bytes(gpu.memory.available_bytes);

    match gpu.memory.kind {
        MemoryKind::Discrete => {
            println!("{gpu:?}: {avail} dedicated VRAM free (of {total})");
        }
        MemoryKind::Unified => {
            println!("{gpu:?}: {avail} unified memory free (of {total})");
        }
        // `MemoryKind` is `#[non_exhaustive]`, so a wildcard arm is required.
        _ => println!("{gpu:?}: {avail} free (of {total})"),
    }
}
```

`list_gpus()` returns `Result<Vec<Gpu>, Error>`. It returns an error only when
no GPU could be detected at all; individual per-device failures are skipped
rather than failing the whole call.

## API

| Item | Description |
| --- | --- |
| `list_gpus() -> Result<Vec<Gpu>, Error>` | Enumerate all detected GPUs with their memory info. |
| `version() -> &'static str` | The crate version, injected at build time from the git tag. |
| `Gpu` | A detected device: `name`, `vendor`, `memory`, and an optional `device_id`. |
| `MemoryInfo` | `kind` (`MemoryKind`), `total_bytes`, `available_bytes`. |
| `MemoryKind` | `Discrete` (dedicated VRAM) or `Unified` (shared system memory). |
| `GpuVendor` | `Nvidia`, `Amd`, `Apple`, `Intel`, or `Unknown`. |
| `Error` / `Result<T>` | The crate's error type and alias. |

Byte counts are `Option<u64>` because not every platform can report every
value (e.g. Windows DXGI reports total dedicated memory but not free VRAM;
free VRAM there comes from `nvidia-smi` when present). Use
`parse::format_bytes` to render them, which shows `None` as `"unknown"`.

## How it works per platform

The public API is identical everywhere; the detection backend is selected at
compile time via `#[cfg]`:

- **Linux** — reads sysfs (`/sys/class/drm`, `/sys/class/hwmon`,
  `/sys/devices/system/memory`) for AMD/Intel, shells out to `nvidia-smi` for
  NVIDIA, and falls back to `lspci` for device enumeration.
- **macOS** — uses `libc` + IOKit to read the GPU and the unified-memory
  pressure, and `sysctl` for total system memory. Apple Silicon is reported as
  `Unified`.
- **Windows** — uses the `windows` crate (DXGI) for adapter enumeration and
  dedicated/shared system memory, plus the registry and `nvidia-smi` where
  available.

A pure, unit-tested detector classifies a device as discrete vs. unified (and
recognizes the NVIDIA GB10 "Spark" as unified) independent of the OS backend.

## Discrete vs. unified, in one line

- **Discrete** → `available_bytes` is free *VRAM*.
- **Unified** → `available_bytes` is free *system* memory the GPU can use.

Check `MemoryKind` before interpreting the numbers.

## Building

```sh
cargo build --release
cargo test
```

The version string is injected by `build.rs` from the git tag
(`git describe --tags --always --dirty`), falling back to the manifest version
when built outside a git checkout (e.g. from crates.io).

## License

MIT
