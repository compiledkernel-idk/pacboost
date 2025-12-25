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

//! Smart package cache with LRU eviction.

use anyhow::{Result, Context};
use console::style;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

const CACHE_INDEX_FILE: &str = "cache_index.json";

/// Cache entry metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub filename: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub added_time: i64,
    pub last_accessed: i64,
    pub access_count: u32,
    pub package_name: String,
    pub version: String,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_size_bytes: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub oldest_entry: Option<i64>,
    pub newest_entry: Option<i64>,
}

/// Package cache manager with LRU eviction
pub struct PackageCache {
    cache_dir: PathBuf,
    max_size_bytes: u64,
    entries: HashMap<String, CacheEntry>,
    stats: CacheStats,
}

impl PackageCache {
    /// Create a new cache manager
    pub fn new(cache_dir: PathBuf, max_size_mb: u64) -> Result<Self> {
        fs::create_dir_all(&cache_dir)?;
        
        let mut cache = Self {
            cache_dir: cache_dir.clone(),
            max_size_bytes: max_size_mb * 1024 * 1024,
            entries: HashMap::new(),
            stats: CacheStats::default(),
        };
        
        cache.load_index()?;
        cache.update_stats();
        
        Ok(cache)
    }

    /// Load cache index from disk
    fn load_index(&mut self) -> Result<()> {
        let index_path = self.cache_dir.join(CACHE_INDEX_FILE);
        
        if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            self.entries = serde_json::from_str(&content)
                .unwrap_or_default();
        }
        
        // Verify entries still exist
        self.entries.retain(|_, entry| {
            self.cache_dir.join(&entry.filename).exists()
        });
        
        Ok(())
    }

    /// Save cache index to disk
    fn save_index(&self) -> Result<()> {
        let index_path = self.cache_dir.join(CACHE_INDEX_FILE);
        let content = serde_json::to_string_pretty(&self.entries)?;
        fs::write(index_path, content)?;
        Ok(())
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.total_entries = self.entries.len();
        self.stats.total_size_bytes = self.entries.values()
            .map(|e| e.size_bytes)
            .sum();
        
        if let Some(oldest) = self.entries.values().map(|e| e.added_time).min() {
            self.stats.oldest_entry = Some(oldest);
        }
        if let Some(newest) = self.entries.values().map(|e| e.added_time).max() {
            self.stats.newest_entry = Some(newest);
        }
    }

    /// Check if a file is in cache by SHA256
    pub fn get_by_hash(&mut self, sha256: &str) -> Option<PathBuf> {
        if let Some(entry) = self.entries.get_mut(sha256) {
            entry.last_accessed = chrono::Utc::now().timestamp();
            entry.access_count += 1;
            self.stats.cache_hits += 1;
            
            let path = self.cache_dir.join(&entry.filename);
            if path.exists() {
                let _ = self.save_index();
                return Some(path);
            }
        }
        
        self.stats.cache_misses += 1;
        None
    }

    /// Check if a package is in cache
    pub fn get_package(&mut self, name: &str, version: &str) -> Option<PathBuf> {
        for entry in self.entries.values_mut() {
            if entry.package_name == name && entry.version == version {
                entry.last_accessed = chrono::Utc::now().timestamp();
                entry.access_count += 1;
                self.stats.cache_hits += 1;
                
                let path = self.cache_dir.join(&entry.filename);
                if path.exists() {
                    let _ = self.save_index();
                    return Some(path);
                }
            }
        }
        
        self.stats.cache_misses += 1;
        None
    }

    /// Add a file to the cache
    pub fn add(&mut self, source_path: &Path, package_name: &str, version: &str) -> Result<PathBuf> {
        let content = fs::read(source_path)?;
        let sha256 = hex::encode(Sha256::digest(&content));
        
        // Check if already cached
        if let Some(entry) = self.entries.get(&sha256) {
            return Ok(self.cache_dir.join(&entry.filename));
        }
        
        let filename = source_path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
        
        let size = content.len() as u64;
        
        // Evict if necessary
        while self.stats.total_size_bytes + size > self.max_size_bytes {
            if !self.evict_lru() {
                break;
            }
        }
        
        // Copy file to cache
        let cache_path = self.cache_dir.join(filename);
        fs::write(&cache_path, &content)?;
        
        // Add entry
        let now = chrono::Utc::now().timestamp();
        self.entries.insert(sha256.clone(), CacheEntry {
            filename: filename.to_string(),
            size_bytes: size,
            sha256,
            added_time: now,
            last_accessed: now,
            access_count: 1,
            package_name: package_name.to_string(),
            version: version.to_string(),
        });
        
        self.update_stats();
        self.save_index()?;
        
        Ok(cache_path)
    }

    /// Evict least recently used entry
    fn evict_lru(&mut self) -> bool {
        let lru_key = self.entries.iter()
            .min_by_key(|(_, e)| e.last_accessed)
            .map(|(k, _)| k.clone());
        
        if let Some(key) = lru_key {
            if let Some(entry) = self.entries.remove(&key) {
                let path = self.cache_dir.join(&entry.filename);
                let _ = fs::remove_file(&path);
                self.update_stats();
                return true;
            }
        }
        
        false
    }

    /// Clean old entries
    pub fn clean_old(&mut self, max_age_days: u32) -> Result<CleanResult> {
        let cutoff = chrono::Utc::now().timestamp() - (max_age_days as i64 * 86400);
        let mut removed_count = 0;
        let mut removed_bytes = 0u64;
        
        let to_remove: Vec<String> = self.entries.iter()
            .filter(|(_, e)| e.last_accessed < cutoff)
            .map(|(k, _)| k.clone())
            .collect();
        
        for key in to_remove {
            if let Some(entry) = self.entries.remove(&key) {
                let path = self.cache_dir.join(&entry.filename);
                if let Ok(()) = fs::remove_file(&path) {
                    removed_count += 1;
                    removed_bytes += entry.size_bytes;
                }
            }
        }
        
        self.update_stats();
        self.save_index()?;
        
        Ok(CleanResult {
            removed_count,
            removed_bytes,
        })
    }

    /// Clear entire cache
    pub fn clear(&mut self) -> Result<CleanResult> {
        let removed_count = self.entries.len();
        let removed_bytes = self.stats.total_size_bytes;
        
        for entry in self.entries.values() {
            let path = self.cache_dir.join(&entry.filename);
            let _ = fs::remove_file(&path);
        }
        
        self.entries.clear();
        self.update_stats();
        self.save_index()?;
        
        Ok(CleanResult {
            removed_count,
            removed_bytes,
        })
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Calculate the hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.stats.cache_hits + self.stats.cache_misses;
        if total == 0 {
            return 0.0;
        }
        (self.stats.cache_hits as f64 / total as f64) * 100.0
    }
}

/// Result of a clean operation
#[derive(Debug, Clone)]
pub struct CleanResult {
    pub removed_count: usize,
    pub removed_bytes: u64,
}

/// Display cache statistics
pub fn display_cache_stats(stats: &CacheStats, hit_rate: f64) {
    println!();
    println!("{} {}", 
        style("::").cyan().bold(),
        style("Package Cache Statistics").white().bold());
    
    println!("   Entries: {}", stats.total_entries);
    println!("   Total Size: {}", format_size(stats.total_size_bytes));
    println!("   Cache Hits: {}", stats.cache_hits);
    println!("   Cache Misses: {}", stats.cache_misses);
    println!("   Hit Rate: {:.1}%", hit_rate);
    
    if let Some(oldest) = stats.oldest_entry {
        println!("   Oldest Entry: {}", format_timestamp(oldest));
    }
    if let Some(newest) = stats.newest_entry {
        println!("   Newest Entry: {}", format_timestamp(newest));
    }
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

fn format_timestamp(ts: i64) -> String {
    use chrono::{DateTime, Utc};
    DateTime::from_timestamp(ts, 0)
        .map(|dt: DateTime<Utc>| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_new_cache() {
        let dir = tempdir().unwrap();
        let cache = PackageCache::new(dir.path().to_path_buf(), 100).unwrap();
        assert_eq!(cache.stats.total_entries, 0);
    }

    #[test]
    fn test_add_and_get() {
        let dir = tempdir().unwrap();
        let mut cache = PackageCache::new(dir.path().to_path_buf(), 100).unwrap();
        
        // Create test file
        let test_file = dir.path().join("test.pkg.tar");
        let mut file = fs::File::create(&test_file).unwrap();
        file.write_all(b"test package content").unwrap();
        
        // Add to cache
        cache.add(&test_file, "test", "1.0.0").unwrap();
        assert_eq!(cache.stats.total_entries, 1);
        
        // Get from cache
        let result = cache.get_package("test", "1.0.0");
        assert!(result.is_some());
        assert_eq!(cache.stats.cache_hits, 1);
    }

    #[test]
    fn test_clear() {
        let dir = tempdir().unwrap();
        let mut cache = PackageCache::new(dir.path().to_path_buf(), 100).unwrap();
        
        let test_file = dir.path().join("test.pkg.tar");
        fs::write(&test_file, b"test").unwrap();
        cache.add(&test_file, "test", "1.0.0").unwrap();
        
        let result = cache.clear().unwrap();
        assert_eq!(result.removed_count, 1);
        assert_eq!(cache.stats.total_entries, 0);
    }
}
