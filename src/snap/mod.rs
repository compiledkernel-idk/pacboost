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

//! Snap package manager integration for pacboost.
//!
//! Provides high-performance Snap management including:
//! - Install, remove, and refresh snaps
//! - Snap Store search
//! - Channel management (stable/beta/edge)
//! - Confinement information

pub mod store;

use anyhow::{anyhow, Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

/// Snap package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snap {
    pub name: String,
    pub version: String,
    pub rev: String,
    pub tracking: String,
    pub publisher: String,
    pub notes: String,
    pub confinement: Confinement,
}

/// Snap confinement level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confinement {
    Strict,
    Classic,
    Devmode,
}

impl std::fmt::Display for Confinement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confinement::Strict => write!(f, "strict"),
            Confinement::Classic => write!(f, "classic"),
            Confinement::Devmode => write!(f, "devmode"),
        }
    }
}

impl From<&str> for Confinement {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "classic" => Confinement::Classic,
            "devmode" => Confinement::Devmode,
            _ => Confinement::Strict,
        }
    }
}

/// Snap client for high-performance Snap management
pub struct SnapClient {
    /// Allow classic confinement without prompt
    allow_classic: bool,
    /// Preferred channel
    channel: String,
}

impl SnapClient {
    /// Create a new Snap client
    pub fn new() -> Self {
        Self {
            allow_classic: false,
            channel: "stable".to_string(),
        }
    }

    /// Allow classic confinement
    pub fn with_classic(mut self) -> Self {
        self.allow_classic = true;
        self
    }

    /// Set preferred channel
    pub fn with_channel(mut self, channel: &str) -> Self {
        self.channel = channel.to_string();
        self
    }

    /// Check if Snap is available on the system
    pub fn is_available() -> bool {
        which::which("snap").is_ok()
    }

    /// Check if snapd service is running
    pub fn is_running() -> bool {
        Command::new("systemctl")
            .args(["is-active", "--quiet", "snapd"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// List installed snaps
    pub fn list(&self) -> Result<Vec<Snap>> {
        let output = Command::new("snap")
            .args(["list"])
            .output()
            .context("Failed to run snap list")?;

        if !output.status.success() {
            return Err(anyhow!(
                "snap list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut snaps = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                let notes = parts[5..].join(" ");
                let confinement = if notes.contains("classic") {
                    Confinement::Classic
                } else if notes.contains("devmode") {
                    Confinement::Devmode
                } else {
                    Confinement::Strict
                };

                snaps.push(Snap {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    rev: parts[2].to_string(),
                    tracking: parts[3].to_string(),
                    publisher: parts[4].to_string(),
                    notes,
                    confinement,
                });
            }
        }

        Ok(snaps)
    }

    /// Search for snaps
    pub fn search(&self, query: &str) -> Result<Vec<SnapSearchResult>> {
        let output = Command::new("snap")
            .args(["find", query])
            .output()
            .context("Failed to run snap find")?;

        if !output.status.success() {
            // snap find returns non-zero if no results
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                results.push(SnapSearchResult {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    publisher: parts[2].to_string(),
                    summary: parts[4..].join(" "),
                });
            }
        }

        Ok(results)
    }

    /// Install a snap
    pub fn install(&self, name: &str) -> Result<()> {
        println!(
            "{} Installing Snap: {}",
            style("::").cyan().bold(),
            style(name).yellow().bold()
        );

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.set_message(format!("Installing {}...", name));
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        let mut args = vec!["install"];

        if self.channel != "stable" {
            args.push("--channel");
            args.push(&self.channel);
        }

        if self.allow_classic {
            args.push("--classic");
        }

        args.push(name);

        let status = Command::new("sudo")
            .arg("snap")
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run snap install")?;

        pb.finish_and_clear();

        if !status.success() {
            // Try with --classic if it failed (might need it)
            if !self.allow_classic {
                println!(
                    "{} Retrying with --classic flag...",
                    style("::").yellow().bold()
                );

                let status = Command::new("sudo")
                    .args(["snap", "install", "--classic", name])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()?;

                if !status.success() {
                    return Err(anyhow!("Failed to install {}", name));
                }
            } else {
                return Err(anyhow!("Failed to install {}", name));
            }
        }

        println!(
            "{} {} installed",
            style("::").green().bold(),
            style(name).white().bold()
        );

        Ok(())
    }

    /// Remove a snap
    pub fn remove(&self, name: &str) -> Result<()> {
        println!(
            "{} Removing Snap: {}",
            style("::").cyan().bold(),
            style(name).yellow().bold()
        );

        let status = Command::new("sudo")
            .args(["snap", "remove", name])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run snap remove")?;

        if !status.success() {
            return Err(anyhow!("Failed to remove {}", name));
        }

        println!(
            "{} {} removed",
            style("::").green().bold(),
            style(name).white().bold()
        );

        Ok(())
    }

    /// Refresh (update) a specific snap
    pub fn refresh(&self, name: &str) -> Result<()> {
        println!(
            "{} Refreshing Snap: {}",
            style("::").cyan().bold(),
            style(name).yellow().bold()
        );

        let status = Command::new("sudo")
            .args(["snap", "refresh", name])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run snap refresh")?;

        if !status.success() {
            return Err(anyhow!("Failed to refresh {}", name));
        }

        Ok(())
    }

    /// Refresh all snaps
    pub fn refresh_all(&self) -> Result<()> {
        println!("{} Refreshing all snaps...", style("::").cyan().bold());

        let status = Command::new("sudo")
            .args(["snap", "refresh"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to run snap refresh")?;

        if !status.success() {
            return Err(anyhow!("Snap refresh failed"));
        }

        println!("{} All snaps refreshed", style("::").green().bold());

        Ok(())
    }

    /// Get info about a snap
    pub fn info(&self, name: &str) -> Result<SnapInfo> {
        let output = Command::new("snap")
            .args(["info", name])
            .output()
            .context("Failed to run snap info")?;

        if !output.status.success() {
            return Err(anyhow!("Failed to get info for {}", name));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut info = SnapInfo::default();
        info.name = name.to_string();

        for line in stdout.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "name" => info.name = value.to_string(),
                    "summary" => info.summary = value.to_string(),
                    "publisher" => info.publisher = value.to_string(),
                    "store-url" => info.store_url = Some(value.to_string()),
                    "license" => info.license = Some(value.to_string()),
                    "description" => info.description = Some(value.to_string()),
                    "snap-id" => info.snap_id = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        Ok(info)
    }

    /// Switch snap channel
    pub fn switch_channel(&self, name: &str, channel: &str) -> Result<()> {
        println!(
            "{} Switching {} to channel {}",
            style("::").cyan().bold(),
            style(name).yellow().bold(),
            style(channel).green().bold()
        );

        let status = Command::new("sudo")
            .args(["snap", "refresh", "--channel", channel, name])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to switch channel")?;

        if !status.success() {
            return Err(anyhow!("Failed to switch {} to channel {}", name, channel));
        }

        Ok(())
    }

    /// Revert a snap to previous revision
    pub fn revert(&self, name: &str) -> Result<()> {
        println!(
            "{} Reverting {} to previous revision",
            style("::").cyan().bold(),
            style(name).yellow().bold()
        );

        let status = Command::new("sudo")
            .args(["snap", "revert", name])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to revert snap")?;

        if !status.success() {
            return Err(anyhow!("Failed to revert {}", name));
        }

        Ok(())
    }

    /// List snap connections (interfaces)
    pub fn connections(&self, name: &str) -> Result<Vec<String>> {
        let output = Command::new("snap")
            .args(["connections", name])
            .output()
            .context("Failed to get connections")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().skip(1).map(|s| s.to_string()).collect())
    }

    /// Enable a snap
    pub fn enable(&self, name: &str) -> Result<()> {
        Command::new("sudo")
            .args(["snap", "enable", name])
            .status()
            .context("Failed to enable snap")?;
        Ok(())
    }

    /// Disable a snap
    pub fn disable(&self, name: &str) -> Result<()> {
        Command::new("sudo")
            .args(["snap", "disable", name])
            .status()
            .context("Failed to disable snap")?;
        Ok(())
    }

    /// Get disk usage
    pub fn disk_usage(&self) -> Result<u64> {
        // Get snap directory size
        let output = Command::new("du").args(["-sb", "/snap"]).output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let size: u64 = stdout
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                Ok(size)
            }
            _ => Ok(0),
        }
    }
}

impl Default for SnapClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Snap search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapSearchResult {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub summary: String,
}

/// Detailed snap info
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapInfo {
    pub name: String,
    pub summary: String,
    pub publisher: String,
    pub store_url: Option<String>,
    pub license: Option<String>,
    pub description: Option<String>,
    pub snap_id: Option<String>,
}

/// Display installed snaps in a table
pub fn display_snaps(snaps: &[Snap]) {
    use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Version").fg(Color::Cyan),
        Cell::new("Channel").fg(Color::Cyan),
        Cell::new("Publisher").fg(Color::Cyan),
        Cell::new("Confinement").fg(Color::Cyan),
    ]);

    for snap in snaps {
        let conf_color = match snap.confinement {
            Confinement::Strict => Color::Green,
            Confinement::Classic => Color::Yellow,
            Confinement::Devmode => Color::Red,
        };

        table.add_row(vec![
            Cell::new(&snap.name).fg(Color::White),
            Cell::new(&snap.version).fg(Color::Green),
            Cell::new(&snap.tracking).fg(Color::Blue),
            Cell::new(&snap.publisher).fg(Color::Magenta),
            Cell::new(snap.confinement.to_string()).fg(conf_color),
        ]);
    }

    println!("{}", table);
}

/// Display search results
pub fn display_search_results(results: &[SnapSearchResult]) {
    use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

    if results.is_empty() {
        println!("{} No results found", style("::").yellow().bold());
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Version").fg(Color::Cyan),
        Cell::new("Publisher").fg(Color::Cyan),
        Cell::new("Summary").fg(Color::Cyan),
    ]);

    for result in results {
        table.add_row(vec![
            Cell::new(&result.name).fg(Color::White),
            Cell::new(&result.version).fg(Color::Green),
            Cell::new(&result.publisher).fg(Color::Magenta),
            Cell::new(&result.summary).fg(Color::DarkGrey),
        ]);
    }

    println!("{}", table);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confinement_from_str() {
        assert_eq!(Confinement::from("classic"), Confinement::Classic);
        assert_eq!(Confinement::from("devmode"), Confinement::Devmode);
        assert_eq!(Confinement::from("strict"), Confinement::Strict);
        assert_eq!(Confinement::from("unknown"), Confinement::Strict);
    }

    #[test]
    fn test_snap_available() {
        let _ = SnapClient::is_available();
    }

    #[test]
    fn test_snap_client_builder() {
        let client = SnapClient::new().with_classic().with_channel("beta");
        assert!(client.allow_classic);
        assert_eq!(client.channel, "beta");
    }
}
