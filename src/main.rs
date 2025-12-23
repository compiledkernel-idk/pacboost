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

const VERSION: &str = "1.1.0";
#[derive(Parser)]
#[command(name = "pacboost")]
#[command(author = "PacBoost Team")]
#[command(version = VERSION)]
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
    if !cli.sync && !cli.sys_upgrade && !cli.remove && !cli.search && !cli.aur && !cli.history && !cli.clean && !cli.news && !cli.health && cli.targets.is_empty() {
        use clap::CommandFactory;
        Cli::command().print_help()?;
        return Ok(());
    }
    if let Some(info) = updater::check_for_updates(VERSION) {
        println!("{}", style(format!( ":: a new version is available: {} (current: {})", info.version, VERSION)).cyan().bold());
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
    use std::io::{self, Write};
    print!("\n{} proceed? [Y/n] ", style("::").bold().cyan()); io::stdout().flush()?;
    let mut input = String::new(); io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() && !input.trim().to_lowercase().starts_with('y') { let _ = manager.handle.trans_release(); return Ok(()); }
    if !pkgs_add.is_empty() {
        let mut to_dl = Vec::new();
        let cache = Path::new("/var/cache/pacman/pkg/");
        for p in &pkgs_add {
            let rs = if let Some(db) = p.db() { db.pkg(p.name()).map(|x| x.download_size()).unwrap_or(p.download_size()) } else { p.download_size() };
            let f = p.filename().unwrap_or("unknown");
            let mut need = true;
            if let Ok(m) = fs::metadata(cache.join(f)) { if m.len() == rs as u64 { need = false; } }
            if need { if let Some(db) = p.db() { to_dl.push((format!("https://geo.mirror.pkgbuild.com/{}/os/x86_64/{}", db.name(), f), f.to_string())); } }
            let sf = format!("{}.sig", f);
            if !cache.join(&sf).exists() { if let Some(db) = p.db() { to_dl.push((format!("https://geo.mirror.pkgbuild.com/{}/os/x86_64/{}", db.name(), sf), sf)); } }
        }
        if !to_dl.is_empty() {
            println!("{}", style(":: fetching packages...").bold());
            let mp = MultiProgress::new();
            downloader::download_packages(to_dl, cache, Some(mp), cli.jobs).await?;
        }
    }
    if !pkgs_add.is_empty() || !pkgs_remove.is_empty() {
        println!("{}", style(":: committing transaction...").bold());
        manager.handle.trans_commit().map_err(|e| anyhow!("failed: {}", e))?;
    }
    
    // Release the lock before AUR installation
    drop(manager);
    let _ = fs::remove_file("/var/lib/pacman/db.lck");
    
    if !aur_targets.is_empty() {
        for t in aur_targets {
            if let Err(e) = handle_aur_install(&t).await {
                eprintln!("{} failed to install {}: {}", style("error:").red().bold(), t, e);
            }
        }
    }
    
    println!("{}", style(":: operation finished.").green().bold());
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

async fn handle_aur_install(pkg_name: &str) -> Result<()> {
    println!("{}", style(format!(":: checking AUR for {}...", pkg_name)).bold());
    let client = reqwest::Client::new();
    let url = format!("https://aur.archlinux.org/rpc/?v=5&type=info&arg[]={}", pkg_name);
    let res: serde_json::Value = client.get(url).send().await?.json().await?;
    
    let results = res.get("results").and_then(|r| r.as_array());
    if results.is_none() || results.unwrap().is_empty() {
        return Err(anyhow!("package {} not found in sync DBs or AUR", pkg_name));
    }
    
    let pkg = &results.unwrap()[0];
    let package_base = pkg["PackageBase"].as_str().ok_or_else(|| anyhow!("invalid AUR response"))?;
    
    println!("{}", style(format!(":: downloading AUR snapshot for {}...", pkg_name)).bold());
    let snapshot_url = format!("https://aur.archlinux.org/cgit/aur.git/snapshot/{}.tar.gz", package_base);
    let tarball_path = format!("/tmp/{}.tar.gz", package_base);
    
    let response = client.get(snapshot_url).send().await?;
    let mut file = fs::File::create(&tarball_path)?;
    let mut content = std::io::Cursor::new(response.bytes().await?);
    std::io::copy(&mut content, &mut file)?;
    
    println!("{}", style(":: extracting and building...").bold());
    let build_parent = "/tmp/pacboost-aur";
    let build_dir = format!("{}/{}", build_parent, package_base);
    let _ = fs::remove_dir_all(&build_dir);
    fs::create_dir_all(build_parent)?;
    
    // Extract
    let status = std::process::Command::new("tar")
        .args(["-xzf", &tarball_path, "-C", build_parent])
        .status()?;
    if !status.success() { return Err(anyhow!("failed to extract tarball")); }
    
    // Check if running as root
    let uid_output = std::process::Command::new("id").arg("-u").output()?;
    let uid = String::from_utf8_lossy(&uid_output.stdout).trim().to_string();
    
    let status = if uid == "0" {
        if let Ok(sudo_user) = std::env::var("SUDO_USER") {
            println!("{}", style(format!(":: dropping privileges to {} for makepkg...", sudo_user)).yellow());
            // Ensure the build directory is accessible by the sudo user
            let _ = std::process::Command::new("chown").args(["-R", &format!("{}:{}", sudo_user, sudo_user), build_parent]).status();
            
            std::process::Command::new("sudo")
                .args(["-u", &sudo_user, "makepkg", "-si", "--noconfirm"])
                .current_dir(&build_dir)
                .status()?
        } else {
            println!("{}", style("! warning: running as root. makepkg cannot run as root.").yellow());
            println!("{}", style("  please run pacboost as a normal user or with sudo.").yellow());
            return Err(anyhow!("cannot build AUR package as root without SUDO_USER"));
        }
    } else {
        std::process::Command::new("makepkg")
            .args(["-si", "--noconfirm"])
            .current_dir(&build_dir)
            .status()?
    };
        
    if !status.success() {
        return Err(anyhow!("makepkg failed with exit code {:?}", status.code()));
    }
    
    Ok(())
}

async fn handle_aur_search(targets: Vec<String>) -> Result<()> {
    if targets.is_empty() { return Err(anyhow!("no targets specified for AUR search")); }
    println!("{}", style(":: searching AUR...").bold());
    let client = reqwest::Client::new();
    for t in targets {
        let url = format!("https://aur.archlinux.org/rpc/?v=5&type=search&arg={}", t);
        let res: serde_json::Value = client.get(url).send().await?.json().await?;
        if let Some(results) = res.get("results").and_then(|r| r.as_array()) {
            for pkg in results {
                println!("{}/{} {}
    {}", style("aur").magenta().bold(), style(pkg["Name"].as_str().unwrap_or("")).bold(), style(pkg["Version"].as_str().unwrap_or("")).green(), pkg["Description"].as_str().unwrap_or(""));
            }
        }
    }
    Ok(())
}
