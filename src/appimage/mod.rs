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

//! AppImage management for pacboost.
//!
//! Provides AppImage discovery, installation, and updates:
//! - AppImageHub integration
//! - Desktop integration
//! - Update checking via zsync

use anyhow::{anyhow, Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const APPIMAGEHUB_API: &str = "https://appimage.github.io/feed.json";
const DEFAULT_APPIMAGE_DIR: &str = "~/.local/bin";

/// AppImage application info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppImage {
    pub name: String,
    pub version: Option<String>,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub executable: bool,
    pub integrated: bool,
}

/// AppImageHub entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppImageHubEntry {
    pub name: String,
    pub description: Option<String>,
    pub categories: Vec<String>,
    pub authors: Vec<Author>,
    pub license: Option<String>,
    pub links: Vec<Link>,
    pub icons: Vec<String>,
    pub screenshots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    #[serde(rename = "type")]
    pub link_type: String,
    pub url: String,
}

/// AppImage manager
pub struct AppImageManager {
    /// Directory to store AppImages
    install_dir: PathBuf,
    /// Applications directory for .desktop files
    applications_dir: PathBuf,
    /// Cache for AppImageHub data
    hub_cache: Option<Vec<AppImageHubEntry>>,
}

impl AppImageManager {
    /// Create a new AppImage manager
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            install_dir: home.join(".local/bin"),
            applications_dir: home.join(".local/share/applications"),
            hub_cache: None,
        }
    }

    /// Create with custom install directory
    pub fn with_dir(dir: PathBuf) -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            install_dir: dir,
            applications_dir: home.join(".local/share/applications"),
            hub_cache: None,
        }
    }

    /// Ensure install directory exists
    fn ensure_dir(&self) -> Result<()> {
        if !self.install_dir.exists() {
            fs::create_dir_all(&self.install_dir).context("Failed to create AppImage directory")?;
        }
        if !self.applications_dir.exists() {
            fs::create_dir_all(&self.applications_dir)
                .context("Failed to create applications directory")?;
        }
        Ok(())
    }

    /// List installed AppImages
    pub fn list(&self) -> Result<Vec<AppImage>> {
        self.ensure_dir()?;

        let mut appimages = Vec::new();

        for entry in fs::read_dir(&self.install_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.to_lowercase().ends_with(".appimage") {
                        let metadata = fs::metadata(&path)?;
                        let executable = metadata.permissions().mode() & 0o111 != 0;

                        let desktop_file = self.applications_dir.join(format!(
                            "{}.desktop",
                            name.replace(".AppImage", "").replace(".appimage", "")
                        ));

                        appimages.push(AppImage {
                            name: name.replace(".AppImage", "").replace(".appimage", ""),
                            version: extract_version(name),
                            path: path.clone(),
                            size_bytes: metadata.len(),
                            executable,
                            integrated: desktop_file.exists(),
                        });
                    }
                }
            }
        }

        Ok(appimages)
    }

    /// Download and install an AppImage from URL
    pub async fn install_from_url(&self, name: &str, url: &str) -> Result<PathBuf> {
        self.ensure_dir()?;

        println!(
            "{} Installing AppImage: {}",
            style("::").cyan().bold(),
            style(name).yellow().bold()
        );

        let filename = if url.contains(".AppImage") || url.contains(".appimage") {
            url.split('/').next_back().unwrap_or(name).to_string()
        } else {
            format!("{}.AppImage", name)
        };

        let target_path = self.install_dir.join(&filename);

        // Download with progress
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.set_message(format!("Downloading {}...", name));
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .context("Failed to download AppImage")?;

        if !response.status().is_success() {
            pb.finish_and_clear();
            return Err(anyhow!("Download failed: HTTP {}", response.status()));
        }

        let bytes = response.bytes().await?;
        pb.finish_and_clear();

        // Write file
        fs::write(&target_path, &bytes)?;

        // Make executable
        let mut perms = fs::metadata(&target_path)?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        fs::set_permissions(&target_path, perms)?;

        println!(
            "{} {} installed to {}",
            style("::").green().bold(),
            style(name).white().bold(),
            style(target_path.display()).dim()
        );

        Ok(target_path)
    }

    /// Install from AppImageHub by name
    pub async fn install_from_hub(&self, name: &str) -> Result<PathBuf> {
        println!(
            "{} Searching AppImageHub for {}...",
            style("::").cyan().bold(),
            style(name).yellow().bold()
        );

        let entries = self.search_hub(name).await?;

        if entries.is_empty() {
            return Err(anyhow!("No AppImage found for '{}'", name));
        }

        let entry = &entries[0];

        // Find download link
        let download_url = entry
            .links
            .iter()
            .find(|l| l.link_type == "Download" || l.link_type.to_lowercase().contains("appimage"))
            .ok_or_else(|| anyhow!("No download link found for {}", name))?;

        self.install_from_url(&entry.name, &download_url.url).await
    }

    /// Search AppImageHub
    pub async fn search_hub(&self, query: &str) -> Result<Vec<AppImageHubEntry>> {
        let client = reqwest::Client::new();
        let response = client
            .get(APPIMAGEHUB_API)
            .send()
            .await
            .context("Failed to fetch AppImageHub data")?;

        if !response.status().is_success() {
            return Err(anyhow!("AppImageHub request failed"));
        }

        let data: serde_json::Value = response.json().await?;

        let items = data
            .get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("Invalid AppImageHub response"))?;

        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for item in items {
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");

            if name.to_lowercase().contains(&query_lower) {
                if let Ok(entry) = serde_json::from_value::<AppImageHubEntry>(item.clone()) {
                    results.push(entry);
                }
            }
        }

        Ok(results)
    }

    /// Remove an AppImage
    pub fn remove(&self, name: &str) -> Result<()> {
        let appimages = self.list()?;

        let app = appimages
            .iter()
            .find(|a| a.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| anyhow!("AppImage '{}' not found", name))?;

        println!(
            "{} Removing AppImage: {}",
            style("::").cyan().bold(),
            style(&app.name).yellow().bold()
        );

        // Remove AppImage file
        fs::remove_file(&app.path).context("Failed to remove AppImage")?;

        // Remove desktop file if exists
        let desktop_file = self.applications_dir.join(format!("{}.desktop", app.name));
        if desktop_file.exists() {
            let _ = fs::remove_file(&desktop_file);
        }

        println!(
            "{} {} removed",
            style("::").green().bold(),
            style(name).white().bold()
        );

        Ok(())
    }

    /// Integrate AppImage with desktop (create .desktop file)
    pub fn integrate(&self, name: &str) -> Result<()> {
        let appimages = self.list()?;

        let app = appimages
            .iter()
            .find(|a| a.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| anyhow!("AppImage '{}' not found", name))?;

        println!(
            "{} Integrating {} with desktop...",
            style("::").cyan().bold(),
            style(&app.name).yellow().bold()
        );

        // Try to extract icon and desktop file using --appimage-extract
        let extract_result = Command::new(&app.path)
            .args(["--appimage-extract", "*.desktop"])
            .current_dir(&self.install_dir)
            .output();

        let desktop_content = if let Ok(output) = extract_result {
            if output.status.success() {
                // Try to read extracted desktop file
                let squashfs = self.install_dir.join("squashfs-root");
                let desktop_files: Vec<_> = fs::read_dir(&squashfs)
                    .ok()
                    .map(|entries| {
                        entries
                            .filter_map(|e| e.ok())
                            .filter(|e| {
                                e.path()
                                    .extension()
                                    .map(|ext| ext == "desktop")
                                    .unwrap_or(false)
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                if let Some(df) = desktop_files.first() {
                    let content = fs::read_to_string(df.path()).ok();
                    // Clean up
                    let _ = fs::remove_dir_all(&squashfs);
                    content
                } else {
                    let _ = fs::remove_dir_all(&squashfs);
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Create desktop file
        let desktop_content = desktop_content.unwrap_or_else(|| {
            format!(
                r#"[Desktop Entry]
Type=Application
Name={}
Exec={}
Icon=application-x-executable
Terminal=false
Categories=Utility;
"#,
                app.name,
                app.path.display()
            )
        });

        // Update Exec path in desktop file
        let desktop_content = desktop_content
            .lines()
            .map(|line| {
                if line.starts_with("Exec=") {
                    format!("Exec={}", app.path.display())
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let desktop_path = self.applications_dir.join(format!("{}.desktop", app.name));

        fs::write(&desktop_path, desktop_content)?;

        // Make desktop file executable
        let mut perms = fs::metadata(&desktop_path)?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        fs::set_permissions(&desktop_path, perms)?;

        println!(
            "{} {} integrated with desktop",
            style("::").green().bold(),
            style(&app.name).white().bold()
        );

        Ok(())
    }

    /// Run an AppImage
    pub fn run(&self, name: &str, args: &[String]) -> Result<()> {
        let appimages = self.list()?;

        let app = appimages
            .iter()
            .find(|a| a.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| anyhow!("AppImage '{}' not found", name))?;

        let mut cmd = Command::new(&app.path);
        cmd.args(args);
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let status = cmd.status().context("Failed to run AppImage")?;

        if !status.success() {
            return Err(anyhow!("AppImage exited with error"));
        }

        Ok(())
    }

    /// Check for updates (basic - checks if zsync file exists)
    pub async fn check_update(&self, name: &str) -> Result<bool> {
        let appimages = self.list()?;

        let app = appimages
            .iter()
            .find(|a| a.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| anyhow!("AppImage '{}' not found", name))?;

        // Try to run with --appimage-update-information
        let output = Command::new(&app.path)
            .arg("--appimage-update-information")
            .output();

        Ok(output.map(|o| o.status.success()).unwrap_or(false))
    }

    /// Get total disk usage
    pub fn disk_usage(&self) -> Result<u64> {
        let apps = self.list()?;
        Ok(apps.iter().map(|a| a.size_bytes).sum())
    }
}

impl Default for AppImageManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract version from AppImage filename
fn extract_version(filename: &str) -> Option<String> {
    // Common patterns: App-1.2.3.AppImage, App_v1.2.3.AppImage
    let re = regex::Regex::new(r"[-_]v?(\d+\.\d+(?:\.\d+)?(?:-\w+)?)[_-]?").ok()?;
    re.captures(filename)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Display AppImages in a table
pub fn display_appimages(apps: &[AppImage]) {
    use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Version").fg(Color::Cyan),
        Cell::new("Size").fg(Color::Cyan),
        Cell::new("Executable").fg(Color::Cyan),
        Cell::new("Integrated").fg(Color::Cyan),
    ]);

    for app in apps {
        let size = format_size(app.size_bytes);
        let exec_color = if app.executable {
            Color::Green
        } else {
            Color::Red
        };
        let int_color = if app.integrated {
            Color::Green
        } else {
            Color::Yellow
        };

        table.add_row(vec![
            Cell::new(&app.name).fg(Color::White),
            Cell::new(app.version.as_deref().unwrap_or("-")).fg(Color::Green),
            Cell::new(&size).fg(Color::Magenta),
            Cell::new(if app.executable { "Yes" } else { "No" }).fg(exec_color),
            Cell::new(if app.integrated { "Yes" } else { "No" }).fg(int_color),
        ]);
    }

    println!("{}", table);
}

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
    fn test_extract_version() {
        assert_eq!(
            extract_version("Firefox-120.0.AppImage"),
            Some("120.0".to_string())
        );
        assert_eq!(
            extract_version("App_v1.2.3_x86_64.AppImage"),
            Some("1.2.3".to_string())
        );
        assert_eq!(extract_version("SimpleApp.AppImage"), None);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
    }

    #[test]
    fn test_appimage_manager_new() {
        let manager = AppImageManager::new();
        assert!(manager.install_dir.to_string_lossy().contains(".local/bin"));
    }
}
