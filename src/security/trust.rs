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

//! Maintainer trust scoring system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trust level for a maintainer
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    /// Unknown maintainer
    Unknown,
    /// Suspicious activity detected
    Suspicious,
    /// Verified by basic checks
    Verified,
    /// Trusted maintainer (long history, many packages)
    Trusted,
}

impl TrustLevel {
    /// Convert from numerical score
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=20 => TrustLevel::Suspicious,
            21..=50 => TrustLevel::Unknown,
            51..=80 => TrustLevel::Verified,
            _ => TrustLevel::Trusted,
        }
    }
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::Unknown => write!(f, "Unknown"),
            TrustLevel::Suspicious => write!(f, "Suspicious"),
            TrustLevel::Verified => write!(f, "Verified"),
            TrustLevel::Trusted => write!(f, "Trusted"),
        }
    }
}

/// Maintainer information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaintainerInfo {
    pub username: String,
    pub packages_count: u32,
    pub total_votes: u32,
    pub average_popularity: f64,
    pub first_submit: Option<String>,
    pub last_active: Option<String>,
    pub orphaned_packages: u32,
    pub flagged_packages: u32,
    pub trusted_user: bool,
}

/// Trust scorer
pub struct TrustScorer {
    /// Cache of maintainer scores
    cache: HashMap<String, (MaintainerInfo, u32)>,
    client: reqwest::Client,
}

impl TrustScorer {
    /// Create a new trust scorer
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            client: reqwest::Client::new(),
        }
    }

    /// Get maintainer information from AUR
    pub async fn get_maintainer_info(&self, username: &str) -> MaintainerInfo {
        // Try to fetch from AUR
        if let Ok(info) = self.fetch_from_aur(username).await {
            return info;
        }

        // Return default for unknown maintainer
        MaintainerInfo {
            username: username.to_string(),
            ..Default::default()
        }
    }

    /// Fetch maintainer info from AUR API
    async fn fetch_from_aur(&self, username: &str) -> anyhow::Result<MaintainerInfo> {
        let url = format!(
            "https://aur.archlinux.org/rpc/v5/search/{}&by=maintainer",
            urlencoding::encode(username)
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Ok(MaintainerInfo {
                username: username.to_string(),
                ..Default::default()
            });
        }

        let data: AurSearchResponse = response.json().await?;

        let packages_count = data.resultcount as u32;
        let total_votes: u32 = data.results.iter().map(|p| p.num_votes as u32).sum();
        let average_popularity: f64 = if packages_count > 0 {
            data.results.iter().map(|p| p.popularity).sum::<f64>() / packages_count as f64
        } else {
            0.0
        };

        let flagged_packages = data
            .results
            .iter()
            .filter(|p| p.out_of_date.is_some())
            .count() as u32;

        Ok(MaintainerInfo {
            username: username.to_string(),
            packages_count,
            total_votes,
            average_popularity,
            first_submit: data
                .results
                .iter()
                .filter_map(|p| p.first_submitted)
                .min()
                .map(format_timestamp),
            last_active: data
                .results
                .iter()
                .filter_map(|p| p.last_modified)
                .max()
                .map(format_timestamp),
            orphaned_packages: 0,
            flagged_packages,
            trusted_user: false, // Would need to check against TU list
        })
    }

    /// Calculate trust score (0-100)
    pub fn calculate_score(&self, info: &MaintainerInfo) -> u32 {
        let mut score: i32 = 50; // Start at neutral

        // Trusted User bonus
        if info.trusted_user {
            score += 40;
        }

        // Package count bonus (max +15)
        score += (info.packages_count.min(15) as i32);

        // Vote bonus (max +15)
        let vote_bonus = (info.total_votes as f64 / 10.0).min(15.0) as i32;
        score += vote_bonus;

        // Popularity bonus (max +10)
        let pop_bonus = (info.average_popularity * 5.0).min(10.0) as i32;
        score += pop_bonus;

        // Account age bonus (would need to parse first_submit)
        // For now, assume some bonus if first_submit exists
        if info.first_submit.is_some() {
            score += 5;
        }

        // Penalties
        // Orphaned packages penalty
        score -= (info.orphaned_packages * 5) as i32;

        // Flagged packages penalty
        score -= (info.flagged_packages * 3) as i32;

        // Unknown maintainer penalty
        if info.packages_count == 0 {
            score -= 20;
        }

        score.clamp(0, 100) as u32
    }

    /// Get trust level for maintainer
    pub async fn get_trust_level(&self, username: &str) -> TrustLevel {
        let info = self.get_maintainer_info(username).await;
        let score = self.calculate_score(&info);
        TrustLevel::from_score(score)
    }
}

impl Default for TrustScorer {
    fn default() -> Self {
        Self::new()
    }
}

/// AUR search response
#[derive(Debug, Deserialize)]
struct AurSearchResponse {
    #[serde(default)]
    resultcount: usize,
    #[serde(default)]
    results: Vec<AurPackage>,
}

#[derive(Debug, Deserialize)]
struct AurPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "NumVotes", default)]
    num_votes: i32,
    #[serde(rename = "Popularity", default)]
    popularity: f64,
    #[serde(rename = "OutOfDate")]
    out_of_date: Option<i64>,
    #[serde(rename = "FirstSubmitted")]
    first_submitted: Option<i64>,
    #[serde(rename = "LastModified")]
    last_modified: Option<i64>,
}

/// Format Unix timestamp to date string
fn format_timestamp(ts: i64) -> String {
    use chrono::{DateTime, Utc};
    DateTime::from_timestamp(ts, 0)
        .map(|dt: DateTime<Utc>| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Display maintainer trust info
pub fn display_trust_info(info: &MaintainerInfo, score: u32) {
    use console::style;

    let level = TrustLevel::from_score(score);
    let level_style = match level {
        TrustLevel::Trusted => style(level.to_string()).green().bold(),
        TrustLevel::Verified => style(level.to_string()).cyan(),
        TrustLevel::Unknown => style(level.to_string()).yellow(),
        TrustLevel::Suspicious => style(level.to_string()).red().bold(),
    };

    println!();
    println!(
        "{} Maintainer: {}",
        style("::").cyan().bold(),
        style(&info.username).white().bold()
    );
    println!("   Trust Level: {} (score: {})", level_style, score);
    println!("   Packages: {}", info.packages_count);
    println!("   Total Votes: {}", info.total_votes);
    println!("   Average Popularity: {:.2}", info.average_popularity);

    if let Some(ref first) = info.first_submit {
        println!("   First Submit: {}", first);
    }
    if let Some(ref last) = info.last_active {
        println!("   Last Active: {}", last);
    }

    if info.flagged_packages > 0 {
        println!(
            "   {} packages flagged out-of-date",
            style(info.flagged_packages).yellow()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_level_from_score() {
        assert_eq!(TrustLevel::from_score(0), TrustLevel::Suspicious);
        assert_eq!(TrustLevel::from_score(30), TrustLevel::Unknown);
        assert_eq!(TrustLevel::from_score(60), TrustLevel::Verified);
        assert_eq!(TrustLevel::from_score(90), TrustLevel::Trusted);
    }

    #[test]
    fn test_calculate_score() {
        let scorer = TrustScorer::new();

        // Unknown maintainer
        let info = MaintainerInfo::default();
        let score = scorer.calculate_score(&info);
        assert!(score < 50);

        // Active maintainer
        let info = MaintainerInfo {
            username: "test".to_string(),
            packages_count: 10,
            total_votes: 100,
            average_popularity: 1.0,
            first_submit: Some("2020-01-01".to_string()),
            ..Default::default()
        };
        let score = scorer.calculate_score(&info);
        assert!(score > 50);

        // Trusted User
        let info = MaintainerInfo {
            username: "tu".to_string(),
            trusted_user: true,
            packages_count: 50,
            ..Default::default()
        };
        let score = scorer.calculate_score(&info);
        assert!(score > 80);
    }

    #[test]
    fn test_trust_scorer_new() {
        let scorer = TrustScorer::new();
        assert!(scorer.cache.is_empty());
    }
}
