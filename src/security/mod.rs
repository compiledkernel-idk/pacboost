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

//! Security module for pacboost.
//!
//! Provides comprehensive security features:
//! - Advanced malware detection
//! - Sandboxed execution
//! - CVE database integration
//! - Trust scoring

pub mod malware;
pub mod sandbox;
pub mod cve;
pub mod trust;

use anyhow::Result;
use console::style;
use std::path::Path;

pub use malware::{MalwareDetector, MalwareReport, ThreatLevel};
pub use sandbox::{Sandbox, SandboxConfig};
pub use cve::{CveChecker, Vulnerability};
pub use trust::{TrustScorer, TrustLevel, MaintainerInfo};

/// Unified security manager
pub struct SecurityManager {
    malware_detector: MalwareDetector,
    cve_checker: CveChecker,
    trust_scorer: TrustScorer,
    config: SecurityConfig,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable malware scanning
    pub enable_malware_scan: bool,
    /// Enable CVE checking
    pub enable_cve_check: bool,
    /// Enable trust scoring
    pub enable_trust_check: bool,
    /// Minimum trust score to proceed
    pub min_trust_score: u32,
    /// Block on critical threats
    pub block_critical: bool,
    /// Require confirmation for high threats
    pub confirm_high: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_malware_scan: true,
            enable_cve_check: true,
            enable_trust_check: true,
            min_trust_score: 30,
            block_critical: true,
            confirm_high: true,
        }
    }
}

/// Combined security report
#[derive(Debug, Clone)]
pub struct SecurityReport {
    pub malware: Option<MalwareReport>,
    pub vulnerabilities: Vec<Vulnerability>,
    pub trust_level: TrustLevel,
    pub trust_score: u32,
    pub overall_safe: bool,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

impl SecurityManager {
    /// Create a new security manager with default configuration
    pub fn new() -> Self {
        Self::with_config(SecurityConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: SecurityConfig) -> Self {
        Self {
            malware_detector: MalwareDetector::new(),
            cve_checker: CveChecker::new(),
            trust_scorer: TrustScorer::new(),
            config,
        }
    }

    /// Perform a comprehensive security scan on a PKGBUILD
    pub async fn scan_pkgbuild(&self, content: &str, maintainer: Option<&str>) -> Result<SecurityReport> {
        let mut warnings = Vec::new();
        let mut blockers = Vec::new();

        // Malware scan
        let malware_report = if self.config.enable_malware_scan {
            let report = self.malware_detector.scan(content);
            
            if report.threat_level == ThreatLevel::Critical {
                blockers.push(format!("Critical malware threat detected: {} issues", report.threats.len()));
            } else if report.threat_level == ThreatLevel::High {
                warnings.push(format!("High-severity security issues: {} threats", report.threats.len()));
            }
            
            Some(report)
        } else {
            None
        };

        // Trust scoring
        let (trust_level, trust_score) = if self.config.enable_trust_check {
            if let Some(maintainer) = maintainer {
                let info = self.trust_scorer.get_maintainer_info(maintainer).await;
                let score = self.trust_scorer.calculate_score(&info);
                let level = TrustLevel::from_score(score);
                
                if score < self.config.min_trust_score {
                    warnings.push(format!("Low trust score: {} (minimum: {})", score, self.config.min_trust_score));
                }
                
                (level, score)
            } else {
                warnings.push("Unknown maintainer - cannot verify trust".to_string());
                (TrustLevel::Unknown, 0)
            }
        } else {
            (TrustLevel::Unknown, 0)
        };

        // Determine overall safety
        let overall_safe = blockers.is_empty() && 
            (trust_score >= self.config.min_trust_score || !self.config.enable_trust_check);

        Ok(SecurityReport {
            malware: malware_report,
            vulnerabilities: Vec::new(), // CVE check done separately for packages
            trust_level,
            trust_score,
            overall_safe,
            warnings,
            blockers,
        })
    }

    /// Check a package for known vulnerabilities
    pub async fn check_package_cve(&self, package_name: &str, version: &str) -> Result<Vec<Vulnerability>> {
        if !self.config.enable_cve_check {
            return Ok(Vec::new());
        }

        self.cve_checker.check_package(package_name, version).await
    }

    /// Display security report
    pub fn display_report(&self, report: &SecurityReport) {
        println!();
        println!("{} {}", 
            style("::").cyan().bold(),
            style("Security Report").white().bold());
        
        // Malware results
        if let Some(ref malware) = report.malware {
            let threat_style = match malware.threat_level {
                ThreatLevel::Critical => style("CRITICAL").red().bold(),
                ThreatLevel::High => style("HIGH").red(),
                ThreatLevel::Medium => style("MEDIUM").yellow(),
                ThreatLevel::Low => style("LOW").blue(),
                ThreatLevel::None => style("NONE").green(),
            };
            
            println!("   Malware Scan: {} ({} threats)", threat_style, malware.threats.len());
            
            for threat in &malware.threats {
                println!("      - {}", style(&threat.description).dim());
            }
        }

        // Trust info
        let trust_style = match report.trust_level {
            TrustLevel::Trusted => style("Trusted").green(),
            TrustLevel::Verified => style("Verified").cyan(),
            TrustLevel::Unknown => style("Unknown").yellow(),
            TrustLevel::Suspicious => style("Suspicious").red(),
        };
        println!("   Trust Level: {} (score: {})", trust_style, report.trust_score);

        // Warnings
        for warning in &report.warnings {
            println!("   {} {}", style("⚠").yellow(), warning);
        }

        // Blockers
        for blocker in &report.blockers {
            println!("   {} {}", style("✗").red().bold(), blocker);
        }

        // Overall status
        if report.overall_safe {
            println!("   {} {}", 
                style("✓").green().bold(),
                style("Package passed security checks").green());
        } else {
            println!("   {} {}", 
                style("✗").red().bold(),
                style("Package FAILED security checks").red().bold());
        }
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.enable_malware_scan);
        assert!(config.block_critical);
    }

    #[test]
    fn test_security_manager_new() {
        let manager = SecurityManager::new();
        assert!(manager.config.enable_malware_scan);
    }
}
