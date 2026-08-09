// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! `termrock` CLI entry.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use termrock_cli::{
    InstallManifest, PlanAction, apply_plan, diff_installed, load_entry, plan_install,
};

fn usage() -> ! {
    eprintln!(
        "termrock — offline source registry CLI\n\
         \n\
         Usage:\n\
           termrock plan  <entry-dir> [--workspace DIR]\n\
           termrock add   <entry-dir> [--workspace DIR] [--force]\n\
           termrock diff  <entry-dir> [--workspace DIR]\n\
           termrock check <entry-dir>\n\
         \n\
         Non-interactive mutations require an explicit command (add).\n\
         Never silently overwrites dirty destinations without --force."
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let cmd = args.remove(0);
    let mut workspace = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut force = false;
    let mut entry_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--workspace" => {
                i += 1;
                if i >= args.len() {
                    usage();
                }
                workspace = PathBuf::from(&args[i]);
            }
            "--force" => force = true,
            "-h" | "--help" => usage(),
            other if !other.starts_with('-') && entry_dir.is_none() => {
                entry_dir = Some(PathBuf::from(other));
            }
            _ => usage(),
        }
        i += 1;
    }
    let Some(entry_dir) = entry_dir else {
        usage();
    };

    match cmd.as_str() {
        "plan" | "add" | "diff" | "check" => {}
        _ => usage(),
    }

    let entry = match load_entry(&entry_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if cmd == "check" {
        println!(
            "ok {}@{} ({} files, schema {})",
            entry.name,
            entry.version,
            entry.files.len(),
            entry.schema
        );
        return ExitCode::SUCCESS;
    }

    let plan = plan_install(&entry, &entry_dir, &workspace, force);
    if cmd == "plan" || cmd == "add" {
        println!("plan {}@{}", entry.name, entry.version);
        for a in &plan.actions {
            match a {
                PlanAction::Create {
                    dest,
                    sha256,
                    force_overwrite,
                    ..
                } => {
                    let tag = if *force_overwrite {
                        "FORCE+BACKUP"
                    } else {
                        "CREATE"
                    };
                    println!(
                        "  {tag} {} ({})",
                        dest.display(),
                        &sha256[..12.min(sha256.len())]
                    );
                }
                PlanAction::Conflict { dest, reason } => {
                    println!("  CONFLICT {}: {reason}", dest.display());
                }
                PlanAction::Unchanged { dest } => {
                    println!("  UNCHANGED {}", dest.display());
                }
            }
        }
        for e in &plan.errors {
            println!("  ERROR {e}");
        }
    }

    if cmd == "plan" {
        return if plan.errors.is_empty()
            && !plan
                .actions
                .iter()
                .any(|a| matches!(a, PlanAction::Conflict { .. }))
        {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }

    if cmd == "add" {
        let manifest = workspace.join("termrock.lock.json");
        match apply_plan(&plan, &workspace, &entry, &manifest) {
            Ok(()) => {
                println!("installed {}@{}", entry.name, entry.version);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    } else {
        // diff
        let manifest_path = workspace.join("termrock.lock.json");
        let manifest = std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|s| serde_json::from_str::<InstallManifest>(&s).ok())
            .unwrap_or_default();
        let report = diff_installed(&entry, &workspace, &manifest);
        if report.is_empty() {
            println!("{}: clean", entry.name);
            ExitCode::SUCCESS
        } else {
            for line in report {
                println!("{line}");
            }
            ExitCode::from(1)
        }
    }
}
