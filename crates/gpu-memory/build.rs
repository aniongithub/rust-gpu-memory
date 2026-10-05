//! Injects the git tag as the crate's reported version at build time — the
//! cargo equivalent of CMake's version injection.
//!
//! The version is read from `git describe --tags --always --dirty` (the
//! leading `v` is stripped, so `v0.1.0` becomes `0.1.0`). When git is
//! unavailable — e.g. building from a crates.io tarball, which has no `.git`
//! directory — it falls back to the manifest version (`CARGO_PKG_VERSION`).
//!
//! The result is exported as the `GPU_MEMORY_VERSION` compile-time env var,
//! consumed by [`gpu_memory::version`].

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());

    let version = git_describe_version().unwrap_or_else(|| {
        // Fallback: the manifest version. `CARGO_PKG_VERSION` is set by cargo
        // for build scripts and is always present.
        std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string())
    });

    // Re-run when build.rs changes, and — when building inside a git checkout —
    // when the checked-out commit changes, so the injected version stays fresh.
    // (Cutting a tag on the *same* commit does not change `.git/HEAD`; a
    // `touch build.rs` or `cargo clean` refreshes it. The release workflow
    // always checks out the tag fresh, so this is a non-issue there.)
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(head) = find_git_head(&manifest_dir) {
        println!("cargo:rerun-if-changed={}", head.display());
    }

    println!("cargo:rustc-env=GPU_MEMORY_VERSION={version}");
}

/// Runs `git describe --tags --always --dirty` and returns the version with
/// any leading `v` stripped. Returns `None` if git is unavailable or the
/// command fails (e.g. no `.git` in a crates.io tarball).
fn git_describe_version() -> Option<String> {
    let out = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let v = v.trim_start_matches('v').to_string();
    if v.is_empty() {
        return None;
    }
    Some(v)
}

/// Walks up from `start` looking for a `.git/HEAD` file, returning its
/// absolute path if found. Used to register a `rerun-if-changed` hook so the
/// build script re-runs when the checkout changes.
fn find_git_head(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let head = d.join(".git").join("HEAD");
        if head.is_file() {
            return Some(head);
        }
        dir = d.parent();
    }
    None
}
