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

//! AUR package building with proper privilege handling.

use anyhow::{Result, anyhow, Context};
use console::style;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::fs;
use std::time::Duration;

use super::client::AurPackageInfo;
use super::pkgbuild::{SecurityValidator, SecurityReport, parse_pkgbuild};
use crate::config::Config;
use crate::error::PacboostError;

/// AUR package builder with resource management
pub struct AurBuilder {
    /// Base build directory
    build_dir: PathBuf,
    /// Number of parallel make jobs
    make_jobs: usize,
    /// Build timeout (0 = unlimited)
    timeout_secs: u64,
    /// Disable compression for faster builds
    disable_compression: bool,
    /// Enable ccache
    use_ccache: bool,
    /// Clean build directory after installation
    clean_after_build: bool,
    /// Security validator
    security_validator: SecurityValidator,
    /// Minimum security score to proceed
    min_security_score: u32,
}

impl AurBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            build_dir: PathBuf::from("/tmp/pacboost-aur"),
            make_jobs: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            timeout_secs: 0,
            disable_compression: true,
            use_ccache: false,
            clean_after_build: true,
            security_validator: SecurityValidator::new(),
            min_security_score: 50,
        }
    }
    
    /// Create a builder from configuration
    pub fn from_config(config: &Config) -> Self {
        Self {
            build_dir: config.aur.build_dir.clone(),
            make_jobs: config.get_make_jobs(),
            timeout_secs: config.aur.build_timeout_secs,
            disable_compression: config.aur.disable_compression,
            use_ccache: config.aur.use_ccache,
            clean_after_build: config.aur.clean_build,
            security_validator: SecurityValidator::new(),
            min_security_score: config.aur.min_security_score,
        }
    }
    
    /// Fetch and extract an AUR package
    pub async fn fetch(&self, info: &AurPackageInfo) -> Result<PathBuf> {
        use indicatif::{ProgressBar, ProgressStyle};
        use futures::StreamExt;
        
        let snapshot_url = info.snapshot_url();
        let tarball_path = self.build_dir.join(format!("{}.tar.gz", info.package_base));
        let extract_dir = self.build_dir.join(&info.package_base);
        
        // Ensure build directory exists
        fs::create_dir_all(&self.build_dir)
            .context("Failed to create build directory")?;
        
        // Remove old build directory if exists
        if extract_dir.exists() {
            println!("   {} cleaning old build directory...", style("->").blue());
            fs::remove_dir_all(&extract_dir)
                .context("Failed to clean old build directory")?;
        }
        
        // Download tarball with progress
        println!("   {} downloading {} snapshot...", style("->").blue(), style(&info.package_base).cyan());
        
        let client = reqwest::Client::new();
        let response = client.get(&snapshot_url)
            .send()
            .await
            .context("Failed to download snapshot")?;
        
        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download {}: HTTP {}",
                info.package_base,
                response.status()
            ));
        }
        
        let total_size = response.content_length().unwrap_or(0);
        
        let pb = if total_size > 0 {
            let pb = ProgressBar::new(total_size);
            pb.set_style(ProgressStyle::default_bar()
                .template("   {spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
                .unwrap()
                .progress_chars("=>-"));
            Some(pb)
        } else {
            None
        };
        
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let mut data = Vec::new();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error downloading chunk")?;
            downloaded += chunk.len() as u64;
            data.extend_from_slice(&chunk);
            if let Some(ref pb) = pb {
                pb.set_position(downloaded);
            }
        }
        
        if let Some(pb) = pb {
            pb.finish_and_clear();
        }
        
        println!("   {} downloaded {:.2} KiB", style("->").green(), data.len() as f64 / 1024.0);
        
        fs::write(&tarball_path, &data)?;
        
        // Extract tarball
        println!("   {} extracting archive...", style("->").blue());
        let status = Command::new("tar")
            .args(["-xzf", tarball_path.to_str().unwrap(), "-C", self.build_dir.to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .context("Failed to run tar")?;
        
        if !status.success() {
            return Err(anyhow!("Failed to extract tarball for {}", info.package_base));
        }
        
        // Remove tarball
        let _ = fs::remove_file(&tarball_path);
        println!("   {} source ready", style("->").green());
        
        Ok(extract_dir)
    }
    
    /// Validate PKGBUILD security
    pub fn validate_pkgbuild(&self, build_dir: &Path) -> Result<SecurityReport> {
        let pkgbuild_path = build_dir.join("PKGBUILD");
        
        if !pkgbuild_path.exists() {
            return Err(anyhow!("PKGBUILD not found in {}", build_dir.display()));
        }
        
        let content = fs::read_to_string(&pkgbuild_path)?;
        let report = self.security_validator.validate(&content);
        
        Ok(report)
    }
    
    /// Import PGP keys from PKGBUILD validpgpkeys array
    pub fn import_pgp_keys(&self, build_dir: &Path) -> Result<()> {
        let pkgbuild_path = build_dir.join("PKGBUILD");
        let content = fs::read_to_string(&pkgbuild_path)?;
        
        // Extract validpgpkeys from PKGBUILD
        let mut keys = Vec::new();
        
        // Look for validpgpkeys=(...) pattern
        if let Some(start) = content.find("validpgpkeys=(") {
            let rest = &content[start + 14..];
            if let Some(end) = rest.find(')') {
                let keys_section = &rest[..end];
                for line in keys_section.lines() {
                    let line = line.trim().trim_matches('\'').trim_matches('"');
                    if !line.is_empty() && !line.starts_with('#') {
                        // Extract just the key ID (last 16 chars if fingerprint)
                        let key = if line.len() > 16 {
                            &line[line.len()-16..]
                        } else {
                            line
                        };
                        keys.push(key.to_string());
                    }
                }
            }
        }
        
        if keys.is_empty() {
            return Ok(());
        }
        
        println!("   {} importing {} PGP key(s)...", style("->").blue(), keys.len());
        
        // Try to import each key
        for key in &keys {
            println!("      importing key {}...", style(key).cyan());
            
            // Try multiple keyservers
            let keyservers = [
                "keyserver.ubuntu.com",
                "keys.openpgp.org",
                "pgp.mit.edu",
            ];
            
            let mut imported = false;
            for server in &keyservers {
                let status = Command::new("gpg")
                    .args(["--keyserver", server, "--recv-keys", key])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                
                if let Ok(s) = status {
                    if s.success() {
                        println!("      {} key {} imported from {}", style("->").green(), key, server);
                        imported = true;
                        break;
                    }
                }
            }
            
            if !imported {
                println!("      {} could not import key {} (will try --skippgpcheck)", style("->").yellow(), key);
            }
        }
        
        Ok(())
    }
    
    /// Build the package in the given directory
    pub fn build(&self, build_dir: &Path, package_name: &str) -> Result<Vec<PathBuf>> {
        use indicatif::{ProgressBar, ProgressStyle};
        
        // Security scan with spinner
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("   {spinner:.cyan} {msg}")
            .unwrap());
        spinner.set_message("scanning PKGBUILD for security issues...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        
        let report = self.validate_pkgbuild(build_dir)?;
        spinner.finish_and_clear();
        
        if report.issues.is_empty() {
            println!("   {} security scan passed (score: {}/100)", style("->").green(), report.score);
        } else {
            println!("   {} security scan complete (score: {}/100)", style("->").yellow(), report.score);
            for issue in &report.issues {
                let prefix = match issue.severity {
                    crate::error::SecuritySeverity::Critical => style("CRITICAL").red().bold(),
                    crate::error::SecuritySeverity::High => style("HIGH").red(),
                    crate::error::SecuritySeverity::Medium => style("MEDIUM").yellow(),
                    crate::error::SecuritySeverity::Low => style("LOW").blue(),
                    crate::error::SecuritySeverity::Info => style("INFO").dim(),
                };
                let line_info = issue.line.map(|l| format!(" (line {})", l)).unwrap_or_default();
                println!("      [{}] {}{}", prefix, issue.description, line_info);
            }
            
            if report.score < self.min_security_score {
                return Err(PacboostError::PkgbuildSecurityIssue {
                    reason: format!("Security score {} is below threshold {}", report.score, self.min_security_score),
                    severity: crate::error::SecuritySeverity::High,
                }.into());
            }
        }
        
        // Get current user info
        let uid = unsafe { libc::getuid() };
        let is_root = uid == 0;
        
        // Import PGP keys before build
        let _ = self.import_pgp_keys(build_dir);
        
        // Build makepkg command
        let makeflags = format!("-j{}", self.make_jobs);
        let pkgext = if self.disable_compression { ".pkg.tar" } else { ".pkg.tar.zst" };
        
        println!("   {} preparing build environment...", style("->").blue());
        println!("      MAKEFLAGS: {}", style(&makeflags).cyan());
        println!("      PKGEXT: {}", style(pkgext).cyan());
        if self.use_ccache {
            println!("      ccache: {}", style("enabled").green());
        }
        
        let status = if is_root {
            // Running as root - need to drop privileges
            if let Ok(sudo_user) = std::env::var("SUDO_USER") {
                println!("   {} dropping privileges to {}...", style("->").yellow(), style(&sudo_user).cyan());
                
                // Change ownership of build directory
                let _ = Command::new("chown")
                    .args(["-R", &format!("{}:{}", sudo_user, sudo_user), build_dir.to_str().unwrap()])
                    .status();
                
                let mut cmd = Command::new("sudo");
                // Use --skippgpcheck to avoid signature verification failures
                cmd.args(["-u", &sudo_user, "makepkg", "-sf", "--noconfirm", "--skippgpcheck"]);
                cmd.env("MAKEFLAGS", &makeflags);
                cmd.env("PKGEXT", pkgext);
                
                if self.use_ccache {
                    cmd.env("PATH", format!("/usr/lib/ccache/bin:{}", std::env::var("PATH").unwrap_or_default()));
                }
                
                cmd.current_dir(build_dir);
                cmd.stdout(Stdio::inherit());
                cmd.stderr(Stdio::inherit());
                
                cmd.status().context("Failed to run makepkg")?
            } else {
                return Err(anyhow!(
                    "Cannot build AUR packages as root without SUDO_USER set. \
                    Please run pacboost with sudo as a normal user."
                ));
            }
        } else {
            // Running as normal user
            let mut cmd = Command::new("makepkg");
            // Use --skippgpcheck to avoid signature verification failures
            cmd.args(["-sf", "--noconfirm", "--skippgpcheck"]);
            cmd.env("MAKEFLAGS", &makeflags);
            cmd.env("PKGEXT", pkgext);
            
            if self.use_ccache {
                cmd.env("PATH", format!("/usr/lib/ccache/bin:{}", std::env::var("PATH").unwrap_or_default()));
            }
            
            cmd.current_dir(build_dir);
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
            
            cmd.status().context("Failed to run makepkg")?
        };
        
        if !status.success() {
            return Err(PacboostError::BuildFailed {
                package: package_name.to_string(),
                reason: format!("makepkg exited with code {:?}", status.code()),
                exit_code: status.code(),
            }.into());
        }
        
        // Find built packages
        let packages = self.find_built_packages(build_dir)?;
        
        if packages.is_empty() {
            return Err(anyhow!("No packages were built"));
        }
        
        Ok(packages)
    }
    
    /// Install built packages
    pub fn install(&self, packages: &[PathBuf]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }
        
        // Remove stale lock file if present (safe because we're about to run pacman)
        let lock_path = std::path::Path::new("/var/lib/pacman/db.lck");
        if lock_path.exists() {
            println!("   {} removing stale lock file...", style("->").yellow());
            let _ = fs::remove_file(lock_path);
        }
        
        println!("{}", style(":: installing packages...").bold());
        
        let package_args: Vec<&str> = packages.iter()
            .filter_map(|p| p.to_str())
            .collect();
        
        let status = Command::new("sudo")
            .arg("pacman")
            .arg("-U")
            .arg("--noconfirm")
            .args(&package_args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run pacman -U")?;
        
        if !status.success() {
            return Err(anyhow!("Failed to install packages"));
        }
        
        Ok(())
    }
    
    /// Build and install a package
    pub async fn build_and_install(&self, info: &AurPackageInfo) -> Result<()> {
        println!();
        println!("{} {} {}", 
            style("::").cyan().bold(),
            style("Processing").white(),
            style(&info.name).yellow().bold());
        
        // Fetch
        println!("   {} fetching source...", style("->").blue());
        let build_dir = self.fetch(info).await?;
        
        // Build  
        println!("   {} running makepkg...", style("->").blue());
        let packages = self.build(&build_dir, &info.name)?;
        
        // Show built packages
        println!("   {} built {} package(s):", style("->").green(), packages.len());
        for pkg in &packages {
            if let Some(name) = pkg.file_name() {
                let size = fs::metadata(pkg).map(|m| m.len()).unwrap_or(0);
                println!("      {} ({:.2} MiB)", 
                    style(name.to_string_lossy()).cyan(),
                    size as f64 / 1024.0 / 1024.0);
            }
        }
        
        // Install
        println!("   {} installing with pacman...", style("->").blue());
        self.install(&packages)?;
        
        // Cleanup
        if self.clean_after_build {
            println!("   {} cleaning build directory...", style("->").dim());
            let _ = fs::remove_dir_all(&build_dir);
        }
        
        println!("   {} {} installed", style("->").green(), style(&info.name).white().bold());
        
        Ok(())
    }
    
    /// Find built package files in build directory
    fn find_built_packages(&self, build_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut packages = Vec::new();
        
        for entry in fs::read_dir(build_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".pkg.tar") || 
                       name.ends_with(".pkg.tar.zst") || 
                       name.ends_with(".pkg.tar.xz") ||
                       name.ends_with(".pkg.tar.gz") {
                        packages.push(path);
                    }
                }
            }
        }
        
        Ok(packages)
    }
    
    /// Clean the build directory
    pub fn clean(&self) -> Result<()> {
        if self.build_dir.exists() {
            fs::remove_dir_all(&self.build_dir)?;
        }
        Ok(())
    }
    
    /// Get build directory path
    pub fn build_dir(&self) -> &Path {
        &self.build_dir
    }
}

impl Default for AurBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Install multiple AUR packages with dependency resolution
pub async fn install_aur_packages_with_deps(
    targets: Vec<String>,
    config: &Config,
    official_check: impl Fn(&str) -> bool + Clone,
) -> Result<()> {
    use super::client::AurClient;
    use super::resolver::DependencyGraph;
    use comfy_table::{Table, presets::UTF8_FULL, Cell, Color, ContentArrangement};
    use std::time::Instant;
    
    if targets.is_empty() {
        return Ok(());
    }
    
    let start_time = Instant::now();
    
    println!();
    println!("{} Resolving dependencies for {} package(s)...", 
        style("::").cyan().bold(), 
        style(targets.len()).white().bold());
    
    // Build dependency graph
    let client = AurClient::new();
    let graph = DependencyGraph::build(targets.clone(), &client, official_check.clone()).await?;
    
    let resolve_time = start_time.elapsed();
    println!("{} Dependency resolution completed in {:.2}s", 
        style("::").cyan().bold(),
        resolve_time.as_secs_f64());
    
    // Check for conflicts
    let conflicts = graph.find_conflicts();
    if !conflicts.is_empty() {
        println!();
        println!("{} {}", style("::").yellow().bold(), style("Conflicts detected:").yellow().bold());
        for (pkg1, pkg2, reason) in &conflicts {
            println!("   {} conflicts with {} ({})", 
                style(pkg1).white().bold(), 
                style(pkg2).white().bold(), 
                style(reason).dim());
        }
    }
    
    // Get sorted AUR packages
    let sorted_aur = graph.aur_packages_sorted()?;
    
    if sorted_aur.is_empty() {
        println!("{} No AUR packages to install", style("::").cyan().bold());
        return Ok(());
    }
    
    // Get official dependencies
    let official_deps = graph.official_deps();
    
    // Summary header
    println!();
    println!("{} {}", style("::").cyan().bold(), style("Package Summary").white().bold());
    println!();
    
    // Official deps
    if !official_deps.is_empty() {
        println!("   {} official dependencies: {}", 
            style(official_deps.len()).yellow().bold(),
            style(official_deps.join(", ")).dim());
        println!();
    }
    
    // AUR packages table with full details
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        Cell::new("Package").fg(Color::Cyan),
        Cell::new("Version").fg(Color::Cyan),
        Cell::new("Votes").fg(Color::Cyan),
        Cell::new("Pop").fg(Color::Cyan),
        Cell::new("Maintainer").fg(Color::Cyan),
        Cell::new("Out of Date").fg(Color::Cyan),
    ]);
    
    let mut total_deps = 0;
    let mut total_makedeps = 0;
    
    for pkg in &sorted_aur {
        let maintainer = pkg.maintainer.as_deref().unwrap_or("orphan");
        let ood = if pkg.out_of_date.is_some() { "Yes" } else { "No" };
        
        total_deps += pkg.depends.len();
        total_makedeps += pkg.make_depends.len();
        
        table.add_row(vec![
            Cell::new(&pkg.name).fg(Color::White),
            Cell::new(&pkg.version).fg(Color::Green),
            Cell::new(format!("{}", pkg.num_votes)).fg(
                if pkg.num_votes > 100 { Color::Green } 
                else if pkg.num_votes > 10 { Color::Yellow } 
                else { Color::Red }
            ),
            Cell::new(format!("{:.1}", pkg.popularity)).fg(
                if pkg.popularity > 1.0 { Color::Green } 
                else { Color::Yellow }
            ),
            Cell::new(maintainer).fg(
                if maintainer == "orphan" { Color::Red } else { Color::White }
            ),
            Cell::new(ood).fg(
                if ood == "Yes" { Color::Red } else { Color::Green }
            ),
        ]);
    }
    
    println!("{}", table);
    
    // Detailed package info
    println!();
    println!("{} {}", style("::").cyan().bold(), style("Package Details").white().bold());
    println!();
    
    for pkg in &sorted_aur {
        println!("   {} {}", style(&pkg.name).cyan().bold(), style(&pkg.version).green());
        if let Some(desc) = &pkg.description {
            println!("      {}", style(desc).dim());
        }
        if let Some(url) = &pkg.url {
            println!("      URL: {}", style(url).blue().underlined());
        }
        if !pkg.depends.is_empty() {
            println!("      Depends: {}", style(pkg.depends.join(", ")).dim());
        }
        if !pkg.make_depends.is_empty() {
            println!("      Make Deps: {}", style(pkg.make_depends.join(", ")).dim());
        }
        if !pkg.license.is_empty() {
            println!("      License: {}", style(pkg.license.join(", ")).dim());
        }
        println!();
    }
    
    // Build summary
    println!("{} {}", style("::").cyan().bold(), style("Build Configuration").white().bold());
    println!("   Packages to build: {}", style(sorted_aur.len()).yellow().bold());
    println!("   Total dependencies: {}", style(total_deps).white());
    println!("   Total make dependencies: {}", style(total_makedeps).white());
    println!("   Parallel jobs: {}", style(config.get_make_jobs()).green());
    println!("   Compression: {}", style(if config.aur.disable_compression { "disabled (fast)" } else { "enabled" }).white());
    
    // Confirmation prompt
    println!();
    print!("{} Proceed with build and installation? [Y/n] ", style("::").cyan().bold());
    use std::io::{self, Write};
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if !input.trim().is_empty() && !input.trim().to_lowercase().starts_with('y') {
        println!("{} Cancelled.", style("::").yellow().bold());
        return Ok(());
    }
    
    // Build and install packages
    let builder = AurBuilder::from_config(config);
    let total = sorted_aur.len();
    
    println!();
    println!("{} {}", style("::").green().bold(), style("Starting build process...").white().bold());
    
    for (i, pkg) in sorted_aur.iter().enumerate() {
        println!();
        println!("{} [{}/{}] {} {}", 
            style("::").cyan().bold(),
            style(i + 1).white().bold(),
            style(total).dim(),
            style(&pkg.name).yellow().bold(),
            style(&pkg.version).green());
        
        let build_start = Instant::now();
        
        if let Err(e) = builder.build_and_install(pkg).await {
            println!();
            println!("{} Build failed for {}: {}", 
                style("error:").red().bold(), 
                style(&pkg.name).white().bold(),
                e);
            return Err(e);
        }
        
        let build_time = build_start.elapsed();
        println!("{} {} completed in {:.1}s", 
            style("::").green().bold(),
            style(&pkg.name).white().bold(),
            build_time.as_secs_f64());
    }
    
    // Final summary
    let total_time = start_time.elapsed();
    println!();
    println!("{} {}", style("::").green().bold(), style("Installation complete").white().bold());
    println!("   Packages installed: {}", style(total).green().bold());
    println!("   Total time: {:.1}s", total_time.as_secs_f64());
    println!();
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_builder_new() {
        let builder = AurBuilder::new();
        assert_eq!(builder.build_dir, PathBuf::from("/tmp/pacboost-aur"));
        assert!(builder.make_jobs >= 1);
        assert!(builder.disable_compression);
    }
    
    #[test]
    fn test_find_built_packages_patterns() {
        // Just testing the file extension detection logic
        let extensions = vec![".pkg.tar", ".pkg.tar.zst", ".pkg.tar.xz", ".pkg.tar.gz"];
        for ext in extensions {
            let name = format!("test-1.0.0-1-x86_64{}", ext);
            assert!(
                name.ends_with(".pkg.tar") || 
                name.ends_with(".pkg.tar.zst") || 
                name.ends_with(".pkg.tar.xz") ||
                name.ends_with(".pkg.tar.gz")
            );
        }
    }
}
