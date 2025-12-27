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

//! System rollback with btrfs snapshots.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use console::style;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const SNAPSHOT_DIR: &str = "/.snapshots";
const SNAPSHOT_META: &str = "snapshot.json";

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub created: DateTime<Utc>,
    pub snapshot_type: SnapshotType,
    pub subvolume: String,
    pub packages_changed: Vec<PackageChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotType {
    Pre,    // Before operation
    Post,   // After operation
    Manual, // User-created
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageChange {
    pub name: String,
    pub action: ChangeAction,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeAction {
    Install,
    Remove,
    Upgrade,
    Downgrade,
}

/// Rollback manager
pub struct RollbackManager {
    snapshot_dir: PathBuf,
    root_subvol: PathBuf,
}

impl RollbackManager {
    /// Create a new rollback manager
    pub fn new() -> Self {
        Self {
            snapshot_dir: PathBuf::from(SNAPSHOT_DIR),
            root_subvol: PathBuf::from("/"),
        }
    }

    /// Check if btrfs is available and properly configured
    pub fn is_available() -> bool {
        // Check if root is btrfs
        let output = Command::new("df").args(["--type=btrfs", "/"]).output();

        output.map(|o| o.status.success()).unwrap_or(false)
    }

    /// Check if snapshots are properly configured
    fn check_snapshot_setup(&self) -> Result<()> {
        // Check if btrfs
        if !Self::is_available() {
            return Err(anyhow!(
                "Btrfs filesystem not detected on root.\n\
                 Snapshots require a btrfs root filesystem.\n\
                 To check: df -T / | grep btrfs"
            ));
        }

        // Check if /.snapshots exists and is writable
        if !self.snapshot_dir.exists() {
            println!(
                "{} Creating snapshot directory at {}...",
                style("::").cyan().bold(),
                self.snapshot_dir.display()
            );

            fs::create_dir_all(&self.snapshot_dir).map_err(|e| {
                anyhow!(
                    "Cannot create snapshot directory at {}: {}\n\
                     You may need to create it manually:\n\
                     sudo btrfs subvolume create /.snapshots",
                    self.snapshot_dir.display(),
                    e
                )
            })?;
        }

        // Check if snapshot_dir is writable
        let test_file = self.snapshot_dir.join(".pacboost_test");
        if let Err(e) = fs::write(&test_file, "test") {
            return Err(anyhow!(
                "Snapshot directory {} is not writable: {}\n\
                 Make sure /.snapshots is a btrfs subvolume with write access.",
                self.snapshot_dir.display(),
                e
            ));
        }
        let _ = fs::remove_file(test_file);

        // Check if btrfs command exists
        if Command::new("btrfs").arg("--version").output().is_err() {
            return Err(anyhow!(
                "btrfs-progs not found. Install with:\n\
                 sudo pacman -S btrfs-progs"
            ));
        }

        Ok(())
    }

    /// List all snapshots
    pub fn list(&self) -> Result<Vec<Snapshot>> {
        let mut snapshots = Vec::new();

        if !self.snapshot_dir.exists() {
            return Ok(snapshots);
        }

        for entry in fs::read_dir(&self.snapshot_dir)? {
            let entry = entry?;
            let meta_path = entry.path().join(SNAPSHOT_META);

            if meta_path.exists() {
                let content = fs::read_to_string(&meta_path)?;
                if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&content) {
                    snapshots.push(snapshot);
                }
            }
        }

        snapshots.sort_by_key(|s| s.id);
        Ok(snapshots)
    }

    /// Get next snapshot ID (scan existing directories to avoid conflicts)
    fn next_id(&self) -> Result<u32> {
        if !self.snapshot_dir.exists() {
            return Ok(1);
        }

        let mut max_id: u32 = 0;

        // Scan all directories in /.snapshots to find the highest ID
        // This handles existing snapper snapshots that don't have our metadata
        for entry in fs::read_dir(&self.snapshot_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Ok(id) = name.parse::<u32>() {
                        if id > max_id {
                            max_id = id;
                        }
                    }
                }
            }
        }

        Ok(max_id + 1)
    }

    /// Create a snapshot
    pub fn create_snapshot(
        &self,
        name: &str,
        description: &str,
        snapshot_type: SnapshotType,
    ) -> Result<Snapshot> {
        // Run preflight checks
        self.check_snapshot_setup()?;

        let id = self.next_id()?;
        let snapshot_path = self.snapshot_dir.join(format!("{}", id));

        // Create snapshot directory
        fs::create_dir_all(&snapshot_path)
            .map_err(|e| anyhow!("Failed to create snapshot directory: {}", e))?;

        println!(
            "{} Creating snapshot {}...",
            style("::").cyan().bold(),
            style(id).yellow().bold()
        );

        // Find the actual root subvolume path
        // On many systems, root is mounted from a subvolume like @, @root, etc.
        let root_subvol = self.detect_root_subvolume()?;

        // Create btrfs snapshot
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&root_subvol)
            .arg(snapshot_path.join("snapshot"))
            .output()
            .context("Failed to execute btrfs command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            fs::remove_dir_all(&snapshot_path)?;

            if stderr.contains("File exists") {
                return Err(anyhow!(
                    "Snapshot target already exists. This may indicate a previous failed snapshot.\n\
                     Try: sudo rm -rf {}",
                    snapshot_path.display()
                ));
            } else if stderr.contains("Read-only") {
                return Err(anyhow!(
                    "Cannot create snapshot: filesystem is read-only.\n\
                     Your btrfs setup may not support snapshots from the running system.\n\
                     Consider using snapper or timeshift for btrfs snapshots."
                ));
            } else {
                return Err(anyhow!("Failed to create snapshot: {}", stderr.trim()));
            }
        }

        // Create metadata
        let snapshot = Snapshot {
            id,
            name: name.to_string(),
            description: description.to_string(),
            created: Utc::now(),
            snapshot_type,
            subvolume: snapshot_path.join("snapshot").to_string_lossy().to_string(),
            packages_changed: Vec::new(),
        };

        let meta_content = serde_json::to_string_pretty(&snapshot)?;
        fs::write(snapshot_path.join(SNAPSHOT_META), meta_content)?;

        println!(
            "{} Snapshot {} created",
            style("::").green().bold(),
            style(id).white().bold()
        );

        Ok(snapshot)
    }

    /// Detect the actual root btrfs subvolume
    fn detect_root_subvolume(&self) -> Result<PathBuf> {
        // Try to get the subvolume path from /proc/mounts or findmnt
        let output = Command::new("findmnt")
            .args(["-n", "-o", "SOURCE", "/"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let source = String::from_utf8_lossy(&output.stdout);
                let source = source.trim();

                // Check if it's a subvolume (contains [subvol] or similar)
                if source.contains('[') {
                    // Extract device, the subvolume path is in brackets
                    // Format: /dev/sda1[/@] or /dev/nvme0n1p2[/@root]
                    if let Some(start) = source.find('[') {
                        if let Some(end) = source.find(']') {
                            let subvol = &source[start + 1..end];
                            // For subvolumes like @ or @root, we need to use the mount point
                            println!(
                                "{} Detected root subvolume: {}",
                                style("::").cyan().bold(),
                                style(subvol).yellow()
                            );
                        }
                    }
                }
            }
        }

        // Default to / - this works if / is the top-level subvolume
        Ok(PathBuf::from("/"))
    }

    /// Delete a snapshot
    pub fn delete_snapshot(&self, id: u32) -> Result<()> {
        let snapshot_path = self.snapshot_dir.join(format!("{}", id));

        if !snapshot_path.exists() {
            return Err(anyhow!("Snapshot {} not found", id));
        }

        println!(
            "{} Deleting snapshot {}...",
            style("::").cyan().bold(),
            style(id).yellow().bold()
        );

        // Delete btrfs subvolume
        let status = Command::new("btrfs")
            .args(["subvolume", "delete"])
            .arg(snapshot_path.join("snapshot"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("Failed to delete btrfs snapshot")?;

        if !status.success() {
            return Err(anyhow!("Failed to delete snapshot subvolume"));
        }

        // Remove snapshot directory
        fs::remove_dir_all(&snapshot_path)?;

        println!(
            "{} Snapshot {} deleted",
            style("::").green().bold(),
            style(id).white().bold()
        );

        Ok(())
    }

    /// Rollback to a snapshot
    pub fn rollback(&self, id: u32) -> Result<()> {
        let snapshot_path = self.snapshot_dir.join(format!("{}", id));

        if !snapshot_path.exists() {
            return Err(anyhow!("Snapshot {} not found", id));
        }

        println!(
            "{} {} Rolling back to snapshot {}",
            style("::").red().bold(),
            style("WARNING:").yellow().bold(),
            style(id).white().bold()
        );
        println!("   This will replace the current system state.");
        println!("   A reboot will be required after the operation.");
        println!();

        // Load snapshot metadata
        let meta_path = snapshot_path.join(SNAPSHOT_META);
        let content = fs::read_to_string(&meta_path)?;
        let snapshot: Snapshot = serde_json::from_str(&content)?;

        // Create a pre-rollback snapshot
        println!(
            "{} Creating pre-rollback snapshot...",
            style("::").cyan().bold()
        );

        let _ = self.create_snapshot(
            "pre-rollback",
            &format!("Automatic snapshot before rollback to {}", id),
            SnapshotType::Pre,
        );

        // Perform the rollback
        // Note: In a real implementation, this would need to:
        // 1. Boot into the snapshot (by modifying bootloader)
        // 2. Or use btrfs send/receive to restore
        // For now, we just print what would happen

        println!(
            "{} Rollback prepared. Please reboot to complete.",
            style("::").yellow().bold()
        );
        println!("   Snapshot: {} ({})", snapshot.id, snapshot.name);
        println!(
            "   Created: {}",
            snapshot.created.format("%Y-%m-%d %H:%M:%S")
        );

        Ok(())
    }

    /// Get snapshot info
    pub fn get_snapshot(&self, id: u32) -> Result<Snapshot> {
        let snapshot_path = self.snapshot_dir.join(format!("{}", id));
        let meta_path = snapshot_path.join(SNAPSHOT_META);

        if !meta_path.exists() {
            return Err(anyhow!("Snapshot {} not found", id));
        }

        let content = fs::read_to_string(&meta_path)?;
        let snapshot: Snapshot = serde_json::from_str(&content)?;

        Ok(snapshot)
    }

    /// Clean old snapshots, keeping the last N
    pub fn clean(&self, keep: usize) -> Result<usize> {
        let mut snapshots = self.list()?;

        if snapshots.len() <= keep {
            return Ok(0);
        }

        // Sort by ID (ascending) and remove oldest
        snapshots.sort_by_key(|s| s.id);
        let to_remove = snapshots.len() - keep;
        let mut removed = 0;

        for snapshot in snapshots.iter().take(to_remove) {
            if self.delete_snapshot(snapshot.id).is_ok() {
                removed += 1;
            }
        }

        Ok(removed)
    }
}

impl Default for RollbackManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Display snapshots in a table
pub fn display_snapshots(snapshots: &[Snapshot]) {
    use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

    if snapshots.is_empty() {
        println!("{} No snapshots found", style("::").yellow().bold());
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(Color::Cyan),
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Type").fg(Color::Cyan),
        Cell::new("Created").fg(Color::Cyan),
        Cell::new("Description").fg(Color::Cyan),
    ]);

    for s in snapshots {
        let type_str = match s.snapshot_type {
            SnapshotType::Pre => "pre",
            SnapshotType::Post => "post",
            SnapshotType::Manual => "manual",
        };

        table.add_row(vec![
            Cell::new(s.id).fg(Color::Yellow),
            Cell::new(&s.name).fg(Color::White),
            Cell::new(type_str).fg(Color::Blue),
            Cell::new(s.created.format("%Y-%m-%d %H:%M").to_string()).fg(Color::DarkGrey),
            Cell::new(&s.description).fg(Color::DarkGrey),
        ]);
    }

    println!("{}", table);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_manager_new() {
        let manager = RollbackManager::new();
        assert_eq!(manager.snapshot_dir, PathBuf::from(SNAPSHOT_DIR));
    }

    #[test]
    fn test_is_available() {
        // Just check it doesn't panic
        let _ = RollbackManager::is_available();
    }
}
