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

//! CVE database integration.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const ARCH_SECURITY_URL: &str = "https://security.archlinux.org/issues/all.json";

/// Known vulnerability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub cve_id: String,
    pub packages: Vec<String>,
    pub severity: VulnerabilitySeverity,
    pub description: String,
    pub fixed_version: Option<String>,
    pub status: VulnerabilityStatus,
    pub published: Option<String>,
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VulnerabilitySeverity {
    Critical,
    High,
    Medium,
    Low,
    Unknown,
}

impl std::fmt::Display for VulnerabilitySeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VulnerabilitySeverity::Critical => write!(f, "Critical"),
            VulnerabilitySeverity::High => write!(f, "High"),
            VulnerabilitySeverity::Medium => write!(f, "Medium"),
            VulnerabilitySeverity::Low => write!(f, "Low"),
            VulnerabilitySeverity::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Vulnerability status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VulnerabilityStatus {
    Vulnerable,
    Fixed,
    NotAffected,
    Unknown,
}

/// CVE checker with caching
pub struct CveChecker {
    cache: HashMap<String, Vec<Vulnerability>>,
    client: reqwest::Client,
}

impl CveChecker {
    /// Create a new CVE checker
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            client: reqwest::Client::new(),
        }
    }

    /// Check a package for known vulnerabilities
    pub async fn check_package(&self, package: &str, version: &str) -> Result<Vec<Vulnerability>> {
        let issues = self.fetch_arch_security().await?;

        let mut vulnerabilities = Vec::new();

        for issue in issues {
            if issue.packages.iter().any(|p| p == package) {
                // Check if the vulnerability affects this version
                let status = if let Some(ref fixed) = issue.fixed_version {
                    if version_gte(version, fixed) {
                        VulnerabilityStatus::Fixed
                    } else {
                        VulnerabilityStatus::Vulnerable
                    }
                } else {
                    VulnerabilityStatus::Vulnerable
                };

                if status == VulnerabilityStatus::Vulnerable {
                    vulnerabilities.push(Vulnerability {
                        status,
                        ..issue.clone()
                    });
                }
            }
        }

        Ok(vulnerabilities)
    }

    /// Check multiple packages
    pub async fn check_packages(
        &self,
        packages: &[(String, String)],
    ) -> Result<HashMap<String, Vec<Vulnerability>>> {
        let issues = self.fetch_arch_security().await?;
        let mut result = HashMap::new();

        for (package, version) in packages {
            let vulns: Vec<_> = issues
                .iter()
                .filter(|issue| issue.packages.iter().any(|p| p == package))
                .filter(|issue| {
                    if let Some(ref fixed) = issue.fixed_version {
                        !version_gte(version, fixed)
                    } else {
                        true
                    }
                })
                .cloned()
                .collect();

            if !vulns.is_empty() {
                result.insert(package.clone(), vulns);
            }
        }

        Ok(result)
    }

    /// Fetch Arch Linux security advisories
    async fn fetch_arch_security(&self) -> Result<Vec<Vulnerability>> {
        let response = self
            .client
            .get(ARCH_SECURITY_URL)
            .send()
            .await
            .context("Failed to fetch Arch security data")?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: Vec<ArchSecurityIssue> = response
            .json()
            .await
            .context("Failed to parse security data")?;

        Ok(data
            .into_iter()
            .map(|issue| {
                let severity = match issue.severity.as_deref() {
                    Some("Critical") => VulnerabilitySeverity::Critical,
                    Some("High") => VulnerabilitySeverity::High,
                    Some("Medium") => VulnerabilitySeverity::Medium,
                    Some("Low") => VulnerabilitySeverity::Low,
                    _ => VulnerabilitySeverity::Unknown,
                };

                Vulnerability {
                    cve_id: issue.cve.clone().unwrap_or_else(|| issue.name.clone()),
                    packages: issue.packages,
                    severity,
                    description: issue.description.unwrap_or_default(),
                    fixed_version: issue.fixed,
                    status: VulnerabilityStatus::Unknown,
                    published: issue.created,
                }
            })
            .collect())
    }

    /// Get a summary of vulnerabilities
    pub fn summarize(vulns: &[Vulnerability]) -> VulnerabilitySummary {
        let mut summary = VulnerabilitySummary::default();

        for vuln in vulns {
            match vuln.severity {
                VulnerabilitySeverity::Critical => summary.critical += 1,
                VulnerabilitySeverity::High => summary.high += 1,
                VulnerabilitySeverity::Medium => summary.medium += 1,
                VulnerabilitySeverity::Low => summary.low += 1,
                VulnerabilitySeverity::Unknown => summary.unknown += 1,
            }
        }

        summary.total = vulns.len();
        summary
    }
}

impl Default for CveChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Arch Security API response format
#[derive(Debug, Deserialize)]
struct ArchSecurityIssue {
    name: String,
    #[serde(default)]
    packages: Vec<String>,
    severity: Option<String>,
    #[serde(rename = "type")]
    issue_type: Option<String>,
    cve: Option<String>,
    description: Option<String>,
    fixed: Option<String>,
    created: Option<String>,
}

/// Summary of vulnerabilities
#[derive(Debug, Clone, Default)]
pub struct VulnerabilitySummary {
    pub total: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub unknown: usize,
}

impl VulnerabilitySummary {
    pub fn has_critical(&self) -> bool {
        self.critical > 0
    }

    pub fn has_high(&self) -> bool {
        self.high > 0
    }
}

/// Simple version comparison (>= check)
fn version_gte(a: &str, b: &str) -> bool {
    // Simplified version comparison
    // In reality, would use alpm version comparison
    let a_parts: Vec<u32> = a
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect();
    let b_parts: Vec<u32> = b
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect();

    for i in 0..a_parts.len().max(b_parts.len()) {
        let av = a_parts.get(i).copied().unwrap_or(0);
        let bv = b_parts.get(i).copied().unwrap_or(0);
        if av > bv {
            return true;
        } else if av < bv {
            return false;
        }
    }
    true
}

/// Display vulnerabilities
pub fn display_vulnerabilities(vulns: &[Vulnerability]) {
    use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
    use console::style;

    if vulns.is_empty() {
        println!("{} No known vulnerabilities", style("✓").green().bold());
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("CVE").fg(Color::Cyan),
        Cell::new("Severity").fg(Color::Cyan),
        Cell::new("Packages").fg(Color::Cyan),
        Cell::new("Description").fg(Color::Cyan),
    ]);

    for vuln in vulns {
        let sev_color = match vuln.severity {
            VulnerabilitySeverity::Critical => Color::Red,
            VulnerabilitySeverity::High => Color::Red,
            VulnerabilitySeverity::Medium => Color::Yellow,
            VulnerabilitySeverity::Low => Color::Blue,
            VulnerabilitySeverity::Unknown => Color::DarkGrey,
        };

        let desc = if vuln.description.len() > 50 {
            format!("{}...", &vuln.description[..47])
        } else {
            vuln.description.clone()
        };

        table.add_row(vec![
            Cell::new(&vuln.cve_id).fg(Color::Yellow),
            Cell::new(vuln.severity.to_string()).fg(sev_color),
            Cell::new(vuln.packages.join(", ")).fg(Color::White),
            Cell::new(&desc).fg(Color::DarkGrey),
        ]);
    }

    println!("{}", table);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_gte() {
        assert!(version_gte("2.0.0", "1.0.0"));
        assert!(version_gte("1.0.0", "1.0.0"));
        assert!(!version_gte("1.0.0", "2.0.0"));
        assert!(version_gte("1.2.3", "1.2.2"));
    }

    #[test]
    fn test_cve_checker_new() {
        let checker = CveChecker::new();
        assert!(checker.cache.is_empty());
    }

    #[test]
    fn test_vulnerability_summary() {
        let vulns = vec![
            Vulnerability {
                cve_id: "CVE-2024-0001".to_string(),
                packages: vec!["test".to_string()],
                severity: VulnerabilitySeverity::Critical,
                description: "Test".to_string(),
                fixed_version: None,
                status: VulnerabilityStatus::Vulnerable,
                published: None,
            },
            Vulnerability {
                cve_id: "CVE-2024-0002".to_string(),
                packages: vec!["test".to_string()],
                severity: VulnerabilitySeverity::High,
                description: "Test".to_string(),
                fixed_version: None,
                status: VulnerabilityStatus::Vulnerable,
                published: None,
            },
        ];

        let summary = CveChecker::summarize(&vulns);
        assert_eq!(summary.total, 2);
        assert_eq!(summary.critical, 1);
        assert_eq!(summary.high, 1);
        assert!(summary.has_critical());
    }
}
