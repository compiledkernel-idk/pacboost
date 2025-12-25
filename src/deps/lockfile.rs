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

//! Lock file for reproducible builds.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

const LOCKFILE_NAME: &str = "pacboost.lock";
const LOCKFILE_VERSION: u32 = 1;

/// Lock file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lock file format version
    pub version: u32,
    /// When the lock file was created
    pub created: String,
    /// Locked packages
    pub packages: HashMap<String, LockedPackage>,
    /// Metadata
    pub metadata: LockfileMetadata,
}

/// Locked package entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub epoch: Option<u32>,
    pub pkgrel: String,
    pub arch: String,
    pub repository: String,
    pub sha256: Option<String>,
    pub dependencies: Vec<String>,
    pub source: PackageSource,
}

/// Source of the package
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PackageSource {
    Official { repo: String },
    Aur { pkgbase: String },
    Local { path: String },
}

/// Lockfile metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockfileMetadata {
    pub hostname: Option<String>,
    pub arch: String,
    pub pacman_version: Option<String>,
}

impl Lockfile {
    /// Create a new empty lock file
    pub fn new() -> Self {
        Self {
            version: LOCKFILE_VERSION,
            created: chrono::Utc::now().to_rfc3339(),
            packages: HashMap::new(),
            metadata: LockfileMetadata {
                hostname: fs::read_to_string("/etc/hostname")
                    .ok()
                    .map(|s| s.trim().to_string()),
                arch: std::env::consts::ARCH.to_string(),
                pacman_version: get_pacman_version(),
            },
        }
    }

    /// Load from file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .context("Failed to read lock file")?;
        let lockfile: Self = serde_json::from_str(&content)
            .context("Failed to parse lock file")?;
        
        if lockfile.version > LOCKFILE_VERSION {
            return Err(anyhow::anyhow!(
                "Lock file version {} is newer than supported version {}",
                lockfile.version,
                LOCKFILE_VERSION
            ));
        }
        
        Ok(lockfile)
    }

    /// Load from default location
    pub fn load_default() -> Result<Self> {
        let path = Self::default_path()?;
        Self::load(&path)
    }

    /// Get default lock file path
    pub fn default_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".config/pacboost").join(LOCKFILE_NAME))
    }

    /// Save to file
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize lock file")?;
        fs::write(path, content)
            .context("Failed to write lock file")?;
        
        Ok(())
    }

    /// Save to default location
    pub fn save_default(&self) -> Result<()> {
        let path = Self::default_path()?;
        self.save(&path)
    }

    /// Add a package to the lock file
    pub fn add_package(&mut self, pkg: LockedPackage) {
        self.packages.insert(pkg.name.clone(), pkg);
    }

    /// Remove a package from the lock file
    pub fn remove_package(&mut self, name: &str) -> Option<LockedPackage> {
        self.packages.remove(name)
    }

    /// Get a locked package
    pub fn get_package(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.get(name)
    }

    /// Check if a package is locked
    pub fn is_locked(&self, name: &str) -> bool {
        self.packages.contains_key(name)
    }

    /// Get package count
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// Diff with current system
    pub fn diff(&self, installed: &[(String, String)]) -> LockfileDiff {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut updated = Vec::new();

        let installed_map: HashMap<_, _> = installed.iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();

        // Check for removed or updated packages
        for (name, locked) in &self.packages {
            if let Some(current_version) = installed_map.get(name.as_str()) {
                if *current_version != locked.version {
                    updated.push((name.clone(), locked.version.clone(), current_version.to_string()));
                }
            } else {
                removed.push(name.clone());
            }
        }

        // Check for new packages
        for (name, _) in installed {
            if !self.packages.contains_key(name) {
                added.push(name.clone());
            }
        }

        LockfileDiff { added, removed, updated }
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

/// Difference between lock file and current state
#[derive(Debug, Clone, Default)]
pub struct LockfileDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<(String, String, String)>, // (name, locked_version, current_version)
}

impl LockfileDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.updated.is_empty()
    }

    pub fn display(&self) {
        use console::style;

        if self.is_empty() {
            println!("{} Lock file matches current state", style("✓").green().bold());
            return;
        }

        if !self.added.is_empty() {
            println!("{} New packages (not in lock):", style("+").green().bold());
            for name in &self.added {
                println!("   + {}", style(name).green());
            }
        }

        if !self.removed.is_empty() {
            println!("{} Removed packages (in lock but not installed):", style("-").red().bold());
            for name in &self.removed {
                println!("   - {}", style(name).red());
            }
        }

        if !self.updated.is_empty() {
            println!("{} Updated packages:", style("~").yellow().bold());
            for (name, locked, current) in &self.updated {
                println!("   ~ {} {} -> {}", 
                    style(name).yellow(),
                    style(locked).dim(),
                    style(current).green());
            }
        }
    }
}

/// Get pacman version
fn get_pacman_version() -> Option<String> {
    std::process::Command::new("pacman")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(|l| l.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_lockfile_new() {
        let lockfile = Lockfile::new();
        assert_eq!(lockfile.version, LOCKFILE_VERSION);
        assert!(lockfile.packages.is_empty());
    }

    #[test]
    fn test_add_remove_package() {
        let mut lockfile = Lockfile::new();
        
        lockfile.add_package(LockedPackage {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            epoch: None,
            pkgrel: "1".to_string(),
            arch: "x86_64".to_string(),
            repository: "extra".to_string(),
            sha256: None,
            dependencies: Vec::new(),
            source: PackageSource::Official { repo: "extra".to_string() },
        });
        
        assert_eq!(lockfile.len(), 1);
        assert!(lockfile.is_locked("test"));
        
        lockfile.remove_package("test");
        assert_eq!(lockfile.len(), 0);
    }

    #[test]
    fn test_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.lock");
        
        let mut lockfile = Lockfile::new();
        lockfile.add_package(LockedPackage {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            epoch: None,
            pkgrel: "1".to_string(),
            arch: "x86_64".to_string(),
            repository: "extra".to_string(),
            sha256: None,
            dependencies: Vec::new(),
            source: PackageSource::Official { repo: "extra".to_string() },
        });
        
        lockfile.save(&path).unwrap();
        let loaded = Lockfile::load(&path).unwrap();
        
        assert_eq!(loaded.len(), 1);
        assert!(loaded.is_locked("test"));
    }

    #[test]
    fn test_diff() {
        let mut lockfile = Lockfile::new();
        lockfile.add_package(LockedPackage {
            name: "a".to_string(),
            version: "1.0".to_string(),
            epoch: None,
            pkgrel: "1".to_string(),
            arch: "x86_64".to_string(),
            repository: "extra".to_string(),
            sha256: None,
            dependencies: Vec::new(),
            source: PackageSource::Official { repo: "extra".to_string() },
        });
        lockfile.add_package(LockedPackage {
            name: "b".to_string(),
            version: "1.0".to_string(),
            epoch: None,
            pkgrel: "1".to_string(),
            arch: "x86_64".to_string(),
            repository: "extra".to_string(),
            sha256: None,
            dependencies: Vec::new(),
            source: PackageSource::Official { repo: "extra".to_string() },
        });
        
        let installed = vec![
            ("a".to_string(), "2.0".to_string()),  // updated
            ("c".to_string(), "1.0".to_string()),  // added
        ];
        
        let diff = lockfile.diff(&installed);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.updated.len(), 1);
    }
}
