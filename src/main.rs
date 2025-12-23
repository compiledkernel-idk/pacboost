/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
...
#[derive(Parser)]
#[command(name = "pacboost")]
#[command(author = "PacBoost Team")]
#[command(version = "1.0.0")]
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.sync && !cli.sys_upgrade && !cli.remove && !cli.search && cli.targets.is_empty() {
        use clap::CommandFactory;
        Cli::command().print_help()?;
        return Ok(());
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
         let rt = tokio::runtime::Runtime::new()?;
         rt.block_on(manager.sync_dbs_manual(Some(mp), cli.jobs))?;
    }
    if cli.targets.is_empty() && !cli.sys_upgrade { return Ok(()); }
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style.clone());
    pb.set_message("resolving...");
    pb.enable_steady_tick(Duration::from_millis(80));
    let mut flags = TransFlag::empty();
    if cli.remove && cli.recursive { flags |= TransFlag::RECURSE; }
    manager.handle.trans_init(flags).map_err(|e| anyhow!("transaction failed: {}", e))?;
    if cli.remove {
        let local_db = manager.handle.localdb();
        for t in &cli.targets {
            if let Ok(p) = local_db.pkg(t.as_str()) { 
                manager.handle.trans_remove_pkg(p).map_err(|e| anyhow!("failed to remove: {}", e))?; 
            } else { return Err(anyhow!("target not found: {}", t)); }
        }
    } else {
        if cli.sys_upgrade { manager.handle.sync_sysupgrade(false).map_err(|e| anyhow!("upgrade failed: {}", e))?; }
        for t in &cli.targets {
            let mut found = false;
            for db in manager.handle.syncdbs() {
                if let Ok(p) = db.pkg(t.as_str()) { 
                    manager.handle.trans_add_pkg(p).map_err(|e| anyhow!("failed to add: {}", e))?; 
                    found = true; 
                    break; 
                }
            }
            if !found { return Err(anyhow!("target not found: {}", t)); }
        }
    }
    manager.handle.trans_prepare().map_err(|e| anyhow!("resolution failed: {}", e))?;
    pb.finish_and_clear();
    let pkgs_add: Vec<_> = manager.handle.trans_add().iter().collect();
    let pkgs_remove: Vec<_> = manager.handle.trans_remove().iter().collect();
    if pkgs_add.is_empty() && pkgs_remove.is_empty() { println!("nothing to do."); let _ = manager.handle.trans_release(); return Ok(()); }
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
            td += rs;
            ti += p.isize();
            let ds = if rs == 0 { "Cached".to_string() } else { format!("{:.2} MiB", rs as f64 / 1024.0 / 1024.0) };
            t.add_row(vec![p.name(), p.version().as_str(), &ds, &format!("{:.2} MiB", p.isize() as f64 / 1024.0 / 1024.0), p.db().map(|d| d.name()).unwrap_or("-")]);
        }
        println!("{}", t);
        println!("\nTotal Download:  {:.2} MiB", td as f64 / 1024.0 / 1024.0);
        println!("Total Installed: {:.2} MiB", ti as f64 / 1024.0 / 1024.0);
    }
    use std::io::{self, Write};
    print!("\n{} proceed with transaction? [Y/n] ", style("::").bold().cyan());
    io::stdout().flush()?;
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
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(downloader::download_packages(to_dl, cache, Some(mp), cli.jobs))?;
        }
    }
    println!("{}", style(":: committing transaction...").bold());
    manager.handle.trans_commit().map_err(|e| anyhow!("failed: {}", e))?;
    println!("{}", style(":: operation finished.").green().bold());
    Ok(())
}
