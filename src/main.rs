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

use anyhow::{Result, anyhow};
use clap::{Parser};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use console::{style};
use comfy_table::Table;
use comfy_table::presets::UTF8_FULL;
use std::time::Duration;
use std::path::Path;
use std::fs;
use alpm::TransFlag;

mod alpm_manager;
mod downloader;
mod updater;
mod reflector;
mod error;
mod config;
mod logging;
mod aur;

const VERSION: &str = "1.6.0";
const LONG_VERSION: &str = concat!(
    "1.6.0\n",
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
    #[arg(short = 'y', long)]
    refresh: bool,
    #[arg(short = 'u', long)]
    sys_upgrade: bool,
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
    #[arg(value_name = "TARGETS")]
    targets: Vec<String>,
}
fn handle_lock_file() -> Result<()> {
    let lock_path = Path::new("/var/lib/pacman/db.lck");
    if !lock_path.exists() { return Ok(()); }
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
                println!("{}", style(format!(":: removing stale lock (pid {})...", pid)).yellow());
                let _ = fs::remove_file(lock_path);
                return Ok(());
            } else {
                return Err(anyhow!("database locked by running process {}", pid));
            }
        },
        Err(_) => {
            println!("{}", style(":: removing corrupt lock file...").yellow());
            let _ = fs::remove_file(lock_path);
            return Ok(());
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
                        println!("{}", style(format!(":: cleaning corrupt entry: {}", name)).yellow());
                        let _ = fs::remove_dir_all(path);
                    }
                }
            }
        }
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.sync && !cli.sys_upgrade && !cli.remove && !cli.search && !cli.aur && !cli.history && !cli.clean && !cli.news && !cli.health && !cli.rank_mirrors && !cli.clean_orphans && !cli.info && !cli.benchmark && cli.targets.is_empty() {
        use clap::CommandFactory;
        Cli::command().print_help()?;
        return Ok(());
    }
    if let Some(info) = updater::check_for_updates(VERSION) {
        println!("{}", style(format!( ":: a new version of pacboost is available: {} (current: {})", info.version, VERSION)).cyan().bold());
        use std::io::{self, Write};
        print!("   update now? [Y/n] "); io::stdout().flush()?;
        let mut input = String::new(); io::stdin().read_line(&mut input)?;
        if input.trim().is_empty() || input.trim().to_lowercase().starts_with('y') {
            if let Err(e) = updater::perform_update(info) {
                eprintln!("{} update failed: {}", style("error:").red().bold(), e);
            } else { println!("   please restart pacboost."); return Ok(()); }
        }
    }
    let _ = handle_lock_file();
    let _ = handle_corrupt_db();
    let spinner_style = ProgressStyle::default_spinner().tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏").template("{spinner:.cyan} {msg}")?;
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.set_message("initializing...");
    pb.enable_steady_tick(Duration::from_millis(80));
    let mut manager = alpm_manager::AlpmManager::new()?;
    pb.finish_and_clear();
    if cli.news {
        return fetch_arch_news().await;
    }
    if cli.history {
        return show_package_history();
    }
    if cli.health {
        return run_health_check();
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
        if cli.targets.is_empty() { return Err(anyhow!("no package specified for info")); }
        return show_package_info(&manager, &cli.targets);
    }
    if cli.benchmark {
        // Get all configured mirrors from alpm_manager
        let mirrors = manager.get_all_mirrors();
        return downloader::run_benchmark(mirrors, 512).await.map(|_| ());
    }
    if cli.aur {
        return handle_aur_search(cli.targets).await;
    }
    if cli.sync && cli.search {
        let results = manager.search(cli.targets)?;
        if results.is_empty() { println!("no matches found."); } else {
            for pkg in results {
                let repo = pkg.db().map(|d| d.name()).unwrap_or("local");
                println!("{}/{} {} {}
    {}", style(repo).cyan().bold(), style(pkg.name()).bold(), style(pkg.version().as_str()).green(), style(format!("[installed: {}]", pkg.isize())).black(), pkg.desc().unwrap_or(""));
            }
        }
        return Ok(());
    }
    if cli.refresh {
         println!("{}", style(":: syncing databases...").bold());
         let mp = MultiProgress::new();
         manager.sync_dbs_manual(Some(mp), cli.jobs).await?;
    }
    let mut aur_targets = Vec::new();
    if cli.targets.is_empty() && !cli.sys_upgrade { return Ok(()); }
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.set_message("resolving...");
    pb.enable_steady_tick(Duration::from_millis(80));
    let mut flags = TransFlag::empty();
    if cli.remove && cli.recursive { flags |= TransFlag::RECURSE; }
    manager.handle.trans_init(flags).map_err(|e| anyhow!("failed to init trans: {}", e))?;
    if cli.remove {
        let local_db = manager.handle.localdb();
        for t in &cli.targets {
            if let Ok(p) = local_db.pkg(t.as_str()) { manager.handle.trans_remove_pkg(p).map_err(|e| anyhow!("failed: {}", e))?; }
            else { return Err(anyhow!("target not found: {}", t)); }
        }
    } else {
        if cli.sys_upgrade { manager.handle.sync_sysupgrade(false).map_err(|e| anyhow!("failed: {}", e))?; }
        for t in &cli.targets {
            // Easter egg: detect if user is trying to install pacboost with pacboost
            if t == "pacboost" || t == "pacboost-bin" {
                println!("{}", style("").bold());
                println!("{}", style("╔═══════════════════════════════════════════════════════════════╗").cyan().bold());
                println!("{}", style("║   Why is dawg trying to install pacboost.                    ║").cyan().bold());
                println!("{}", style("║   I'm not sure why you would do that.                       ║").cyan().bold());
                println!("{}", style("║                                                               ║").cyan().bold());
                println!("{}", style("║   Maybe you should try something else man.                      ║").cyan().bold());
                println!("{}", style("╚═══════════════════════════════════════════════════════════════╝").cyan().bold());
                println!("{}", style("").bold());
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }
            
            let mut found = false;
            for db in manager.handle.syncdbs() {
                if let Ok(p) = db.pkg(t.as_str()) { manager.handle.trans_add_pkg(p).map_err(|e| anyhow!("failed: {}", e))?; found = true; break; }
            }
            if !found { aur_targets.push(t.clone()); }
        }
    }
    manager.handle.trans_prepare().map_err(|e| anyhow!("failed: {}", e))?;
    pb.finish_and_clear();
    let pkgs_add: Vec<_> = manager.handle.trans_add().iter().collect();
    let pkgs_remove: Vec<_> = manager.handle.trans_remove().iter().collect();
    if pkgs_add.is_empty() && pkgs_remove.is_empty() && aur_targets.is_empty() { println!("nothing to do."); let _ = manager.handle.trans_release(); return Ok(()); }
    if !pkgs_remove.is_empty() {
        println!("{}", style("\nREMOVAL").red().bold());
        let mut t = Table::new(); t.load_preset(UTF8_FULL); t.set_header(vec!["package", "version", "size"]);
        for p in &pkgs_remove { t.add_row(vec![p.name(), p.version().as_str(), &format!("{:.2} MiB", p.isize() as f64 / 1024.0 / 1024.0)]); }
        println!("{}", t);
    }
    if !pkgs_add.is_empty() {
        println!("{}", style("\nINSTALLATION").green().bold());
        let mut t = Table::new(); t.load_preset(UTF8_FULL); t.set_header(vec!["package", "version", "download", "installed", "repo"]);
        let (mut td, mut ti) = (0, 0);
        for p in &pkgs_add {
            let rs = if let Some(db) = p.db() { db.pkg(p.name()).map(|x| x.download_size()).unwrap_or(p.download_size()) } else { p.download_size() };
            td += rs; ti += p.isize();
            let ds = if rs == 0 { "Cached".to_string() } else { format!("{:.2} MiB", rs as f64 / 1024.0 / 1024.0) };
            t.add_row(vec![p.name(), p.version().as_str(), &ds, &format!("{:.2} MiB", p.isize() as f64 / 1024.0 / 1024.0), p.db().map(|d| d.name()).unwrap_or("-")]);
        }
        println!("{}", t);
        println!("\nTotal Download:  {:.2} MiB", td as f64 / 1024.0 / 1024.0);
        println!("Total Installed: {:.2} MiB", ti as f64 / 1024.0 / 1024.0);
    }
    if !cli.noconfirm {
        use std::io::{self, Write};
        print!("\n{} proceed? [Y/n] ", style("::").bold().cyan()); io::stdout().flush()?;
        let mut input = String::new(); io::stdin().read_line(&mut input)?;
        if !input.trim().is_empty() && !input.trim().to_lowercase().starts_with('y') { let _ = manager.handle.trans_release(); return Ok(()); }
    }
    if !pkgs_add.is_empty() {
        let mut to_dl: Vec<(Vec<String>, String)> = Vec::new();
        let cache = Path::new("/var/cache/pacman/pkg/");
        for p in &pkgs_add {
            let rs = if let Some(db) = p.db() { db.pkg(p.name()).map(|x| x.download_size()).unwrap_or(p.download_size()) } else { p.download_size() };
            let f = p.filename().unwrap_or("unknown");
            let mut need = true;
            if let Ok(m) = fs::metadata(cache.join(f)) { if m.len() == rs as u64 { need = false; } }
            if need {
                if let Some(db) = p.db() {
                    // Collect ALL mirrors for this package
                    let mirrors: Vec<String> = manager.get_repo_mirrors(db.name())
                        .iter()
                        .map(|server| format!("{}/{}", server, f))
                        .collect();
                    let mirrors = if mirrors.is_empty() {
                        vec![format!("https://geo.mirror.pkgbuild.com/{}/os/x86_64/{}", db.name(), f)]
                    } else { mirrors };
                    to_dl.push((mirrors, f.to_string()));
                }
            }
            let sf = format!("{}.sig", f);
            if !cache.join(&sf).exists() {
                if let Some(db) = p.db() {
                    let sig_mirrors: Vec<String> = manager.get_repo_mirrors(db.name())
                        .iter()
                        .map(|server| format!("{}/{}", server, sf))
                        .collect();
                    let sig_mirrors = if sig_mirrors.is_empty() {
                        vec![format!("https://geo.mirror.pkgbuild.com/{}/os/x86_64/{}", db.name(), sf)]
                    } else { sig_mirrors };
                    to_dl.push((sig_mirrors, sf));
                }
            }
        }
        if !to_dl.is_empty() {
            println!("{}", style(":: fetching packages...").bold());
            let mp = MultiProgress::new();
            
            let tasks: Vec<_> = to_dl.into_iter().map(|(mirrors, filename)| {
                downloader::DownloadTask::new(mirrors, filename)
            }).collect();
            
            let config = downloader::DownloadConfig {
                max_connections: cli.jobs * 4, // Allow more connections per job
                ..Default::default()
            };
            
            let engine = downloader::DownloadEngine::new(config)?;
            engine.download_all(tasks, cache, Some(mp)).await?;
        }
    }
    if !pkgs_add.is_empty() || !pkgs_remove.is_empty() {
        println!("{}", style(":: committing transaction...").bold());
        manager.handle.trans_commit().map_err(|e| anyhow!("failed: {}", e))?;
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
        if pkg.reason() == alpm::PackageReason::Depend && pkg.required_by().is_empty() && pkg.optional_for().is_empty() {
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
        t.add_row(vec![pkg.name(), &format!("{:.2} MiB", pkg.isize() as f64 / 1024.0 / 1024.0)]);
        total_size += pkg.isize();
    }
    println!("{}", t);
    println!("Total Size: {:.2} MiB", total_size as f64 / 1024.0 / 1024.0);
    
    use std::io::{self, Write};
    print!("\n{} remove these packages? [y/N] ", style("::").bold().cyan()); io::stdout().flush()?;
    let mut input = String::new(); io::stdin().read_line(&mut input)?;
    if !input.trim().to_lowercase().starts_with('y') { return Ok(()); }
    
    // We need to re-initialize transaction for removal
    manager.handle.trans_init(TransFlag::RECURSE).map_err(|e| anyhow!("failed to init trans: {}", e))?;
    for pkg in orphans {
        manager.handle.trans_remove_pkg(pkg).map_err(|e| anyhow!("failed to remove {}: {}", pkg.name(), e))?;
    }
    manager.handle.trans_prepare().map_err(|e| anyhow!("failed to prepare: {}", e))?;
    manager.handle.trans_commit().map_err(|e| anyhow!("failed to commit: {}", e))?;
    
    println!("{}", style(":: orphans removed.").green().bold());
    Ok(())
}

fn show_package_info(manager: &alpm_manager::AlpmManager, targets: &[String]) -> Result<()> {
    let db = manager.handle.localdb();
    let sync_dbs = manager.handle.syncdbs();
    
    for target in targets {
        let pkg = if let Ok(p) = db.pkg(target.as_str()) {
            Some(p)
        } else {
            // Check sync dbs
            let mut found = None;
            for sdb in sync_dbs {
                if let Ok(p) = sdb.pkg(target.as_str()) {
                    found = Some(p);
                    break;
                }
            }
            found
        };
        
        if let Some(p) = pkg {
            println!("{}", style(format!("Package: {}", p.name())).bold().cyan());
            println!("  Version      : {}", p.version());
            println!("  Description  : {}", p.desc().unwrap_or("-"));
            println!("  Architecture : {}", p.arch().unwrap_or("-"));
            println!("  URL          : {}", p.url().unwrap_or("-"));
            println!("  Licenses     : {:?}", p.licenses().iter().collect::<Vec<_>>());
            println!("  Groups       : {:?}", p.groups().iter().collect::<Vec<_>>());
            println!("  Provides     : {:?}", p.provides().iter().map(|d| d.to_string()).collect::<Vec<_>>());
            println!("  Depends On   : {:?}", p.depends().iter().map(|d| d.to_string()).collect::<Vec<_>>());
            println!("  Optional Deps: {:?}", p.optdepends().iter().map(|d| d.to_string()).collect::<Vec<_>>());
            println!("  Required By  : {:?}", p.required_by().iter().collect::<Vec<_>>());
            println!("  Installed Size: {:.2} MiB", p.isize() as f64 / 1024.0 / 1024.0);
            println!("  Packager     : {}", p.packager().unwrap_or("None"));
            println!("  Build Date   : {}", p.build_date());
            println!("");
        } else {
            eprintln!("error: package '{}' not found", target);
        }
    }
    Ok(())
}
async fn fetch_arch_news() -> Result<()> {
    println!("{}", style(":: fetching arch linux news...").bold());
    let client = reqwest::Client::new();
    let res = client.get("https://archlinux.org/feeds/news/").send().await?.text().await?;
    let channel = rss::Channel::read_from(res.as_bytes()).map_err(|e| anyhow!("failed to parse rss: {}", e))?;
    
    let mut t = Table::new();
    t.load_preset(UTF8_FULL);
    t.set_header(vec!["Date", "Title"]);
    for item in channel.items().iter().take(5) {
        t.add_row(vec![item.pub_date().unwrap_or(""), item.title().unwrap_or("")]);
    }
    println!("{}", t);
    Ok(())
}

fn show_package_history() -> Result<()> {
    println!("{}", style(":: package history (last 20 entries)...").bold());
    let log_path = "/var/log/pacman.log";
    if !Path::new(log_path).exists() { return Err(anyhow!("log file not found")); }
    let content = fs::read_to_string(log_path)?;
    let lines: Vec<_> = content.lines().rev().filter(|l| l.contains("installed") || l.contains("removed") || l.contains("upgraded")).take(20).collect();
    
    let mut t = Table::new();
    t.load_preset(UTF8_FULL);
    t.set_header(vec!["Log Entry"]);
    for line in lines { t.add_row(vec![line]); }
    println!("{}", t);
    Ok(())
}

fn run_health_check() -> Result<()> {
    println!("{}", style(":: running system health check...").bold());
    
    // Check for failed services
    let output = std::process::Command::new("systemctl").args(["--failed", "--quiet"]).output()?;
    if !output.status.success() { println!("{}", style("! some systemd services have failed").red()); }
    else { println!("{}", style("✓ all systemd services are running fine").green()); }
    
    // Check disk space
    let output = std::process::Command::new("df").args(["-h", "/"]).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = stdout.lines().nth(1) { println!(":: disk usage (/): {}", line); }
    
    // Check for broken symlinks in /usr/bin
    let mut broken = 0;
    if let Ok(entries) = fs::read_dir("/usr/bin") {
        for entry in entries.flatten() {
            if let Ok(md) = fs::symlink_metadata(entry.path()) {
                if md.file_type().is_symlink() && !entry.path().exists() { broken += 1; }
            }
        }
    }
    if broken > 0 { println!("{}", style(format!("! found {} broken symlinks in /usr/bin", broken)).yellow()); }
    else { println!("{}", style("✓ no broken symlinks found in /usr/bin").green()); }
    
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
    println!(":: removed {} files ({:.2} MiB)", count, size as f64 / 1024.0 / 1024.0);
    Ok(())
}

/// Fetch AUR package snapshot (async, parallelizable)
/// Returns (package_name, build_directory)
async fn fetch_aur(pkg_name: &str) -> Result<(String, String)> {
    let client = reqwest::Client::new();
    let url = format!("https://aur.archlinux.org/rpc/?v=5&type=info&arg[]={}", pkg_name);
    let res: serde_json::Value = client.get(url).send().await?.json().await?;
    
    let results = res.get("results")
        .and_then(|r| r.as_array())
        .filter(|arr| !arr.is_empty())
        .ok_or_else(|| anyhow!("package {} not found in AUR", pkg_name))?;
    
    let pkg = &results[0];
    let package_base = pkg["PackageBase"].as_str().ok_or_else(|| anyhow!("invalid AUR response"))?;
    
    let snapshot_url = format!("https://aur.archlinux.org/cgit/aur.git/snapshot/{}.tar.gz", package_base);
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
    if !status.success() { return Err(anyhow!("failed to extract tarball for {}", pkg_name)); }
    
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
    let uid = String::from_utf8_lossy(&uid_output.stdout).trim().to_string();
    
    let status = if uid == "0" {
        if let Ok(sudo_user) = std::env::var("SUDO_USER") {
            println!("{}", style(format!(":: dropping privileges to {} for makepkg...", sudo_user)).yellow());
            let build_parent = "/tmp/pacboost-aur";
            let _ = std::process::Command::new("chown")
                .args(["-R", &format!("{}:{}", sudo_user, sudo_user), build_parent])
                .status();
            
            std::process::Command::new("sudo")
                .args(["-u", &sudo_user, "makepkg", "-si", "--noconfirm"])
                .env("MAKEFLAGS", &makeflags)
                .env("PKGEXT", ".pkg.tar")  // Disable compression for speed
                .current_dir(build_dir)
                .status()?
        } else {
            println!("{}", style("! warning: running as root. makepkg cannot run as root.").yellow());
            println!("{}", style("  please run pacboost as a normal user or with sudo.").yellow());
            return Err(anyhow!("cannot build AUR package as root without SUDO_USER"));
        }
    } else {
        std::process::Command::new("makepkg")
            .args(["-si", "--noconfirm"])
            .env("MAKEFLAGS", &makeflags)
            .env("PKGEXT", ".pkg.tar")  // Disable compression for speed
            .current_dir(build_dir)
            .status()?
    };
        
    if !status.success() {
        return Err(anyhow!("makepkg failed for {} with exit code {:?}", pkg_name, status.code()));
    }
    
    println!("{}", style(format!(":: {} installed successfully.", pkg_name)).green().bold());
    Ok(())
}

/// Install multiple AUR packages with dependency resolution
async fn install_aur_packages(targets: Vec<String>) -> Result<()> {
    if targets.is_empty() { return Ok(()); }
    
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
    if targets.is_empty() { return Err(anyhow!("no targets specified for AUR search")); }
    
    println!();
    println!("{} Searching AUR for: {}", 
        style("::").cyan().bold(),
        style(targets.join(", ")).white().bold());
    println!();
    
    let client = aur::AurClient::new();
    for t in targets {
        match client.search(&t).await {
            Ok(results) => {
                if results.is_empty() {
                    println!("   No results found for '{}'", style(&t).yellow());
                    continue;
                }
                
                println!("{} {} result(s) for '{}':", 
                    style("::").cyan().bold(),
                    style(results.len()).white().bold(),
                    style(&t).yellow());
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
                    
                    println!("{}/{} {}{}{}",
                        style("aur").magenta().bold(),
                        style(&pkg.name).white().bold(),
                        style(&pkg.version).green(),
                        ood_marker,
                        orphan_marker);
                    
                    if let Some(desc) = &pkg.description {
                        println!("    {}", style(desc).dim());
                    }
                    
                    println!("    Votes: {}  Popularity: {:.2}  Maintainer: {}",
                        style(pkg.num_votes).cyan(),
                        pkg.popularity,
                        style(maintainer).white());
                    
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
