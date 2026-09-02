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
        "termrock — offline source registry CLI + environment tools\n\
         \n\
         Usage:\n\
           termrock doctor [--profile modern|compatible|minimal|inline|headless]\n\
           termrock contract list\n\
           termrock contract check\n\
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

fn run_contract(args: &[String]) -> ExitCode {
    use termrock::registry::{ContractIssueLevel, official_kernel_contracts, validate_contracts};

    let sub = args.first().map(String::as_str).unwrap_or("check");
    match sub {
        "list" => {
            for c in official_kernel_contracts() {
                println!(
                    "{:<22} {:<10} complete={} stories={} {}",
                    c.id,
                    c.kind.id(),
                    c.complete,
                    c.stories.len(),
                    c.module.as_deref().unwrap_or("-")
                );
            }
            ExitCode::SUCCESS
        }
        "check" | _ if sub == "check" || args.is_empty() => {
            let catalog = official_kernel_contracts();
            let report = validate_contracts(&catalog);
            for issue in &report.issues {
                let tag = match issue.level {
                    ContractIssueLevel::Error => "error",
                    ContractIssueLevel::Warning => "warn",
                    _ => "info",
                };
                eprintln!(
                    "{tag}: {} [{}] {}",
                    issue.contract_id, issue.code, issue.message
                );
            }
            println!(
                "contracts: {} items, {} errors, {} warnings",
                catalog.len(),
                report.error_count(),
                report.issues.len().saturating_sub(report.error_count())
            );
            if report.ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        "-h" | "--help" => usage(),
        other => {
            eprintln!("error: unknown contract subcommand {other} (list|check)");
            ExitCode::FAILURE
        }
    }
}

fn run_doctor(args: &[String]) -> ExitCode {
    use termrock::capability::{
        CapabilityOverrides, CapabilityProfile, build_doctor_report, format_doctor_text,
    };

    let mut preferred: Option<CapabilityProfile> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--profile" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --profile requires a value");
                    return ExitCode::FAILURE;
                }
                preferred = CapabilityProfile::parse(&args[i]);
                if preferred.is_none() {
                    eprintln!(
                        "error: unknown profile {:?} (modern|compatible|minimal|inline|headless)",
                        args[i]
                    );
                    return ExitCode::FAILURE;
                }
            }
            "-h" | "--help" => usage(),
            other => {
                eprintln!("error: unknown doctor flag {other}");
                return ExitCode::FAILURE;
            }
        }
        i += 1;
    }

    // Merge env overrides (TERMROCK_*, NO_COLOR) with optional profile preference.
    let mut overrides = CapabilityOverrides::from_env_keys(
        env::var("TERMROCK_COLOR").ok().as_deref(),
        env::var("TERMROCK_PROFILE").ok().as_deref(),
    );
    if preferred.is_some() {
        overrides.profile = preferred;
    }
    let report = build_doctor_report(preferred, overrides);
    print!("{}", format_doctor_text(&report));
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let cmd = args.remove(0);

    if cmd == "doctor" {
        return run_doctor(&args);
    }
    if cmd == "contract" {
        return run_contract(&args);
    }
    if matches!(cmd.as_str(), "-h" | "--help") {
        usage();
    }

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
