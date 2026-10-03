//! Example CLI for the `gpu-memory` crate.
//!
//! Prints a table of GPUs with total and available memory, or JSON with
//! `--json`.

use gpu_memory::{list_gpus, Gpu};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let as_json = args.iter().any(|a| a == "--json" || a == "-j");

    match list_gpus() {
        Ok(gpus) => {
            if as_json {
                match serde_json::to_string_pretty(&gpus) {
                    Ok(json) => {
                        println!("{json}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("error: failed to serialize JSON: {e}");
                        ExitCode::FAILURE
                    }
                }
            } else {
                print_table(&gpus);
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_table(gpus: &[Gpu]) {
    if gpus.is_empty() {
        println!("No GPUs found.");
        return;
    }

    // Column widths.
    let name_w = gpus.iter().map(|g| g.name.len()).max().unwrap_or(0).max(4);
    let vendor_w = gpus.iter().map(|g| g.vendor.as_str().len()).max().unwrap_or(0).max(6);
    let kind_w = gpus
        .iter()
        .map(|g| g.memory.kind.as_str().len())
        .max()
        .unwrap_or(0)
        .max(4);

    let header = format!(
        "{:<name_w$}  {:<vendor_w$}  {:<kind_w$}  {:>10}  {:>10}",
        "NAME",
        "VENDOR",
        "KIND",
        "TOTAL",
        "AVAILABLE",
    );
    println!("{header}");
    let sep = "-".repeat(header.len());
    println!("{sep}");

    for g in gpus {
        let total = gpu_memory::parse::format_bytes(g.memory.total_bytes);
        let avail = gpu_memory::parse::format_bytes(g.memory.available_bytes);
        let kind = g.memory.kind.as_str();
        println!(
            "{:<name_w$}  {:<vendor_w$}  {:<kind_w$}  {:>10}  {:>10}",
            g.name,
            g.vendor.as_str(),
            kind,
            total,
            avail,
        );
    }
}
