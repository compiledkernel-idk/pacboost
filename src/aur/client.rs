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

//! AUR RPC API client with caching and rate limiting.

use anyhow::{anyhow, Result};
use lru::LruCache;
use serde::Deserialize;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::AurRpcResponse;

/// AUR package information from RPC API
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AurPackageInfo {
    #[serde(rename = "ID")]
    pub id: u64,
    pub name: String,
    pub package_base: String,
    #[serde(rename = "PackageBaseID")]
    pub package_base_id: u64,
    pub version: String,
    pub description: Option<String>,
    #[serde(rename = "URL")]
    pub url: Option<String>,
    pub num_votes: u32,
    pub popularity: f64,
    pub out_of_date: Option<u64>,
    pub maintainer: Option<String>,
    pub submitter: Option<String>,
    pub first_submitted: u64,
    pub last_modified: u64,
    #[serde(rename = "URLPath")]
    pub url_path: String,

    // Dependencies
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(default)]
    pub make_depends: Vec<String>,
    #[serde(default)]
    pub opt_depends: Vec<String>,
    #[serde(default)]
    pub check_depends: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub replaces: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub license: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl AurPackageInfo {
    /// Get all dependencies (depends + makedepends)
    pub fn all_deps(&self) -> Vec<String> {
        let mut deps = self.depends.clone();
        deps.extend(self.make_depends.clone());
        deps
    }

    /// Get the snapshot download URL
    pub fn snapshot_url(&self) -> String {
        format!(
            "https://aur.archlinux.org/cgit/aur.git/snapshot/{}.tar.gz",
            self.package_base
        )
    }
}

/// High-performance AUR RPC client with caching
pub struct AurClient {
    client: reqwest::Client,
    cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
    base_url: String,
    last_request: Arc<RwLock<Instant>>,
    min_request_interval: Duration,
}

#[derive(Clone)]
struct CacheEntry {
    info: AurPackageInfo,
    cached_at: Instant,
}

impl AurClient {
    /// Create a new AUR client with default settings
    pub fn new() -> Self {
        Self::with_config("https://aur.archlinux.org/rpc/".to_string(), 500)
    }

    /// Create a new AUR client with custom settings
    pub fn with_config(base_url: String, cache_size: usize) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(5)
            .user_agent("pacboost/1.3.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(cache_size).unwrap(),
            ))),
            base_url,
            last_request: Arc::new(RwLock::new(Instant::now())),
            min_request_interval: Duration::from_millis(100), // Rate limiting
        }
    }

    /// Rate limit requests to avoid hammering AUR
    async fn rate_limit(&self) {
        let mut last = self.last_request.write().await;
        let elapsed = last.elapsed();
        if elapsed < self.min_request_interval {
            tokio::time::sleep(self.min_request_interval - elapsed).await;
        }
        *last = Instant::now();
    }

    /// Get package info, using cache if available
    pub async fn get_info(&self, name: &str) -> Result<AurPackageInfo> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.peek(name) {
                // Cache entries valid for 5 minutes
                if entry.cached_at.elapsed() < Duration::from_secs(300) {
                    return Ok(entry.info.clone());
                }
            }
        }

        // Fetch from API
        let results = self.get_info_batch(&[name.to_string()]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Package '{}' not found in AUR", name))
    }

    /// Batch fetch multiple packages (up to 250 per request)
    pub async fn get_info_batch(&self, names: &[String]) -> Result<Vec<AurPackageInfo>> {
        if names.is_empty() {
            return Ok(vec![]);
        }

        // Check cache for already-known packages
        let mut cached = Vec::new();
        let mut to_fetch = Vec::new();

        {
            let cache = self.cache.read().await;
            for name in names {
                if let Some(entry) = cache.peek(name) {
                    if entry.cached_at.elapsed() < Duration::from_secs(300) {
                        cached.push(entry.info.clone());
                        continue;
                    }
                }
                to_fetch.push(name.clone());
            }
        }

        if to_fetch.is_empty() {
            return Ok(cached);
        }

        // AUR RPC supports up to 250 packages per request
        const BATCH_SIZE: usize = 250;
        let mut all_results = cached;

        for chunk in to_fetch.chunks(BATCH_SIZE) {
            self.rate_limit().await;

            let args: Vec<String> = chunk
                .iter()
                .map(|n| format!("arg[]={}", urlencoding::encode(n)))
                .collect();

            let url = format!("{}?v=5&type=info&{}", self.base_url, args.join("&"));

            let response: AurRpcResponse = self.client.get(&url).send().await?.json().await?;

            if let Some(error) = response.error {
                return Err(anyhow!("AUR RPC error: {}", error));
            }

            // Update cache
            {
                let mut cache = self.cache.write().await;
                for pkg in &response.results {
                    cache.put(
                        pkg.name.clone(),
                        CacheEntry {
                            info: pkg.clone(),
                            cached_at: Instant::now(),
                        },
                    );
                }
            }

            all_results.extend(response.results);
        }

        Ok(all_results)
    }

    /// Search for packages by keyword
    pub async fn search(&self, query: &str) -> Result<Vec<AurPackageInfo>> {
        self.rate_limit().await;

        let url = format!(
            "{}?v=5&type=search&arg={}",
            self.base_url,
            urlencoding::encode(query)
        );

        let response: AurRpcResponse = self.client.get(&url).send().await?.json().await?;

        if let Some(error) = response.error {
            return Err(anyhow!("AUR RPC error: {}", error));
        }

        Ok(response.results)
    }

    /// Search with field specifier (name, name-desc, maintainer, depends, makedepends, optdepends, checkdepends)
    pub async fn search_by(&self, field: &str, query: &str) -> Result<Vec<AurPackageInfo>> {
        self.rate_limit().await;

        let url = format!(
            "{}?v=5&type=search&by={}&arg={}",
            self.base_url,
            field,
            urlencoding::encode(query)
        );

        let response: AurRpcResponse = self.client.get(&url).send().await?.json().await?;

        if let Some(error) = response.error {
            return Err(anyhow!("AUR RPC error: {}", error));
        }

        Ok(response.results)
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        (cache.len(), cache.cap().get())
    }
}

impl Default for AurClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse dependency string into name and optional version constraint
pub fn parse_dependency(dep: &str) -> (String, Option<String>) {
    // Dependencies can be in format: name, name>=version, name=version, etc.
    let dep = dep.trim();

    // Check for version operators
    for op in &[">=", "<=", "=", ">", "<"] {
        if let Some(pos) = dep.find(op) {
            let name = dep[..pos].to_string();
            let version = dep[pos..].to_string();
            return (name, Some(version));
        }
    }

    // Check for provides (: separator)
    if let Some(pos) = dep.find(':') {
        let name = dep[..pos].to_string();
        return (name, None);
    }

    (dep.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dependency() {
        let (name, version) = parse_dependency("gcc");
        assert_eq!(name, "gcc");
        assert!(version.is_none());

        let (name, version) = parse_dependency("python>=3.10");
        assert_eq!(name, "python");
        assert_eq!(version.unwrap(), ">=3.10");

        let (name, version) = parse_dependency("rust=1.70.0");
        assert_eq!(name, "rust");
        assert_eq!(version.unwrap(), "=1.70.0");
    }

    #[test]
    fn test_snapshot_url() {
        let info = AurPackageInfo {
            id: 1,
            name: "test-pkg".to_string(),
            package_base: "test-pkg-base".to_string(),
            package_base_id: 1,
            version: "1.0.0".to_string(),
            description: None,
            url: None,
            num_votes: 0,
            popularity: 0.0,
            out_of_date: None,
            maintainer: None,
            submitter: None,
            first_submitted: 0,
            last_modified: 0,
            url_path: "".to_string(),
            depends: vec![],
            make_depends: vec![],
            opt_depends: vec![],
            check_depends: vec![],
            conflicts: vec![],
            provides: vec![],
            replaces: vec![],
            groups: vec![],
            license: vec![],
            keywords: vec![],
        };

        assert_eq!(
            info.snapshot_url(),
            "https://aur.archlinux.org/cgit/aur.git/snapshot/test-pkg-base.tar.gz"
        );
    }
}
