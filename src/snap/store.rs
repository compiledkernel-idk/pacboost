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

//! Snap Store API client.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const SNAP_STORE_API: &str = "https://api.snapcraft.io/v2";

/// Snap Store API client
pub struct SnapStore {
    client: reqwest::Client,
}

/// Store search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSnap {
    pub name: String,
    pub snap_id: String,
    pub publisher: Publisher,
    pub summary: String,
    pub description: Option<String>,
    pub version: String,
    pub channel: String,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub media: Vec<Media>,
    pub ratings_average: Option<f64>,
    pub total_ratings: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publisher {
    pub display_name: String,
    pub username: String,
    #[serde(default)]
    pub validation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    #[serde(rename = "type")]
    pub media_type: String,
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    snap: SnapDetails,
}

#[derive(Debug, Deserialize)]
struct SnapDetails {
    name: String,
    snap_id: String,
    publisher: Publisher,
    summary: String,
    #[serde(default)]
    categories: Vec<Category>,
}

impl SnapStore {
    /// Create a new Snap Store client
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Search for snaps in the store
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<StoreSnap>> {
        let url = format!(
            "{}/snaps/find?q={}&fields=snap_id,name,publisher,summary,version,categories",
            SNAP_STORE_API,
            urlencoding::encode(query)
        );

        let response = self
            .client
            .get(&url)
            .header("Snap-Device-Series", "16")
            .header("Snap-Device-Architecture", "amd64")
            .send()
            .await
            .context("Failed to search Snap Store")?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let results: SearchResponse = response
            .json()
            .await
            .context("Failed to parse search results")?;

        Ok(results
            .results
            .into_iter()
            .take(limit)
            .map(|r| StoreSnap {
                name: r.snap.name,
                snap_id: r.snap.snap_id,
                publisher: r.snap.publisher,
                summary: r.snap.summary,
                description: None,
                version: String::new(),
                channel: "stable".to_string(),
                categories: r.snap.categories,
                media: Vec::new(),
                ratings_average: None,
                total_ratings: None,
            })
            .collect())
    }

    /// Get detailed info about a snap
    pub async fn info(&self, name: &str) -> Result<Option<StoreSnap>> {
        let url = format!(
            "{}/snaps/info/{}",
            SNAP_STORE_API,
            urlencoding::encode(name)
        );

        let response = self
            .client
            .get(&url)
            .header("Snap-Device-Series", "16")
            .header("Snap-Device-Architecture", "amd64")
            .send()
            .await
            .context("Failed to get snap info")?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Ok(None);
        }

        let snap: StoreSnap = response.json().await.context("Failed to parse snap info")?;

        Ok(Some(snap))
    }

    /// Get featured snaps
    pub async fn featured(&self, limit: usize) -> Result<Vec<StoreSnap>> {
        self.search("*", limit).await
    }

    /// Get snaps by category
    pub async fn by_category(&self, category: &str, limit: usize) -> Result<Vec<StoreSnap>> {
        let url = format!(
            "{}/snaps/find?category={}&fields=snap_id,name,publisher,summary,version",
            SNAP_STORE_API,
            urlencoding::encode(category)
        );

        let response = self
            .client
            .get(&url)
            .header("Snap-Device-Series", "16")
            .header("Snap-Device-Architecture", "amd64")
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let results: SearchResponse = response.json().await?;

        Ok(results
            .results
            .into_iter()
            .take(limit)
            .map(|r| StoreSnap {
                name: r.snap.name,
                snap_id: r.snap.snap_id,
                publisher: r.snap.publisher,
                summary: r.snap.summary,
                description: None,
                version: String::new(),
                channel: "stable".to_string(),
                categories: r.snap.categories,
                media: Vec::new(),
                ratings_average: None,
                total_ratings: None,
            })
            .collect())
    }
}

impl Default for SnapStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_store_new() {
        let store = SnapStore::new();
        // Just ensure it creates successfully
        drop(store);
    }
}
