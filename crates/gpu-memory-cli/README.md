# gpu-memory (CLI)

A tiny command-line tool that prints the total and available memory of every
GPU on the machine, labeling each as **discrete** (dedicated VRAM) or
**unified** (shared system memory).

It is the example binary for the [`gpu-memory`](../gpu-memory/README.md) crate.

## Install

See the [project README](../../README.md) for one-liner installers for macOS,
Linux, and Windows.

## Usage

```sh
gpu-memory            # human-readable table
gpu-memory --json     # machine-readable JSON (alias: -j)
gpu-memory --version  # print the version (alias: -V)
```

### Table output

```
NAME                        VENDOR  KIND          TOTAL   AVAILABLE
-------------------------------------------------------------------
Apple GPU (unified memory)  apple   unified   128.0 GiB    10.3 GiB
```

- **KIND** is `discrete` or `unified`.
- **TOTAL** / **AVAILABLE** are the device's total and currently-available
  memory. For a discrete GPU this is VRAM; for a unified GPU it is the shared
  system memory.

### JSON output

```json
[
  {
    "name": "Apple GPU (unified memory)",
    "vendor": "Apple",
    "memory": {
      "kind": "Unified",
      "total_bytes": 137438953472,
      "available_bytes": 11009540096
    },
    "device_id": null
  }
]
```

Byte counts are integers (`u64`), or `null` when the platform can't report
the value — safe to consume in scripts.

## Exit codes

- `0` — at least one GPU was detected (or `--version` was requested).
- `1` — no GPU could be detected, or detection failed.

## Building from source

```sh
cargo build --release
./target/release/gpu-memory
```

## License

MIT
