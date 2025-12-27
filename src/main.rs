/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unused_parens)]

use alpm::TransFlag;
use anyhow::{anyhow, Result};
use clap::Parser;
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

mod alpm_manager;
mod aur;
mod config;
mod downloader;
mod error;
mod logging;
mod reflector;
mod updater;

// New modules
mod appimage;
mod containers;
mod flatpak;
mod snap;

mod deps;
mod rollback;
mod security;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\n",
    "Copyright (C) 2025  compiledkernel-idk and pacboost contributors\n",
    "License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>\n\n",
    "This is free software; you are free to change and redistribute it.\n",
    "There is NO WARRANTY, to the extent permitted by law."
);

#[derive(Parser)]
#[command(name = "pacboost")]
#[command(author = "PacBoost Team")]
#[command(version = VERSION)]
#[command(long_version = LONG_VERSION)]
#[command(about = "High-performance Arch Linux package manager frontend.")]
struct Cli {
    #[arg(short = 'S', long)]
    sync: bool,
    #[arg(short = 'R', long)]
    remove: bool,
    #[arg(short = 's', long)]
    search: bool,
    #[arg(short = 'y', long, action = clap::ArgAction::Count)]
    refresh: u8,
    #[arg(short = 'u', long, action = clap::ArgAction::Count)]
    sys_upgrade: u8,
    #[arg(short = 'r', long)]
    recursive: bool,
    #[arg(short = 'j', long, default_value_t = 4)]
    jobs: usize,
    #[arg(short = 'A', long)]
    aur: bool,
    #[arg(long)]
    history: bool,
    #[arg(long)]
    clean: bool,
    #[arg(long)]
    news: bool,
    #[arg(long)]
    health: bool,
    #[arg(long)]
    rank_mirrors: bool,
    #[arg(long)]
    clean_orphans: bool,
    #[arg(long)]
    info: bool,
    #[arg(long, help = "Benchmark mirror download speeds")]
    benchmark: bool,
    #[arg(long, help = "Bypass any confirmation prompts")]
    noconfirm: bool,
    #[arg(long, help = "Generate a technical system and networking report")]
    sys_report: bool,

    #[arg(
        short = 'w',
        long = "downloadonly",
        help = "Download packages but do not install/upgrade anything"
    )]
    downloadonly: bool,

    // TUI
    #[arg(short = 'T', long, help = "TUI mode (removed)")]
    tui: bool,

    // Flatpak commands
    #[arg(long, help = "Install a Flatpak application")]
    flatpak_install: Option<String>,
    #[arg(long, help = "Remove a Flatpak application")]
    flatpak_remove: Option<String>,
    #[arg(long, help = "Search Flatpak applications")]
    flatpak_search: Option<String>,
    #[arg(long, help = "Update all Flatpak applications")]
    flatpak_update: bool,
    #[arg(long, help = "List installed Flatpak applications")]
    flatpak_list: bool,

    // Snap commands
    #[arg(long, help = "Install a Snap package")]
    snap_install: Option<String>,
    #[arg(long, help = "Remove a Snap package")]
    snap_remove: Option<String>,
    #[arg(long, help = "Search Snap packages")]
    snap_search: Option<String>,
    #[arg(long, help = "Refresh all Snap packages")]
    snap_refresh: bool,
    #[arg(long, help = "List installed Snap packages")]
    snap_list: bool,

    // AppImage commands
    #[arg(long, help = "Install an AppImage from URL")]
    appimage_install: Option<String>,
    #[arg(long, help = "List installed AppImages")]
    appimage_list: bool,
    #[arg(long, help = "Remove an AppImage")]
    appimage_remove: Option<String>,

    // Security
    #[arg(long, help = "Check for CVE vulnerabilities")]
    check_cve: bool,
    #[arg(long, help = "Enable sandbox for AUR builds")]
    sandbox: bool,
    #[arg(long, help = "Show security scan of PKGBUILD")]
    security_scan: Option<String>,

    // Rollback
    #[arg(long, help = "Create a system snapshot before operation")]
    snapshot: bool,
    #[arg(long, help = "List system snapshots")]
    snapshots: bool,
    #[arg(long, help = "Rollback to a snapshot by ID")]
    rollback_to: Option<u32>,

    // Download options
    #[arg(long, help = "Download rate limit in KB/s")]
    rate_limit: Option<u64>,
    #[arg(long, help = "Show cache statistics")]
    cache_stats: bool,

    // Lock file
    #[arg(long, help = "Generate a lock file")]
    lock: bool,
    #[arg(long, help = "Check lock file differences")]
    lock_diff: bool,

    #[arg(value_name = "TARGETS")]
    targets: Vec<String>,
}
fn handle_lock_file() -> Result<()> {
    let lock_path = Path::new("/var/lib/pacman/db.lck");
    if !lock_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(lock_path).unwrap_or_default();
    let trimmed = content.trim();
    if trimmed.is_empty() {
        println!("{}", style(":: removing stale lock file...").yellow());
        let _ = fs::remove_file(lock_path);
        return Ok(());
    }
    match trimmed.parse::<i32>() {
        Ok(pid) => {
            if !Path::new(&format!("/proc/{}", pid)).exists() {
                println!(
                    "{}",
                    style(format!(":: removing stale lock (pid {})...", pid)).yellow()
                );
                let _ = fs::remove_file(lock_path);
                Ok(())
            } else {
                Err(anyhow!("database locked by running process {}", pid))
            }
        }
        Err(_) => {
            println!("{}", style(":: removing corrupt lock file...").yellow());
            let _ = fs::remove_file(lock_path);
            Ok(())
        }
    }
}

fn handle_corrupt_db() -> Result<()> {
    let local_path = Path::new("/var/lib/pacman/local");
    if local_path.exists() {
        for entry in fs::read_dir(local_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && !path.join("desc").exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains('-') {
                        println!(
                            "{}",
                            style(format!(":: cleaning corrupt entry: {}", name)).yellow()
                        );
                        let _ = fs::remove_dir_all(path);
                    }
                }
            }
        }
    }
    Ok(())
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // If no arguments, default to -Syu
    let mut sync = cli.sync;
    let mut sys_upgrade = cli.sys_upgrade;
    let mut refresh = cli.refresh;
    let targets = cli.targets.clone();

    let has_action = sync
        || sys_upgrade > 0
        || cli.remove
        || cli.search
        || cli.aur
        || cli.history
        || cli.clean
        || cli.news
        || cli.health
        || cli.rank_mirrors
        || cli.clean_orphans
        || cli.info
        || cli.benchmark
        || cli.tui
        || cli.flatpak_install.is_some()
        || cli.flatpak_remove.is_some()
        || cli.flatpak_search.is_some()
        || cli.flatpak_update
        || cli.flatpak_list
        || cli.snap_install.is_some()
        || cli.snap_remove.is_some()
        || cli.snap_search.is_some()
        || cli.snap_refresh
        || cli.snap_list
        || cli.appimage_install.is_some()
        || cli.appimage_list
        || cli.appimage_remove.is_some()
        || cli.check_cve
        || cli.security_scan.is_some()
        || cli.snapshot
        || cli.snapshots
        || cli.rollback_to.is_some()
        || cli.cache_stats
        || cli.lock
        || cli.lock_diff
        || cli.sys_report
        || refresh > 0
        || !targets.is_empty();

    if !has_action {
        // Default to -Syu
        sync = true;
        sys_upgrade = 1;
        refresh = 1;
    }

    // TUI removed for smaller binary
    if cli.tui {
        println!("TUI has been removed. Use CLI commands instead.");
        return Ok(());
    }

    // Flatpak commands
    if cli.flatpak_list {
        if !flatpak::FlatpakClient::is_available() {
            return Err(anyhow!("Flatpak is not installed"));
        }
        let client = flatpak::FlatpakClient::new();
        let apps = client.list_apps()?;
        flatpak::display_apps(&apps);
        return Ok(());
    }
    if let Some(ref app_id) = cli.flatpak_install {
        let client = flatpak::FlatpakClient::new();
        return client.install(app_id);
    }
    if let Some(ref app_id) = cli.flatpak_remove {
        let client = flatpak::FlatpakClient::new();
        return client.remove(app_id);
    }
    if let Some(ref query) = cli.flatpak_search {
        let client = flatpak::FlatpakClient::new();
        let results = client.search(query)?;
        flatpak::display_search_results(&results);
        return Ok(());
    }
    if cli.flatpak_update {
        let client = flatpak::FlatpakClient::new();
        return client.update_all();
    }

    // Snap commands
    if cli.snap_list {
        if !snap::SnapClient::is_available() {
            return Err(anyhow!("Snap is not installed"));
        }
        let client = snap::SnapClient::new();
        let snaps = client.list()?;
        snap::display_snaps(&snaps);
        return Ok(());
    }
    if let Some(ref name) = cli.snap_install {
        let client = snap::SnapClient::new();
        return client.install(name);
    }
    if let Some(ref name) = cli.snap_remove {
        let client = snap::SnapClient::new();
        return client.remove(name);
    }
    if let Some(ref query) = cli.snap_search {
        let client = snap::SnapClient::new();
        let results = client.search(query)?;
        snap::display_search_results(&results);
        return Ok(());
    }
    if cli.snap_refresh {
        let client = snap::SnapClient::new();
        return client.refresh_all();
    }

    // AppImage commands
    if cli.appimage_list {
        let manager = appimage::AppImageManager::new();
        let apps = manager.list()?;
        appimage::display_appimages(&apps);
        return Ok(());
    }
    if let Some(ref url) = cli.appimage_install {
        let manager = appimage::AppImageManager::new();
        let name = url.split('/').next_back().unwrap_or("app");
        manager.install_from_url(name, url).await?;
        return Ok(());
    }
    if let Some(ref name) = cli.appimage_remove {
        let manager = appimage::AppImageManager::new();
        return manager.remove(name);
    }

    // Security commands
    if cli.check_cve {
        println!(
            "{} Checking for known vulnerabilities...",
            style("::").cyan().bold()
        );
        let cve_checker = security::CveChecker::new();
        // Check installed packages
        if let Ok(handle) = alpm::Alpm::new("/", "/var/lib/pacman") {
            let localdb = handle.localdb();
            let packages: Vec<_> = localdb
                .pkgs()
                .iter()
                .map(|p| (p.name().to_string(), p.version().to_string()))
                .collect();
            let vulns = cve_checker.check_packages(&packages).await?;
            if vulns.is_empty() {
                println!(
                    "{} No known vulnerabilities found",
                    style("✓").green().bold()
                );
            } else {
                for (pkg, issues) in &vulns {
                    println!(
                        "{} {} has {} known vulnerabilities:",
                        style("!").red().bold(),
                        style(pkg).yellow(),
                        issues.len()
                    );
                    security::cve::display_vulnerabilities(issues);
                }
            }
        }
        return Ok(());
    }
    if let Some(ref pkgbuild_path) = cli.security_scan {
        println!(
            "{} Scanning PKGBUILD: {}",
            style("::").cyan().bold(),
            pkgbuild_path
        );
        let content = fs::read_to_string(pkgbuild_path)?;
        let report = security::MalwareDetector::new().scan(&content);
        let sec_manager = security::SecurityManager::new();
        sec_manager.display_report(&security::SecurityReport {
            malware: Some(report.clone()),
            vulnerabilities: Vec::new(),
            trust_level: security::TrustLevel::Unknown,
            trust_score: report.score,
            overall_safe: report.is_safe(),
            warnings: Vec::new(),
            blockers: Vec::new(),
        });
        return Ok(());
    }

    // Rollback commands
    if cli.snapshots {
        let manager = rollback::RollbackManager::new();
        let snapshots = manager.list()?;
        rollback::display_snapshots(&snapshots);
        return Ok(());
    }
    if let Some(id) = cli.rollback_to {
        let manager = rollback::RollbackManager::new();
        return manager.rollback(id);
    }
    if cli.snapshot {
        let manager = rollback::RollbackManager::new();
        manager.create_snapshot(
            "manual",
            "User-created snapshot",
            rollback::SnapshotType::Manual,
        )?;
        return Ok(());
    }

    // Cache stats
    if cli.cache_stats {
        let cache_dir = dirs::cache_dir()
            .map(|p| p.join("pacboost"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/pacboost-cache"));
        let cache = downloader::cache::PackageCache::new(cache_dir, 2048)?;
        downloader::cache::display_cache_stats(cache.stats(), cache.hit_rate());
        return Ok(());
    }

    // Lock file commands
    if cli.lock {
        println!("{} Generating lock file...", style("::").cyan().bold());
        let mut lockfile = deps::lockfile::Lockfile::new();
        if let Ok(handle) = alpm::Alpm::new("/", "/var/lib/pacman") {
            for pkg in handle.localdb().pkgs() {
                lockfile.add_package(deps::lockfile::LockedPackage {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    epoch: None,
                    pkgrel: "1".to_string(),
                    arch: pkg.arch().unwrap_or("any").to_string(),
                    repository: pkg
                        .db()
                        .map(|d| d.name().to_string())
                        .unwrap_or_else(|| "local".to_string()),
                    sha256: None,
                    dependencies: pkg.depends().iter().map(|d| d.to_string()).collect(),
                    source: deps::lockfile::PackageSource::Official {
                        repo: "local".to_string(),
                    },
                });
            }
        }
        lockfile.save_default()?;
        println!("{} Lock file saved", style("✓").green().bold());
        return Ok(());
    }
    if cli.lock_diff {
        if let Ok(lockfile) = deps::lockfile::Lockfile::load_default() {
            if let Ok(handle) = alpm::Alpm::new("/", "/var/lib/pacman") {
                let installed: Vec<_> = handle
                    .localdb()
                    .pkgs()
                    .iter()
                    .map(|p| (p.name().to_string(), p.version().to_string()))
                    .collect();
                let diff = lockfile.diff(&installed);
                diff.display();
            }
        } else {
            println!(
                "{} No lock file found. Run with --lock first.",
                style("!").yellow().bold()
            );
        }
        return Ok(());
    }

    // Skip update check when noconfirm is set (for automated/benchmark scenarios)
    if !cli.noconfirm {
        if let Some(info) = updater::check_for_updates(VERSION) {
            println!(
                "{}",
                style(format!(
                    ":: a new version of pacboost is available: {} (current: {})",
                    info.version, VERSION
                ))
                .cyan()
                .bold()
            );
            use std::io::{self, Write};
            print!("   update now? [Y/n] ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if input.trim().is_empty() || input.trim().to_lowercase().starts_with('y') {
                if let Err(e) = updater::perform_update(info) {
                    eprintln!("{} update failed: {}", style("error:").red().bold(), e);
                } else {
                    println!("   restart pacboost to apply changes.");
                    return Ok(());
                }
            }
        }
    }
    let _ = handle_lock_file();
    // handle_corrupt_db is expensive, run only on health check

    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
        .template("{spinner:.cyan} {msg}")?;
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.set_message("initializing transaction...");
    pb.enable_steady_tick(Duration::from_millis(80));
    let mut manager = alpm_manager::AlpmManager::new()?;
    pb.finish_and_clear();

    // Print quick host info for context
    if (sync || sys_upgrade > 0 || !targets.is_empty()) && !cli.search && !cli.info {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_cpu_usage(); // This also gets CPU names usually
        sys.refresh_memory();

        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        let _os = System::name().unwrap_or_else(|| "Linux".to_string());
        let cpu = sys
            .cpus()
            .first()
            .map(|c| c.brand())
            .unwrap_or("Unknown CPU")
            .trim();
        let ram = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

        println!(
            "{} {} version {} on {}",
            style("::").bold().cyan(),
            style("pacboost").bold(),
            env!("CARGO_PKG_VERSION"),
            style(hostname).yellow(),
        );
        println!("   {} | {:.1} GB RAM", style(cpu).dim(), ram);
    }
    if cli.news {
        return fetch_arch_news().await;
    }
    if cli.history {
        return show_package_history();
    }
    if cli.health {
        let _ = handle_corrupt_db();
        return run_health_check();
    }
    if cli.sys_report {
        return run_system_report().await;
    }
    if cli.clean {
        return clean_cache();
    }
    if cli.rank_mirrors {
        return reflector::rank_mirrors(20).await;
    }
    if cli.clean_orphans {
        return clean_orphans(&mut manager);
    }
    if cli.info {
        if targets.is_empty() {
            return Err(anyhow!("no package specified for info"));
        }
        return show_package_info(&manager, &targets).await;
    }
    if cli.benchmark {
        // Get all configured mirrors from alpm_manager
        let mirrors = manager.get_all_mirrors();
        return downloader::run_benchmark(mirrors, 512).await.map(|_| ());
    }
    if cli.aur {
        return handle_aur_search(targets).await;
    }
    if sync && cli.search {
        let results = manager.search(targets.clone())?;
        if results.is_empty() {
            println!("no matches found.");
        } else {
            for pkg in results {
                let repo = pkg.db().map(|d| d.name()).unwrap_or("local");
                println!(
                    "{}/{} {} {}
    {}",
                    style(repo).cyan().bold(),
                    style(pkg.name()).bold(),
                    style(pkg.version().as_str()).green(),
                    style(format!("[installed: {}]", pkg.isize())).black(),
                    pkg.desc().unwrap_or("")
                );
            }
        }
        return Ok(());
    }
    if refresh > 0 {
        println!("{}", style(":: syncing databases...").bold());
        let mp = MultiProgress::new();
        manager
            .sync_dbs_manual(Some(mp), cli.jobs, refresh > 1)
            .await?;
    }
    let mut aur_targets = Vec::new();
    if targets.is_empty() && sys_upgrade == 0 {
        return Ok(());
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.set_message("resolving...");
    pb.enable_steady_tick(Duration::from_millis(80));
    let mut flags = TransFlag::empty();
    if cli.remove && cli.recursive {
        flags |= TransFlag::RECURSE;
    }
    manager
        .handle
        .trans_init(flags)
        .map_err(|e| anyhow!("failed to init trans: {}", e))?;
    if cli.remove {
        let local_db = manager.handle.localdb();
        for t in &targets {
            if let Ok(p) = local_db.pkg(t.as_str()) {
                manager
                    .handle
                    .trans_remove_pkg(p)
                    .map_err(|e| anyhow!("failed: {}", e))?;
            } else {
                return Err(anyhow!("target not found: {}", t));
            }
        }
    } else {
        if sys_upgrade > 0 {
            manager
                .handle
                .sync_sysupgrade(sys_upgrade > 1)
                .map_err(|e| anyhow!("failed: {}", e))?;
        }
        for t in &targets {
            // Easter egg: detect if user is trying to install pacboost with pacboost
            if t == "pacboost" || t == "pacboost-bin" {
                println!("{}", style("").bold());
                println!(
                    "{}",
                    style("╔═══════════════════════════════════════════════════════════════╗")
                        .cyan()
                        .bold()
                );
                println!(
                    "{}",
                    style("║   Why is dawg trying to install pacboost.                    ║")
                        .cyan()
                        .bold()
                );
                println!(
                    "{}",
                    style("║   I'm not sure why you would do that.                       ║")
                        .cyan()
                        .bold()
                );
                println!(
                    "{}",
                    style("║                                                               ║")
                        .cyan()
                        .bold()
                );
                println!(
                    "{}",
                    style("║   Maybe you should try something else man.                      ║")
                        .cyan()
                        .bold()
                );
                println!(
                    "{}",
                    style("╚═══════════════════════════════════════════════════════════════╝")
                        .cyan()
                        .bold()
                );
                println!("{}", style("").bold());
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }

            let mut found = false;
            for db in manager.handle.syncdbs() {
                if let Ok(p) = db.pkg(t.as_str()) {
                    manager
                        .handle
                        .trans_add_pkg(p)
                        .map_err(|e| anyhow!("failed: {}", e))?;
                    found = true;
                    break;
                }
            }
            if !found {
                aur_targets.push(t.clone());
            }
        }
    }
    // disable check_space for speed - we catch errors on write anyway
    manager.handle.set_check_space(false);

    manager
        .handle
        .trans_prepare()
        .map_err(|e| anyhow!("failed: {}", e))?;
    pb.finish_and_clear();

    // Convert to Vec for parallel processing
    let pkgs_add: Vec<_> = manager.handle.trans_add().iter().collect();
    let pkgs_remove: Vec<_> = manager.handle.trans_remove().iter().collect();

    if pkgs_add.is_empty() && pkgs_remove.is_empty() && aur_targets.is_empty() {
        println!("nothing to do.");
        let _ = manager.handle.trans_release();
        return Ok(());
    }

    if !pkgs_remove.is_empty() {
        println!("{}", style("\nREMOVAL").red().bold());
        let mut t = Table::new();
        t.load_preset(UTF8_FULL);
        t.set_header(vec!["package", "version", "size"]);
        for p in &pkgs_remove {
            t.add_row(vec![
                p.name(),
                p.version().as_str(),
                &format!("{:.2} MiB", p.isize() as f64 / 1024.0 / 1024.0),
            ]);
        }
        println!("{}", t);
    }

    if !pkgs_add.is_empty() {
        println!("{}", style("\nINSTALLATION").green().bold());
        let mut t = Table::new();
        t.load_preset(UTF8_FULL);
        t.set_header(vec![
            "repo",
            "package",
            "version",
            "license",
            "dl weight",
            "inst weight",
        ]);

        for p in &pkgs_add {
            let repo = p.db().map(|db| db.name()).unwrap_or("unknown");
            let licenses = p.licenses().into_iter().collect::<Vec<_>>().join(", ");
            t.add_row(vec![
                repo,
                p.name(),
                p.version().as_str(),
                &licenses,
                &format!("{:.1} MB", p.download_size() as f64 / 1024.0 / 1024.0),
                &format!("{:.1} MB", p.isize() as f64 / 1024.0 / 1024.0),
            ]);
        }
        println!("{}", t);

        let count = pkgs_add.len();
        let total_inst: i64 = pkgs_add.iter().map(|p| p.isize()).sum();
        let total_dl: i64 = pkgs_add.iter().map(|p| p.download_size()).sum();

        println!(
            " Installing {} packages | Download: {:.1} MB | Installed: {:.1} MB",
            style(count).bold(),
            total_dl as f64 / 1024.0 / 1024.0,
            total_inst as f64 / 1024.0 / 1024.0
        );
    }

    if !cli.noconfirm {
        use std::io::{self, Write};
        print!("\n{} proceed? [Y/n] ", style("::").bold().cyan());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() && !input.trim().to_lowercase().starts_with('y') {
            let _ = manager.handle.trans_release();
            return Ok(());
        }
    }

    if !pkgs_add.is_empty() {
        let cache = Path::new("/var/cache/pacman/pkg/");
        let mut tasks: Vec<downloader::TurboTask> = Vec::with_capacity(pkgs_add.len() * 2);

        for p in &pkgs_add {
            let f = p.filename().unwrap_or("unknown");
            let size = p.download_size();

            // Skip if cached
            if let Ok(m) = fs::metadata(cache.join(f)) {
                if m.len() == size as u64 {
                    continue;
                }
            }

            if let Some(db) = p.db() {
                let servers: Vec<String> = db
                    .servers()
                    .iter()
                    .map(|s| format!("{}/{}", s, f))
                    .collect();
                if !servers.is_empty() {
                    tasks.push(
                        downloader::TurboTask::new(servers, f.to_string()).with_size(size as u64),
                    );
                }
            }

            // Sig
            let sf = format!("{}.sig", f);
            if !cache.join(&sf).exists() {
                if let Some(db) = p.db() {
                    let sigs: Vec<String> = db
                        .servers()
                        .iter()
                        .map(|s| format!("{}/{}", s, sf))
                        .collect();
                    if !sigs.is_empty() {
                        tasks.push(downloader::TurboTask::new(sigs, sf));
                    }
                }
            }
        }

        if !tasks.is_empty() {
            let mp = MultiProgress::new();
            let engine = downloader::TurboEngine::new(downloader::TurboConfig::fast_network())?;
            engine.download_all(tasks, cache, Some(mp)).await?;
        }
    }

    if !pkgs_add.is_empty() || !pkgs_remove.is_empty() {
        if cli.downloadonly {
            println!("{}", style(":: packages downloaded.").bold().green());
            let _ = manager.handle.trans_release();
        } else {
            // Set up event callbacks for the commit phase with proper counting
            use std::sync::atomic::{AtomicUsize, Ordering};
            let total_pkgs = pkgs_add.len() + pkgs_remove.len();
            let pkg_idx = Box::leak(Box::new(AtomicUsize::new(0)));
            let hook_idx = Box::leak(Box::new(AtomicUsize::new(0)));

            manager.handle.set_event_cb(
                (pkg_idx as &AtomicUsize, hook_idx as &AtomicUsize),
                move |event, (p_idx, h_idx)| match event.event() {
                    alpm::Event::PackageOperationStart(e) => {
                        let i = p_idx.fetch_add(1, Ordering::SeqCst) + 1;
                        let name = match e.operation() {
                            alpm::PackageOperation::Install(p) => p.name(),
                            alpm::PackageOperation::Upgrade(p, _) => p.name(),
                            alpm::PackageOperation::Reinstall(p, _) => p.name(),
                            alpm::PackageOperation::Downgrade(p, _) => p.name(),
                            alpm::PackageOperation::Remove(p) => p.name(),
                        };
                        match e.operation() {
                            alpm::PackageOperation::Install(_) => println!(
                                "({}/{}) installing {}...",
                                i,
                                total_pkgs,
                                style(name).bold()
                            ),
                            alpm::PackageOperation::Upgrade(_, _) => println!(
                                "({}/{}) upgrading {}...",
                                i,
                                total_pkgs,
                                style(name).bold()
                            ),
                            alpm::PackageOperation::Reinstall(_, _) => println!(
                                "({}/{}) reinstalling {}...",
                                i,
                                total_pkgs,
                                style(name).bold()
                            ),
                            alpm::PackageOperation::Downgrade(_, _) => println!(
                                "({}/{}) downgrading {}...",
                                i,
                                total_pkgs,
                                style(name).bold()
                            ),
                            alpm::PackageOperation::Remove(_) => println!(
                                "({}/{}) removing {}...",
                                i,
                                total_pkgs,
                                style(name).bold()
                            ),
                        }
                    }
                    alpm::Event::ScriptletInfo(e) => print!("{}", e.line()),
                    alpm::Event::HookRunStart(e) => {
                        let i = h_idx.fetch_add(1, Ordering::SeqCst) + 1;
                        println!(
                            ":: ({}/?) running hook: {}...",
                            i,
                            style(e.desc().unwrap_or(e.name())).dim()
                        );
                    }
                    _ => {}
                },
            );

            // Progress callback for "extracting..." feel
            manager
                .handle
                .set_progress_cb((), |_op, _, percent, _, _, _| {
                    if percent == 100 || percent % 25 == 0 {
                        // Just a simple way to show activity
                    }
                });

            println!("{}", style(":: committing transaction...").bold());
            manager
                .handle
                .trans_commit()
                .map_err(|e| anyhow!("failed: {}", e))?;
        }
    }

    // Properly release ALPM handle - lock is automatically released on drop
    // DO NOT manually delete lock file - this is unsafe and can corrupt the database!
    drop(manager);

    if !aur_targets.is_empty() {
        install_aur_packages(aur_targets).await?;
    }

    Ok(())
}

fn clean_orphans(manager: &mut alpm_manager::AlpmManager) -> Result<()> {
    println!("{}", style(":: checking for orphans...").bold());
    let localdb = manager.handle.localdb();
    let mut orphans = Vec::new();

    for pkg in localdb.pkgs() {
        if pkg.reason() == alpm::PackageReason::Depend
            && pkg.required_by().is_empty()
            && pkg.optional_for().is_empty()
        {
            orphans.push(pkg);
        }
    }

    if orphans.is_empty() {
        println!("{}", style(":: no orphans found.").green());
        return Ok(());
    }

    let mut t = Table::new();
    t.load_preset(UTF8_FULL);
    t.set_header(vec!["Orphan Package", "Size"]);

    let mut total_size = 0;
    for pkg in &orphans {
        t.add_row(vec![
            pkg.name(),
            &format!("{:.2} MiB", pkg.isize() as f64 / 1024.0 / 1024.0),
        ]);
        total_size += pkg.isize();
    }
    println!("{}", t);
    println!("Total Size: {:.2} MiB", total_size as f64 / 1024.0 / 1024.0);

    use std::io::{self, Write};
    print!(
        "\n{} remove these packages? [y/N] ",
        style("::").bold().cyan()
    );
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if !input.trim().to_lowercase().starts_with('y') {
        return Ok(());
    }

    // We need to re-initialize transaction for removal
    manager
        .handle
        .trans_init(TransFlag::RECURSE)
        .map_err(|e| anyhow!("failed to init trans: {}", e))?;
    for pkg in orphans {
        manager
            .handle
            .trans_remove_pkg(pkg)
            .map_err(|e| anyhow!("failed to remove {}: {}", pkg.name(), e))?;
    }
    manager
        .handle
        .trans_prepare()
        .map_err(|e| anyhow!("failed to prepare: {}", e))?;
    manager
        .handle
        .trans_commit()
        .map_err(|e| anyhow!("failed to commit: {}", e))?;

    println!("{}", style(":: orphans removed.").green().bold());
    Ok(())
}

async fn show_package_info(manager: &alpm_manager::AlpmManager, targets: &[String]) -> Result<()> {
    let db = manager.handle.localdb();
    let sync_dbs = manager.handle.syncdbs();

    let mut aur_targets = Vec::new();
    let client = aur::AurClient::new();

    for target in targets {
        if let Ok(p) = db.pkg(target.as_str()) {
            display_alpm_pkg(p);
        } else {
            let mut found = false;
            for sdb in sync_dbs {
                if let Ok(p) = sdb.pkg(target.as_str()) {
                    display_alpm_pkg(p);
                    found = true;
                    break;
                }
            }
            if !found {
                aur_targets.push(target.clone());
            }
        }
    }

    if !aur_targets.is_empty() {
        if let Ok(results) = client.get_info_batch(&aur_targets).await {
            for p in &results {
                display_aur_pkg(p);
            }
            // Report missing
            let found_names: HashSet<String> = results.iter().map(|r| r.name.clone()).collect();
            for t in aur_targets {
                if !found_names.contains(&t) {
                    eprintln!("error: package '{}' not found in any repository", t);
                }
            }
        }
    }
    Ok(())
}

fn display_alpm_pkg(p: &alpm::Package) {
    println!("{}", style(format!("Package: {}", p.name())).bold().cyan());
    println!("  Version      : {}", p.version());
    println!("  Description  : {}", p.desc().unwrap_or("-"));
    println!("  Architecture : {}", p.arch().unwrap_or("-"));
    println!("  URL          : {}", p.url().unwrap_or("-"));
    println!(
        "  Licenses     : {:?}",
        p.licenses().iter().collect::<Vec<_>>()
    );
    println!(
        "  Groups       : {:?}",
        p.groups().iter().collect::<Vec<_>>()
    );
    println!(
        "  Provides     : {:?}",
        p.provides()
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
    println!(
        "  Depends On   : {:?}",
        p.depends()
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
    println!(
        "  Optional Deps: {:?}",
        p.optdepends()
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
    println!(
        "  Required By  : {:?}",
        p.required_by().iter().collect::<Vec<_>>()
    );
    println!(
        "  Installed Size: {:.2} MiB",
        p.isize() as f64 / 1024.0 / 1024.0
    );
    println!("  Packager     : {}", p.packager().unwrap_or("None"));
    println!("  Build Date   : {}", p.build_date());
    println!();
}

fn display_aur_pkg(p: &aur::AurPackageInfo) {
    println!(
        "{}",
        style(format!("Package: {} (AUR)", p.name)).bold().green()
    );
    println!("  Version      : {}", p.version);
    println!(
        "  Description  : {}",
        p.description.as_deref().unwrap_or("-")
    );
    println!("  URL          : {}", p.url.as_deref().unwrap_or("-"));
    println!("  Votes        : {}", p.num_votes);
    println!("  Popularity   : {:.2}", p.popularity);
    println!(
        "  Maintainer   : {}",
        p.maintainer.as_deref().unwrap_or("None")
    );
    println!("  Licenses     : {:?}", p.license);
    println!("  Depends On   : {:?}", p.depends);
    println!("  Make Deps    : {:?}", p.make_depends);
    println!("  Conflicts    : {:?}", p.conflicts);
    println!("  Provides     : {:?}", p.provides);
    println!();
}
async fn fetch_arch_news() -> Result<()> {
    println!("{}", style(":: fetching arch linux news...").bold());
    let client = reqwest::Client::new();
    let res = client
        .get("https://archlinux.org/feeds/news/")
        .send()
        .await?
        .text()
        .await?;
    let channel = rss::Channel::read_from(res.as_bytes())
        .map_err(|e| anyhow!("failed to parse rss: {}", e))?;

    let mut t = Table::new();
    t.load_preset(UTF8_FULL);
    t.set_header(vec!["Date", "Title"]);
    for item in channel.items().iter().take(5) {
        t.add_row(vec![
            item.pub_date().unwrap_or(""),
            item.title().unwrap_or(""),
        ]);
    }
    println!("{}", t);
    Ok(())
}

fn show_package_history() -> Result<()> {
    println!(
        "{}",
        style(":: package history (last 20 entries)...").bold()
    );
    let log_path = "/var/log/pacman.log";
    if !Path::new(log_path).exists() {
        return Err(anyhow!("log file not found"));
    }
    let content = fs::read_to_string(log_path)?;
    let lines: Vec<_> = content
        .lines()
        .rev()
        .filter(|l| l.contains("installed") || l.contains("removed") || l.contains("upgraded"))
        .take(20)
        .collect();

    let mut t = Table::new();
    t.load_preset(UTF8_FULL);
    t.set_header(vec!["Log Entry"]);
    for line in lines {
        t.add_row(vec![line]);
    }
    println!("{}", t);
    Ok(())
}

fn run_health_check() -> Result<()> {
    println!("{}", style(":: running system health check...").bold());

    // Check for failed services
    let output = std::process::Command::new("systemctl")
        .args(["--failed", "--quiet"])
        .output()?;
    if !output.status.success() {
        println!("{}", style("! some systemd services have failed").red());
    } else {
        println!(
            "{}",
            style("✓ all systemd services are running fine").green()
        );
    }

    // Check disk space
    let output = std::process::Command::new("df")
        .args(["-h", "/"])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = stdout.lines().nth(1) {
        println!(":: disk usage (/): {}", line);
    }

    // Check for broken symlinks in /usr/bin
    let mut broken = 0;
    if let Ok(entries) = fs::read_dir("/usr/bin") {
        for entry in entries.flatten() {
            if let Ok(md) = fs::symlink_metadata(entry.path()) {
                if md.file_type().is_symlink() && !entry.path().exists() {
                    broken += 1;
                }
            }
        }
    }
    if broken > 0 {
        println!(
            "{}",
            style(format!("! found {} broken symlinks in /usr/bin", broken)).yellow()
        );
    } else {
        println!(
            "{}",
            style("✓ no broken symlinks found in /usr/bin").green()
        );
    }

    Ok(())
}

fn clean_cache() -> Result<()> {
    println!("{}", style(":: cleaning package cache...").bold());
    let cache_dir = "/var/cache/pacman/pkg/";
    let mut count = 0;
    let mut size = 0;
    if let Ok(entries) = fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(md) = fs::metadata(&path) {
                    size += md.len();
                    let _ = fs::remove_file(path);
                    count += 1;
                }
            }
        }
    }
    println!(
        ":: removed {} files ({:.2} MiB)",
        count,
        size as f64 / 1024.0 / 1024.0
    );
    Ok(())
}

async fn run_system_report() -> Result<()> {
    println!(
        "{}",
        style(":: generating technical system report...")
            .bold()
            .cyan()
    );
    println!("{}", style("─".repeat(60)).dim());

    // 1. Networking Context
    println!("{}", style("NETWORKING CONTEXT").bold());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let targets = vec![
        ("Arch Linux Core", "https://archlinux.org"),
        ("AUR RPC API", "https://aur.archlinux.org/rpc"),
        ("Cloudflare DNS", "https://1.1.1.1"),
    ];

    for (name, url) in targets {
        let start = std::time::Instant::now();
        let res = client.get(url).send().await;
        let elapsed = start.elapsed().as_millis();
        match res {
            Ok(_) => println!("  {:<20} : {}ms", name, style(elapsed).green()),
            Err(_) => println!("  {:<20} : {}", name, style("FAILED").red()),
        }
    }

    // 2. Pacman configuration detection
    println!("\n{}", style("PACMAN CONFIGURATION").bold());
    if let Ok(content) = fs::read_to_string("/etc/pacman.conf") {
        let parallel = content
            .lines()
            .find(|l| l.trim().starts_with("ParallelDownloads"))
            .unwrap_or("  ParallelDownloads : Not set (default 1)");
        println!("  {}", parallel.trim());
    }

    // 3. Engine Architecture
    println!("\n{}", style("ENGINE ARCHITECTURE").bold());
    println!("  Runtime           : Tokio Async Loop (Multi-threaded)");
    println!("  HTTP Client       : Reqwest / Rustls (Memory Safe)");
    println!("  Transfer Mode     : Segmented Parallel Racing (Enabled)");
    println!("  ALPM Integration  : Native libalpm (via r-alpm)");

    // 4. System Info
    println!("\n{}", style("SYSTEM RESOURCES").bold());
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();
    println!("  CPU Cores         : {}", sys.cpus().len());
    println!(
        "  Total Memory      : {:.2} GB",
        sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0
    );
    println!(
        "  OS                : {}",
        System::name().unwrap_or_else(|| "Unknown".to_string())
    );

    println!("{}", style("─".repeat(60)).dim());
    println!(
        "{}",
        style("This report provides context for benchmark comparisons.").dim()
    );
    Ok(())
}

/// Fetch AUR package snapshot (async, parallelizable)
/// Returns (package_name, build_directory)
async fn fetch_aur(pkg_name: &str) -> Result<(String, String)> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://aur.archlinux.org/rpc/?v=5&type=info&arg[]={}",
        pkg_name
    );
    let res: serde_json::Value = client.get(url).send().await?.json().await?;

    let results = res
        .get("results")
        .and_then(|r| r.as_array())
        .filter(|arr| !arr.is_empty())
        .ok_or_else(|| anyhow!("package {} not found in AUR", pkg_name))?;

    let pkg = &results[0];
    let package_base = pkg["PackageBase"]
        .as_str()
        .ok_or_else(|| anyhow!("invalid AUR response"))?;

    let snapshot_url = format!(
        "https://aur.archlinux.org/cgit/aur.git/snapshot/{}.tar.gz",
        package_base
    );
    let tarball_path = format!("/tmp/{}.tar.gz", package_base);

    let response = client.get(snapshot_url).send().await?;
    let mut file = fs::File::create(&tarball_path)?;
    let mut content = std::io::Cursor::new(response.bytes().await?);
    std::io::copy(&mut content, &mut file)?;

    let build_parent = "/tmp/pacboost-aur";
    let build_dir = format!("{}/{}", build_parent, package_base);
    let _ = fs::remove_dir_all(&build_dir);
    fs::create_dir_all(build_parent)?;

    // Extract
    let status = std::process::Command::new("tar")
        .args(["-xzf", &tarball_path, "-C", build_parent])
        .status()?;
    if !status.success() {
        return Err(anyhow!("failed to extract tarball for {}", pkg_name));
    }

    Ok((pkg_name.to_string(), build_dir))
}

/// Build AUR package with maximum performance optimizations
fn build_aur(pkg_name: &str, build_dir: &str) -> Result<()> {
    println!("{}", style(format!(":: building {}...", pkg_name)).bold());

    // Get number of CPU cores for parallel compilation
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let makeflags = format!("-j{}", num_cpus);

    // Check if running as root
    let uid_output = std::process::Command::new("id").arg("-u").output()?;
    let uid = String::from_utf8_lossy(&uid_output.stdout)
        .trim()
        .to_string();

    let status = if uid == "0" {
        if let Ok(sudo_user) = std::env::var("SUDO_USER") {
            println!(
                "{}",
                style(format!(
                    ":: dropping privileges to {} for makepkg...",
                    sudo_user
                ))
                .yellow()
            );
            let build_parent = "/tmp/pacboost-aur";
            let _ = std::process::Command::new("chown")
                .args(["-R", &format!("{}:{}", sudo_user, sudo_user), build_parent])
                .status();

            std::process::Command::new("sudo")
                .args(["-u", &sudo_user, "makepkg", "-si", "--noconfirm"])
                .env("MAKEFLAGS", &makeflags)
                .env("PKGEXT", ".pkg.tar") // Disable compression for speed
                .current_dir(build_dir)
                .status()?
        } else {
            println!(
                "{}",
                style("! warning: running as root. makepkg cannot run as root.").yellow()
            );
            println!(
                "{}",
                style("  run pacboost as a normal user or with sudo.").yellow()
            );
            return Err(anyhow!(
                "cannot build AUR package as root without SUDO_USER"
            ));
        }
    } else {
        std::process::Command::new("makepkg")
            .args(["-si", "--noconfirm"])
            .env("MAKEFLAGS", &makeflags)
            .env("PKGEXT", ".pkg.tar") // Disable compression for speed
            .current_dir(build_dir)
            .status()?
    };

    if !status.success() {
        return Err(anyhow!(
            "makepkg failed for {} with exit code {:?}",
            pkg_name,
            status.code()
        ));
    }

    println!(
        "{}",
        style(format!(":: {} installed.", pkg_name)).green().bold()
    );
    Ok(())
}

/// Install multiple AUR packages with dependency resolution
async fn install_aur_packages(targets: Vec<String>) -> Result<()> {
    if targets.is_empty() {
        return Ok(());
    }

    // Load configuration
    let cfg = config::Config::load();

    // Create a temporary ALPM handle to check official packages
    let check_official = |name: &str| -> bool {
        // Check if package exists in official repos or is installed locally
        if let Ok(handle) = alpm::Alpm::new("/", "/var/lib/pacman") {
            // Register sync databases
            let dbs = ["core", "extra", "multilib"];
            for db_name in dbs {
                let _ = handle.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT);
            }

            // Check sync dbs
            for db in handle.syncdbs() {
                if db.pkg(name).is_ok() {
                    return true;
                }
            }

            // Also check if it's already installed locally
            if handle.localdb().pkg(name).is_ok() {
                return true;
            }
        }
        false
    };

    // Use the new AUR subsystem with dependency resolution
    aur::builder::install_aur_packages_with_deps(targets, &cfg, check_official).await
}

async fn handle_aur_search(targets: Vec<String>) -> Result<()> {
    if targets.is_empty() {
        return Err(anyhow!("no targets specified for AUR search"));
    }

    println!();
    println!(
        "{} Searching AUR for: {}",
        style("::").cyan().bold(),
        style(targets.join(", ")).white().bold()
    );
    println!();

    let client = aur::AurClient::new();
    for t in targets {
        match client.search(&t).await {
            Ok(results) => {
                if results.is_empty() {
                    println!("   No results found for '{}'", style(&t).yellow());
                    continue;
                }

                println!(
                    "{} {} result(s) for '{}':",
                    style("::").cyan().bold(),
                    style(results.len()).white().bold(),
                    style(&t).yellow()
                );
                println!();

                for pkg in results {
                    let maintainer = pkg.maintainer.as_deref().unwrap_or("orphan");
                    let ood_marker = if pkg.out_of_date.is_some() {
                        style(" [out-of-date]").red().to_string()
                    } else {
                        String::new()
                    };
                    let orphan_marker = if maintainer == "orphan" {
                        style(" [orphan]").red().to_string()
                    } else {
                        String::new()
                    };

                    println!(
                        "{}/{} {}{}{}",
                        style("aur").magenta().bold(),
                        style(&pkg.name).white().bold(),
                        style(&pkg.version).green(),
                        ood_marker,
                        orphan_marker
                    );

                    if let Some(desc) = &pkg.description {
                        println!("    {}", style(desc).dim());
                    }

                    println!(
                        "    Votes: {}  Popularity: {:.2}  Maintainer: {}",
                        style(pkg.num_votes).cyan(),
                        pkg.popularity,
                        style(maintainer).white()
                    );

                    if let Some(url) = &pkg.url {
                        println!("    URL: {}", style(url).blue().underlined());
                    }
                    println!();
                }
            }
            Err(e) => eprintln!("{} search failed: {}", style("error:").red().bold(), e),
        }
    }
    Ok(())
}
