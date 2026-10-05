# gpu-memory

Platform-independent reporting of **total** and **available** GPU memory in
Rust, with correct handling of **discrete** (dedicated VRAM) vs. **unified /
UMA** GPUs.

| Kind | What it is | `total_bytes` | `available_bytes` |
| --- | --- | --- | --- |
| **Discrete** | Dedicated VRAM on a discrete GPU (e.g. RTX 4090) | VRAM size | Free VRAM |
| **Unified (UMA)** | GPU shares system RAM (integrated GPUs, Apple silicon, NVIDIA GB10 "Spark") | Total system RAM | Available system RAM |

A naive implementation reports the small VRAM carve-out on a UMA machine and
calls it a day. This project detects the special cases and reports the full
picture, so callers can tell the two apart.

## Install

### CLI — one-liner

**macOS / Linux** (installs to `~/.local/bin`):

```sh
curl -fsSL https://raw.githubusercontent.com/aniongithub/rust-gpu-memory/main/install.sh | sh
```

**Windows** (PowerShell, installs to `%LOCALAPPDATA%\gpu-memory\bin`):

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

> The repository is currently **private**, so the scripts need a token with
> read access: `export GH_TOKEN=...` (or `GITHUB_TOKEN`) before running. Once
> the repo is public, the same one-liners work with no token.

Pin a version with `GPU_MEMORY_VERSION=v0.1.0` (env var, both scripts).

### CLI — manual

Download a prebuilt binary from the [releases](../../releases) page:

| Asset | Platform |
| --- | --- |
| `gpu-memory-aarch64-apple-darwin.tar.gz` | macOS (Apple Silicon) |
| `gpu-memory-x86_64-apple-darwin.tar.gz` | macOS (Intel) |
| `gpu-memory-x86_64-unknown-linux-gnu.tar.gz` | Linux (x86_64) |
| `gpu-memory-aarch64-unknown-linux-gnu.tar.gz` | Linux (ARM64, e.g. DGX Spark) |
| `gpu-memory-x86_64-pc-windows-msvc.zip` | Windows (x86_64) |
| `gpu-memory-aarch64-pc-windows-msvc.zip` | Windows (ARM64, e.g. RTX Spark) |

### Library

```toml
[dependencies]
gpu-memory = "0.1"
```

## Documentation

- **Developers** — the [`gpu-memory` crate README](crates/gpu-memory/README.md):
  API, semantics, discrete vs. unified behavior, building, and repository
  layout.
- **CLI users** — the [`gpu-memory` CLI README](crates/gpu-memory-cli/README.md):
  usage, flags, and output formats.

## Supported platforms

| OS | Targets | Notes |
| --- | --- | --- |
| Linux | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | sysfs + `nvidia-smi` (when present) + `lspci` |
| macOS | `x86_64-apple-darwin`, `aarch64-apple-darwin` | `libc` (IOKit / sysctl), unified memory |
| Windows | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` | DXGI + registry + `nvidia-smi` (when present) |

Other targets compile to a stub that returns `UnsupportedPlatform`.

### GPU vendor support

What you get for each vendor on each OS. **Discrete** = dedicated VRAM;
**Unified** = shared system RAM.

| Vendor | Linux | macOS | Windows |
| --- | --- | --- | --- |
| **NVIDIA** | Discrete · total + free ¹ | — | Discrete · total + free ¹ |
| **AMD** | Discrete · total + free | — | Discrete · total only |
| **Intel** | Discrete (Arc) · total + free · Unified (iGPU) · total + avail | — | Discrete · total only · Unified (iGPU) · total + avail |
| **Apple** | — | Unified · total + avail | — |
| **Other** | Listed, no memory | — | Listed, total only if discrete |

- **total + free** / **total + avail** — both `total_bytes` and `available_bytes` are populated.
- **total only** — only `total_bytes` is populated; the platform API doesn't expose free VRAM.
- ¹ Free VRAM comes from `nvidia-smi` (must be on `PATH`). Without it, Linux lists the card with no memory and Windows falls back to DXGI (total only).
- **macOS** reports a single unified Apple GPU (system RAM via `sysctl`) and does not enumerate discrete GPUs, so discrete AMD/NVIDIA on Intel Macs are not reported.

### Special UMA accelerators (NVIDIA GB10 "Spark")

The GB10 Grace-Blackwell SoC is a unified-memory platform. Generic
enumeration would misreport it as a discrete GPU with a small VRAM carve-out,
so it is detected explicitly and reported as **unified** with the full system
memory:

- **DGX Spark** (Linux, aarch64) — detected by system model / vendor.
- **RTX Spark** (Windows) — detected by GPU adapter name / system model.

Detection is a pure, unit-tested function over a small set of machine
signals, so it runs on any host.

## License

MIT
