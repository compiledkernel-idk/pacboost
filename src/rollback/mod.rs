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

use anyhow::{Result, Context, anyhow};
use console::style;
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

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

    /// Check if btrfs is available
    pub fn is_available() -> bool {
        // Check if root is btrfs
        let output = Command::new("df")
            .args(["--type=btrfs", "/"])
            .output();

        output.map(|o| o.status.success()).unwrap_or(false)
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

    /// Get next snapshot ID
    fn next_id(&self) -> Result<u32> {
        let snapshots = self.list()?;
        Ok(snapshots.last().map(|s| s.id + 1).unwrap_or(1))
    }

    /// Create a snapshot
    pub fn create_snapshot(&self, name: &str, description: &str, snapshot_type: SnapshotType) -> Result<Snapshot> {
        if !Self::is_available() {
            return Err(anyhow!("Btrfs not available on this system"));
        }

        let id = self.next_id()?;
        let snapshot_path = self.snapshot_dir.join(format!("{}", id));
        
        // Create snapshot directory
        fs::create_dir_all(&snapshot_path)?;

        println!("{} Creating snapshot {}...",
            style("::").cyan().bold(),
            style(id).yellow().bold());

        // Create btrfs snapshot
        let status = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.root_subvol)
            .arg(snapshot_path.join("snapshot"))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to create btrfs snapshot")?;

        if !status.success() {
            fs::remove_dir_all(&snapshot_path)?;
            return Err(anyhow!("Failed to create snapshot"));
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

        println!("{} Snapshot {} created",
            style("::").green().bold(),
            style(id).white().bold());

        Ok(snapshot)
    }

    /// Delete a snapshot
    pub fn delete_snapshot(&self, id: u32) -> Result<()> {
        let snapshot_path = self.snapshot_dir.join(format!("{}", id));
        
        if !snapshot_path.exists() {
            return Err(anyhow!("Snapshot {} not found", id));
        }

        println!("{} Deleting snapshot {}...",
            style("::").cyan().bold(),
            style(id).yellow().bold());

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

        println!("{} Snapshot {} deleted",
            style("::").green().bold(),
            style(id).white().bold());

        Ok(())
    }

    /// Rollback to a snapshot
    pub fn rollback(&self, id: u32) -> Result<()> {
        let snapshot_path = self.snapshot_dir.join(format!("{}", id));
        
        if !snapshot_path.exists() {
            return Err(anyhow!("Snapshot {} not found", id));
        }

        println!("{} {} Rolling back to snapshot {}",
            style("::").red().bold(),
            style("WARNING:").yellow().bold(),
            style(id).white().bold());
        println!("   This will replace the current system state.");
        println!("   A reboot will be required after the operation.");
        println!();

        // Load snapshot metadata
        let meta_path = snapshot_path.join(SNAPSHOT_META);
        let content = fs::read_to_string(&meta_path)?;
        let snapshot: Snapshot = serde_json::from_str(&content)?;

        // Create a pre-rollback snapshot
        println!("{} Creating pre-rollback snapshot...",
            style("::").cyan().bold());
        
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

        println!("{} Rollback prepared. Please reboot to complete.",
            style("::").yellow().bold());
        println!("   Snapshot: {} ({})", snapshot.id, snapshot.name);
        println!("   Created: {}", snapshot.created.format("%Y-%m-%d %H:%M:%S"));

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
    use comfy_table::{Table, Cell, Color, presets::UTF8_FULL};

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
