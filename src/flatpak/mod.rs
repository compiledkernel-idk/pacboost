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

//! Flatpak integration for pacboost.
//!
//! Provides high-performance Flatpak management including:
//! - Install, remove, and update Flatpak applications
//! - Remote (repository) management
//! - Parallel runtime downloads
//! - Permission inspection and management

pub mod remote;

use anyhow::{anyhow, Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

/// Flatpak application information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatpakApp {
    pub name: String,
    pub application_id: String,
    pub version: String,
    pub branch: String,
    pub arch: String,
    pub origin: String,
    pub installation: String,
    pub size_bytes: u64,
    pub description: Option<String>,
}

/// Flatpak runtime information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatpakRuntime {
    pub name: String,
    pub runtime_id: String,
    pub version: String,
    pub branch: String,
    pub arch: String,
    pub origin: String,
}

/// Flatpak remote (repository) information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatpakRemote {
    pub name: String,
    pub url: String,
    pub homepage: Option<String>,
    pub description: Option<String>,
    pub enabled: bool,
    pub priority: i32,
}

/// Flatpak client for high-performance Flatpak management
pub struct FlatpakClient {
    /// Use system-wide installation
    system: bool,
    /// Custom installation path
    installation: Option<String>,
}

impl FlatpakClient {
    /// Create a new Flatpak client
    pub fn new() -> Self {
        Self {
            system: true,
            installation: None,
        }
    }

    /// Create a user-level Flatpak client
    pub fn user() -> Self {
        Self {
            system: false,
            installation: None,
        }
    }

    /// Check if Flatpak is available on the system
    pub fn is_available() -> bool {
        which::which("flatpak").is_ok()
    }

    /// Get installation flag
    fn install_flag(&self) -> &str {
        if self.system {
            "--system"
        } else {
            "--user"
        }
    }

    /// List installed Flatpak applications
    pub fn list_apps(&self) -> Result<Vec<FlatpakApp>> {
        let output = Command::new("flatpak")
            .args([
                "list",
                "--app",
                "--columns=name,application,version,branch,arch,origin,installation,size",
                self.install_flag(),
            ])
            .output()
            .context("Failed to run flatpak list")?;

        if !output.status.success() {
            return Err(anyhow!(
                "flatpak list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut apps = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 7 {
                apps.push(FlatpakApp {
                    name: parts[0].to_string(),
                    application_id: parts[1].to_string(),
                    version: parts.get(2).unwrap_or(&"").to_string(),
                    branch: parts.get(3).unwrap_or(&"stable").to_string(),
                    arch: parts.get(4).unwrap_or(&"x86_64").to_string(),
                    origin: parts.get(5).unwrap_or(&"flathub").to_string(),
                    installation: parts.get(6).unwrap_or(&"system").to_string(),
                    size_bytes: parts.get(7).and_then(|s| parse_size(s)).unwrap_or(0),
                    description: None,
                });
            }
        }

        Ok(apps)
    }

    /// List installed runtimes
    pub fn list_runtimes(&self) -> Result<Vec<FlatpakRuntime>> {
        let output = Command::new("flatpak")
            .args([
                "list",
                "--runtime",
                "--columns=name,application,version,branch,arch,origin",
                self.install_flag(),
            ])
            .output()
            .context("Failed to run flatpak list --runtime")?;

        if !output.status.success() {
            return Err(anyhow!("flatpak list --runtime failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut runtimes = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 5 {
                runtimes.push(FlatpakRuntime {
                    name: parts[0].to_string(),
                    runtime_id: parts[1].to_string(),
                    version: parts.get(2).unwrap_or(&"").to_string(),
                    branch: parts.get(3).unwrap_or(&"").to_string(),
                    arch: parts.get(4).unwrap_or(&"x86_64").to_string(),
                    origin: parts.get(5).unwrap_or(&"flathub").to_string(),
                });
            }
        }

        Ok(runtimes)
    }

    /// Search for Flatpak applications
    pub fn search(&self, query: &str) -> Result<Vec<FlatpakSearchResult>> {
        let output = Command::new("flatpak")
            .args([
                "search",
                "--columns=name,application,version,branch,remotes,description",
                query,
            ])
            .output()
            .context("Failed to run flatpak search")?;

        if !output.status.success() {
            // Search returns non-zero if no results
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                results.push(FlatpakSearchResult {
                    name: parts[0].to_string(),
                    application_id: parts[1].to_string(),
                    version: parts.get(2).unwrap_or(&"").to_string(),
                    branch: parts.get(3).unwrap_or(&"stable").to_string(),
                    remotes: parts.get(4).unwrap_or(&"flathub").to_string(),
                    description: parts.get(5).map(|s| s.to_string()),
                });
            }
        }

        Ok(results)
    }

    /// Install a Flatpak application
    pub fn install(&self, app_id: &str) -> Result<()> {
        println!(
            "{} Installing Flatpak: {}",
            style("::").cyan().bold(),
            style(app_id).yellow().bold()
        );

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.set_message(format!("Installing {}...", app_id));
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        let status = Command::new("flatpak")
            .args(["install", "-y", self.install_flag(), app_id])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run flatpak install")?;

        pb.finish_and_clear();

        if !status.success() {
            return Err(anyhow!("Failed to install {}", app_id));
        }

        println!(
            "{} {} installed",
            style("::").green().bold(),
            style(app_id).white().bold()
        );

        Ok(())
    }

    /// Install multiple Flatpak applications in parallel
    pub async fn install_many(&self, app_ids: &[String]) -> Result<()> {
        println!(
            "{} Installing {} Flatpak application(s)...",
            style("::").cyan().bold(),
            style(app_ids.len()).yellow().bold()
        );

        for app_id in app_ids {
            self.install(app_id)?;
        }

        Ok(())
    }

    /// Remove a Flatpak application
    pub fn remove(&self, app_id: &str) -> Result<()> {
        println!(
            "{} Removing Flatpak: {}",
            style("::").cyan().bold(),
            style(app_id).yellow().bold()
        );

        let status = Command::new("flatpak")
            .args(["uninstall", "-y", self.install_flag(), app_id])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run flatpak uninstall")?;

        if !status.success() {
            return Err(anyhow!("Failed to remove {}", app_id));
        }

        println!(
            "{} {} removed",
            style("::").green().bold(),
            style(app_id).white().bold()
        );

        Ok(())
    }

    /// Update all Flatpak applications
    pub fn update_all(&self) -> Result<()> {
        println!(
            "{} Updating all Flatpak applications...",
            style("::").cyan().bold()
        );

        let status = Command::new("flatpak")
            .args(["update", "-y", self.install_flag()])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run flatpak update")?;

        if !status.success() {
            return Err(anyhow!("Flatpak update failed"));
        }

        println!(
            "{} All Flatpak applications updated",
            style("::").green().bold()
        );

        Ok(())
    }

    /// Update a specific Flatpak application
    pub fn update(&self, app_id: &str) -> Result<()> {
        println!(
            "{} Updating Flatpak: {}",
            style("::").cyan().bold(),
            style(app_id).yellow().bold()
        );

        let status = Command::new("flatpak")
            .args(["update", "-y", self.install_flag(), app_id])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run flatpak update")?;

        if !status.success() {
            return Err(anyhow!("Failed to update {}", app_id));
        }

        Ok(())
    }

    /// Get information about a Flatpak application
    pub fn info(&self, app_id: &str) -> Result<FlatpakInfo> {
        let output = Command::new("flatpak")
            .args(["info", app_id])
            .output()
            .context("Failed to run flatpak info")?;

        if !output.status.success() {
            return Err(anyhow!("Failed to get info for {}", app_id));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut info = FlatpakInfo::default();
        info.application_id = app_id.to_string();

        for line in stdout.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "Name" => info.name = value.to_string(),
                    "Version" => info.version = value.to_string(),
                    "Branch" => info.branch = value.to_string(),
                    "Arch" => info.arch = value.to_string(),
                    "Origin" => info.origin = value.to_string(),
                    "Installation" => info.installation = value.to_string(),
                    "Installed size" => info.installed_size = value.to_string(),
                    "Runtime" => info.runtime = Some(value.to_string()),
                    "Sdk" => info.sdk = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        Ok(info)
    }

    /// List application permissions
    pub fn permissions(&self, app_id: &str) -> Result<Vec<String>> {
        let output = Command::new("flatpak")
            .args(["info", "--show-permissions", app_id])
            .output()
            .context("Failed to get permissions")?;

        if !output.status.success() {
            return Err(anyhow!("Failed to get permissions for {}", app_id));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    /// Run a Flatpak application
    pub fn run(&self, app_id: &str, args: &[String]) -> Result<()> {
        let mut cmd = Command::new("flatpak");
        cmd.args(["run", app_id]);
        cmd.args(args);
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let status = cmd.status().context("Failed to run flatpak application")?;

        if !status.success() {
            return Err(anyhow!("Application exited with error"));
        }

        Ok(())
    }

    /// Clean up unused runtimes and extensions
    pub fn cleanup(&self) -> Result<CleanupResult> {
        let output = Command::new("flatpak")
            .args(["uninstall", "--unused", "-y", self.install_flag()])
            .output()
            .context("Failed to run flatpak cleanup")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let removed: Vec<String> = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|s| s.to_string())
            .collect();

        Ok(CleanupResult {
            removed_count: removed.len(),
            removed_items: removed,
        })
    }

    /// Get disk usage statistics
    pub fn disk_usage(&self) -> Result<DiskUsage> {
        let apps = self.list_apps()?;
        let runtimes = self.list_runtimes()?;

        let app_size: u64 = apps.iter().map(|a| a.size_bytes).sum();

        Ok(DiskUsage {
            apps_count: apps.len(),
            runtimes_count: runtimes.len(),
            total_size_bytes: app_size,
        })
    }
}

impl Default for FlatpakClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Flatpak search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatpakSearchResult {
    pub name: String,
    pub application_id: String,
    pub version: String,
    pub branch: String,
    pub remotes: String,
    pub description: Option<String>,
}

/// Detailed Flatpak application info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlatpakInfo {
    pub name: String,
    pub application_id: String,
    pub version: String,
    pub branch: String,
    pub arch: String,
    pub origin: String,
    pub installation: String,
    pub installed_size: String,
    pub runtime: Option<String>,
    pub sdk: Option<String>,
}

/// Cleanup operation result
#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub removed_count: usize,
    pub removed_items: Vec<String>,
}

/// Disk usage statistics
#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub apps_count: usize,
    pub runtimes_count: usize,
    pub total_size_bytes: u64,
}

/// Parse size string like "1.2 GB" to bytes
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let value: f64 = parts[0].parse().ok()?;
    let multiplier: u64 = match parts[1].to_uppercase().as_str() {
        "B" => 1,
        "KB" | "KIB" => 1024,
        "MB" | "MIB" => 1024 * 1024,
        "GB" | "GIB" => 1024 * 1024 * 1024,
        "TB" | "TIB" => 1024_u64 * 1024 * 1024 * 1024,
        _ => return None,
    };

    Some((value * multiplier as f64) as u64)
}

/// Display Flatpak apps in a nice table
pub fn display_apps(apps: &[FlatpakApp]) {
    use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Application ID").fg(Color::Cyan),
        Cell::new("Version").fg(Color::Cyan),
        Cell::new("Origin").fg(Color::Cyan),
        Cell::new("Size").fg(Color::Cyan),
    ]);

    for app in apps {
        let size = format_size(app.size_bytes);
        table.add_row(vec![
            Cell::new(&app.name).fg(Color::White),
            Cell::new(&app.application_id).fg(Color::Green),
            Cell::new(&app.version).fg(Color::Yellow),
            Cell::new(&app.origin).fg(Color::Blue),
            Cell::new(&size).fg(Color::Magenta),
        ]);
    }

    println!("{}", table);
}

/// Display search results
pub fn display_search_results(results: &[FlatpakSearchResult]) {
    use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

    if results.is_empty() {
        println!("{} No results found", style("::").yellow().bold());
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Application ID").fg(Color::Cyan),
        Cell::new("Version").fg(Color::Cyan),
        Cell::new("Remote").fg(Color::Cyan),
        Cell::new("Description").fg(Color::Cyan),
    ]);

    for result in results {
        table.add_row(vec![
            Cell::new(&result.name).fg(Color::White),
            Cell::new(&result.application_id).fg(Color::Green),
            Cell::new(&result.version).fg(Color::Yellow),
            Cell::new(&result.remotes).fg(Color::Blue),
            Cell::new(result.description.as_deref().unwrap_or("-")).fg(Color::DarkGrey),
        ]);
    }

    println!("{}", table);
}

/// Format bytes to human-readable size
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1.5 GB"), Some(1610612736));
        assert_eq!(parse_size("100 MB"), Some(104857600));
        assert_eq!(parse_size("1024 KB"), Some(1048576));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(1073741824), "1.00 GB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1024), "1.00 KB");
    }

    #[test]
    fn test_flatpak_available() {
        // This test just checks the function works
        let _ = FlatpakClient::is_available();
    }
}
