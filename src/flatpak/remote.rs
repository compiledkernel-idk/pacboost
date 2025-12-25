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

//! Flatpak remote (repository) management.

use anyhow::{Result, Context, anyhow};
use console::style;
use std::process::{Command, Stdio};
use serde::{Deserialize, Serialize};

/// Flatpak remote information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    pub title: Option<String>,
    pub url: String,
    pub collection_id: Option<String>,
    pub priority: i32,
    pub options: RemoteOptions,
}

/// Remote configuration options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteOptions {
    pub gpg_verify: bool,
    pub gpg_verify_summary: bool,
    pub enabled: bool,
    pub prio: i32,
}

/// Remote manager for Flatpak repositories
pub struct RemoteManager {
    system: bool,
}

impl RemoteManager {
    /// Create a new remote manager for system-wide remotes
    pub fn new() -> Self {
        Self { system: true }
    }

    /// Create a remote manager for user remotes
    pub fn user() -> Self {
        Self { system: false }
    }

    fn install_flag(&self) -> &str {
        if self.system { "--system" } else { "--user" }
    }

    /// List configured remotes
    pub fn list(&self) -> Result<Vec<Remote>> {
        let output = Command::new("flatpak")
            .args(["remotes", "--columns=name,title,url,collection,priority,options", self.install_flag()])
            .output()
            .context("Failed to run flatpak remotes")?;

        if !output.status.success() {
            return Err(anyhow!("flatpak remotes failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut remotes = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let options_str = parts.get(5).unwrap_or(&"");
                remotes.push(Remote {
                    name: parts[0].to_string(),
                    title: if parts[1].is_empty() { None } else { Some(parts[1].to_string()) },
                    url: parts[2].to_string(),
                    collection_id: parts.get(3).filter(|s| !s.is_empty()).map(|s| s.to_string()),
                    priority: parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(1),
                    options: parse_options(options_str),
                });
            }
        }

        Ok(remotes)
    }

    /// Add a new remote
    pub fn add(&self, name: &str, url: &str) -> Result<()> {
        println!("{} Adding remote: {} ({})",
            style("::").cyan().bold(),
            style(name).yellow().bold(),
            style(url).dim());

        let status = Command::new("flatpak")
            .args(["remote-add", "--if-not-exists", self.install_flag(), name, url])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to add remote")?;

        if !status.success() {
            return Err(anyhow!("Failed to add remote {}", name));
        }

        println!("{} Remote {} added", style("::").green().bold(), style(name).white().bold());
        Ok(())
    }

    /// Add Flathub repository (most common)
    pub fn add_flathub(&self) -> Result<()> {
        self.add("flathub", "https://flathub.org/repo/flathub.flatpakrepo")
    }

    /// Remove a remote
    pub fn remove(&self, name: &str) -> Result<()> {
        println!("{} Removing remote: {}",
            style("::").cyan().bold(),
            style(name).yellow().bold());

        let status = Command::new("flatpak")
            .args(["remote-delete", "--force", self.install_flag(), name])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to remove remote")?;

        if !status.success() {
            return Err(anyhow!("Failed to remove remote {}", name));
        }

        println!("{} Remote {} removed", style("::").green().bold(), style(name).white().bold());
        Ok(())
    }

    /// Enable a remote
    pub fn enable(&self, name: &str) -> Result<()> {
        let status = Command::new("flatpak")
            .args(["remote-modify", "--enable", self.install_flag(), name])
            .status()
            .context("Failed to enable remote")?;

        if !status.success() {
            return Err(anyhow!("Failed to enable remote {}", name));
        }

        Ok(())
    }

    /// Disable a remote
    pub fn disable(&self, name: &str) -> Result<()> {
        let status = Command::new("flatpak")
            .args(["remote-modify", "--disable", self.install_flag(), name])
            .status()
            .context("Failed to disable remote")?;

        if !status.success() {
            return Err(anyhow!("Failed to disable remote {}", name));
        }

        Ok(())
    }

    /// Set remote priority (lower = higher priority)
    pub fn set_priority(&self, name: &str, priority: i32) -> Result<()> {
        let status = Command::new("flatpak")
            .args(["remote-modify", "--prio", &priority.to_string(), self.install_flag(), name])
            .status()
            .context("Failed to set priority")?;

        if !status.success() {
            return Err(anyhow!("Failed to set priority for {}", name));
        }

        Ok(())
    }

    /// Update remote metadata
    pub fn update(&self, name: &str) -> Result<()> {
        let status = Command::new("flatpak")
            .args(["update", "--appstream", self.install_flag(), name])
            .status()
            .context("Failed to update remote")?;

        if !status.success() {
            return Err(anyhow!("Failed to update remote {}", name));
        }

        Ok(())
    }

    /// Get remote info
    pub fn info(&self, name: &str) -> Result<Remote> {
        let remotes = self.list()?;
        remotes.into_iter()
            .find(|r| r.name == name)
            .ok_or_else(|| anyhow!("Remote {} not found", name))
    }
}

impl Default for RemoteManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse options string into RemoteOptions
fn parse_options(s: &str) -> RemoteOptions {
    let mut opts = RemoteOptions::default();
    opts.enabled = true; // Default to enabled
    
    for part in s.split(',') {
        let part = part.trim();
        match part {
            "disabled" => opts.enabled = false,
            "gpg-verify" => opts.gpg_verify = true,
            "gpg-verify-summary" => opts.gpg_verify_summary = true,
            _ => {
                if let Some(prio) = part.strip_prefix("prio=") {
                    opts.prio = prio.parse().unwrap_or(1);
                }
            }
        }
    }
    
    opts
}

/// Display remotes in a nice table
pub fn display_remotes(remotes: &[Remote]) {
    use comfy_table::{Table, Cell, Color, presets::UTF8_FULL};
    
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("URL").fg(Color::Cyan),
        Cell::new("Priority").fg(Color::Cyan),
        Cell::new("Enabled").fg(Color::Cyan),
    ]);
    
    for remote in remotes {
        let enabled_str = if remote.options.enabled { "Yes" } else { "No" };
        let enabled_color = if remote.options.enabled { Color::Green } else { Color::Red };
        
        table.add_row(vec![
            Cell::new(&remote.name).fg(Color::Yellow),
            Cell::new(&remote.url).fg(Color::White),
            Cell::new(remote.priority).fg(Color::Blue),
            Cell::new(enabled_str).fg(enabled_color),
        ]);
    }
    
    println!("{}", table);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_options() {
        let opts = parse_options("gpg-verify,disabled,prio=5");
        assert!(opts.gpg_verify);
        assert!(!opts.enabled);
        assert_eq!(opts.prio, 5);
    }
    
    #[test]
    fn test_remote_manager_new() {
        let rm = RemoteManager::new();
        assert!(rm.system);
        
        let rm = RemoteManager::user();
        assert!(!rm.system);
    }
}
