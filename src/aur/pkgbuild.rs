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

//! PKGBUILD parsing and security validation.

use anyhow::Result;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::fs;

use crate::error::SecuritySeverity;

/// Parsed PKGBUILD metadata
#[derive(Debug, Clone, Default)]
pub struct Pkgbuild {
    pub pkgname: Vec<String>,
    pub pkgbase: String,
    pub pkgver: String,
    pub pkgrel: String,
    pub epoch: Option<u32>,
    pub pkgdesc: String,
    pub arch: Vec<String>,
    pub url: String,
    pub license: Vec<String>,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    pub checkdepends: Vec<String>,
    pub optdepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub source: Vec<String>,
    pub sha256sums: Vec<String>,
    pub sha512sums: Vec<String>,
    pub md5sums: Vec<String>,
    pub validpgpkeys: Vec<String>,
    pub backup: Vec<String>,
    pub options: Vec<String>,
    pub install: Option<String>,
    
    /// Raw PKGBUILD content
    pub raw_content: String,
}

/// Security issue found in PKGBUILD
#[derive(Debug, Clone)]
pub struct SecurityIssue {
    pub severity: SecuritySeverity,
    pub description: String,
    pub line: Option<usize>,
    pub pattern: String,
}

/// Security validation report
#[derive(Debug, Clone, Default)]
pub struct SecurityReport {
    pub issues: Vec<SecurityIssue>,
    pub score: u32,  // 0-100, higher is safer
    pub passed: bool,
}

impl SecurityReport {
    /// Check if report has critical issues
    pub fn has_critical(&self) -> bool {
        self.issues.iter()
            .any(|i| i.severity == SecuritySeverity::Critical)
    }
    
    /// Check if report has high severity issues
    pub fn has_high(&self) -> bool {
        self.issues.iter()
            .any(|i| i.severity == SecuritySeverity::High)
    }
}

/// Security validator for PKGBUILD files
pub struct SecurityValidator {
    /// Patterns that indicate definitely malicious code
    critical_patterns: Vec<(Regex, &'static str)>,
    /// Patterns that are highly suspicious
    high_patterns: Vec<(Regex, &'static str)>,
    /// Patterns that should be reviewed
    medium_patterns: Vec<(Regex, &'static str)>,
    /// Patterns that are just informational
    low_patterns: Vec<(Regex, &'static str)>,
    /// Suspicious commands
    suspicious_commands: HashSet<&'static str>,
}

impl SecurityValidator {
    /// Create a new security validator with default patterns
    pub fn new() -> Self {
        Self {
            critical_patterns: vec![
                // rm -rf / followed by anything that's not a variable (simplified pattern)
                (Regex::new(r"rm\s+-rf\s+/[a-zA-Z]").unwrap(), "Destructive rm -rf on root filesystem"),
                (Regex::new(r":\s*\(\s*\)\s*\{").unwrap(), "Potential fork bomb pattern"),
                (Regex::new(r"eval\s+.*\$\(curl").unwrap(), "Remote code execution via eval+curl"),
                (Regex::new(r"eval\s+.*\$\(wget").unwrap(), "Remote code execution via eval+wget"),
                (Regex::new(r"/dev/sd[a-z]").unwrap(), "Direct disk device access"),
                (Regex::new(r"mkfs\s").unwrap(), "Filesystem formatting command"),
                (Regex::new(r"dd\s+if=.*of=/dev/").unwrap(), "Direct disk write with dd"),
            ],
            high_patterns: vec![
                (Regex::new(r"curl\s+.*\|\s*(ba)?sh").unwrap(), "Piping curl to shell"),
                (Regex::new(r"wget\s+.*\|\s*(ba)?sh").unwrap(), "Piping wget to shell"),
                (Regex::new(r"/etc/shadow").unwrap(), "Access to shadow password file"),
                (Regex::new(r"/etc/passwd").unwrap(), "Access to passwd file"),
                (Regex::new(r"~/.ssh").unwrap(), "Access to SSH directory"),
                (Regex::new(r"\.gnupg").unwrap(), "Access to GPG directory"),
                (Regex::new(r"chmod\s+777").unwrap(), "World-writable permissions"),
                (Regex::new(r"setuid|setgid").unwrap(), "SUID/SGID modification"),
                (Regex::new(r"nc\s+-[el]").unwrap(), "Netcat listener (potential backdoor)"),
                (Regex::new(r"ncat\s.*-[el]").unwrap(), "Ncat listener (potential backdoor)"),
            ],
            medium_patterns: vec![
                (Regex::new(r"curl\s").unwrap(), "Network access via curl"),
                (Regex::new(r"wget\s").unwrap(), "Network access via wget"),
                (Regex::new(r"git\s+clone").unwrap(), "Git clone in PKGBUILD"),
                (Regex::new(r"pip\s+install").unwrap(), "pip install (bypasses pacman)"),
                (Regex::new(r"npm\s+install").unwrap(), "npm install (bypasses pacman)"),
                (Regex::new(r"cargo\s+install").unwrap(), "cargo install (bypasses pacman)"),
                (Regex::new(r"go\s+get").unwrap(), "go get (bypasses pacman)"),
                (Regex::new(r"sudo\s").unwrap(), "sudo usage in PKGBUILD"),
                (Regex::new(r"doas\s").unwrap(), "doas usage in PKGBUILD"),
            ],
            low_patterns: vec![
                (Regex::new(r"rm\s+-rf").unwrap(), "Recursive deletion"),
                (Regex::new(r"chmod\s").unwrap(), "Permission changes"),
                (Regex::new(r"chown\s").unwrap(), "Ownership changes"),
            ],
            suspicious_commands: [
                "rm", "curl", "wget", "nc", "ncat", "netcat", "base64",
                "eval", "exec", "dd", "mkfs", "fdisk", "parted",
            ].into_iter().collect(),
        }
    }
    
    /// Validate a PKGBUILD file
    pub fn validate(&self, content: &str) -> SecurityReport {
        let mut issues = Vec::new();
        
        // Check each line
        for (line_num, line) in content.lines().enumerate() {
            let line_num = line_num + 1;
            
            // Skip comments
            if line.trim().starts_with('#') {
                continue;
            }
            
            // Check critical patterns
            for (pattern, desc) in &self.critical_patterns {
                if pattern.is_match(line) {
                    issues.push(SecurityIssue {
                        severity: SecuritySeverity::Critical,
                        description: desc.to_string(),
                        line: Some(line_num),
                        pattern: pattern.as_str().to_string(),
                    });
                }
            }
            
            // Check high severity patterns
            for (pattern, desc) in &self.high_patterns {
                if pattern.is_match(line) {
                    issues.push(SecurityIssue {
                        severity: SecuritySeverity::High,
                        description: desc.to_string(),
                        line: Some(line_num),
                        pattern: pattern.as_str().to_string(),
                    });
                }
            }
            
            // Check medium severity patterns
            for (pattern, desc) in &self.medium_patterns {
                if pattern.is_match(line) {
                    issues.push(SecurityIssue {
                        severity: SecuritySeverity::Medium,
                        description: desc.to_string(),
                        line: Some(line_num),
                        pattern: pattern.as_str().to_string(),
                    });
                }
            }
            
            // Check low severity patterns
            for (pattern, desc) in &self.low_patterns {
                if pattern.is_match(line) {
                    issues.push(SecurityIssue {
                        severity: SecuritySeverity::Low,
                        description: desc.to_string(),
                        line: Some(line_num),
                        pattern: pattern.as_str().to_string(),
                    });
                }
            }
        }
        
        // Check for network access in build() function
        if self.has_network_in_build(content) {
            issues.push(SecurityIssue {
                severity: SecuritySeverity::Medium,
                description: "Network access detected in build() function - violates Arch packaging guidelines".to_string(),
                line: None,
                pattern: "network in build()".to_string(),
            });
        }
        
        // Calculate security score
        let score = self.calculate_score(&issues);
        let passed = score >= 50 && !issues.iter().any(|i| i.severity == SecuritySeverity::Critical);
        
        SecurityReport {
            issues,
            score,
            passed,
        }
    }
    
    /// Check if there's network access in the build() function
    fn has_network_in_build(&self, content: &str) -> bool {
        // Simple heuristic: look for network commands between build() and the next function
        let network_patterns = ["curl", "wget", "git clone", "pip install", "npm install"];
        
        let mut in_build = false;
        for line in content.lines() {
            let trimmed = line.trim();
            
            if trimmed.starts_with("build()") || trimmed.starts_with("build ()") {
                in_build = true;
            } else if in_build && (
                trimmed.starts_with("package()") ||
                trimmed.starts_with("package_") ||
                trimmed.starts_with("check()")
            ) {
                in_build = false;
            }
            
            if in_build && !trimmed.starts_with('#') {
                for pattern in &network_patterns {
                    if trimmed.contains(pattern) {
                        return true;
                    }
                }
            }
        }
        
        false
    }
    
    /// Calculate security score based on issues
    fn calculate_score(&self, issues: &[SecurityIssue]) -> u32 {
        let mut score: i32 = 100;
        
        for issue in issues {
            match issue.severity {
                SecuritySeverity::Critical => score -= 100,
                SecuritySeverity::High => score -= 30,
                SecuritySeverity::Medium => score -= 10,
                SecuritySeverity::Low => score -= 3,
                SecuritySeverity::Info => score -= 1,
            }
        }
        
        score.max(0) as u32
    }
}

impl Default for SecurityValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a PKGBUILD file
pub fn parse_pkgbuild(path: &Path) -> Result<Pkgbuild> {
    let content = fs::read_to_string(path)?;
    parse_pkgbuild_content(&content)
}

/// Parse PKGBUILD content string
pub fn parse_pkgbuild_content(content: &str) -> Result<Pkgbuild> {
    let mut pkgbuild = Pkgbuild {
        raw_content: content.to_string(),
        ..Default::default()
    };
    
    // Simple regex-based parsing for common fields
    // Note: This is a simplified parser - a full parser would need to handle
    // bash syntax properly including variable expansion, arrays, etc.
    
    // Single value fields
    if let Some(cap) = Regex::new(r"pkgbase=([^\n]+)")?.captures(content) {
        pkgbuild.pkgbase = cap.get(1).unwrap().as_str().trim().trim_matches('"').trim_matches('\'').to_string();
    }
    
    if let Some(cap) = Regex::new(r"pkgver=([^\n]+)")?.captures(content) {
        pkgbuild.pkgver = cap.get(1).unwrap().as_str().trim().trim_matches('"').trim_matches('\'').to_string();
    }
    
    if let Some(cap) = Regex::new(r"pkgrel=([^\n]+)")?.captures(content) {
        pkgbuild.pkgrel = cap.get(1).unwrap().as_str().trim().trim_matches('"').trim_matches('\'').to_string();
    }
    
    if let Some(cap) = Regex::new(r"epoch=([^\n]+)")?.captures(content) {
        let epoch_str = cap.get(1).unwrap().as_str().trim();
        pkgbuild.epoch = epoch_str.parse().ok();
    }
    
    if let Some(cap) = Regex::new(r#"pkgdesc=["']([^"']+)["']"#)?.captures(content) {
        pkgbuild.pkgdesc = cap.get(1).unwrap().as_str().to_string();
    }
    
    if let Some(cap) = Regex::new(r#"url=["']([^"']+)["']"#)?.captures(content) {
        pkgbuild.url = cap.get(1).unwrap().as_str().to_string();
    }
    
    // Array fields
    pkgbuild.pkgname = parse_array(content, "pkgname");
    pkgbuild.arch = parse_array(content, "arch");
    pkgbuild.license = parse_array(content, "license");
    pkgbuild.depends = parse_array(content, "depends");
    pkgbuild.makedepends = parse_array(content, "makedepends");
    pkgbuild.checkdepends = parse_array(content, "checkdepends");
    pkgbuild.optdepends = parse_array(content, "optdepends");
    pkgbuild.provides = parse_array(content, "provides");
    pkgbuild.conflicts = parse_array(content, "conflicts");
    pkgbuild.replaces = parse_array(content, "replaces");
    pkgbuild.source = parse_array(content, "source");
    pkgbuild.sha256sums = parse_array(content, "sha256sums");
    pkgbuild.sha512sums = parse_array(content, "sha512sums");
    pkgbuild.md5sums = parse_array(content, "md5sums");
    pkgbuild.validpgpkeys = parse_array(content, "validpgpkeys");
    pkgbuild.backup = parse_array(content, "backup");
    pkgbuild.options = parse_array(content, "options");
    
    // If pkgbase is empty, use first pkgname
    if pkgbuild.pkgbase.is_empty() && !pkgbuild.pkgname.is_empty() {
        pkgbuild.pkgbase = pkgbuild.pkgname[0].clone();
    }
    
    Ok(pkgbuild)
}

/// Parse a bash array from PKGBUILD content
fn parse_array(content: &str, field: &str) -> Vec<String> {
    // Try to match array assignment: field=(...)
    let pattern = format!(r"{}=\(([^)]*)\)", field);
    if let Ok(re) = Regex::new(&pattern) {
        if let Some(cap) = re.captures(content) {
            let array_content = cap.get(1).unwrap().as_str();
            return array_content
                .split_whitespace()
                .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    
    // Try single value: field=value
    let pattern = format!(r"{}=([^\n\(]+)", field);
    if let Ok(re) = Regex::new(&pattern) {
        if let Some(cap) = re.captures(content) {
            let value = cap.get(1).unwrap().as_str().trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return vec![value.to_string()];
            }
        }
    }
    
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_security_validator_critical() {
        let validator = SecurityValidator::new();
        let content = "rm -rf /etc/passwd";  // Malicious
        let report = validator.validate(content);
        
        assert!(!report.passed);
        assert!(report.has_critical() || report.has_high());
    }
    
    #[test]
    fn test_security_validator_safe() {
        let validator = SecurityValidator::new();
        let content = r#"
pkgname=test
pkgver=1.0.0
pkgrel=1
arch=('x86_64')

build() {
    make
}

package() {
    make DESTDIR="$pkgdir" install
}
"#;
        let report = validator.validate(content);
        
        assert!(report.passed);
        assert_eq!(report.score, 100);
    }
    
    #[test]
    fn test_security_validator_network_in_build() {
        let validator = SecurityValidator::new();
        let content = r#"
build() {
    curl -O https://example.com/file
    make
}
"#;
        let report = validator.validate(content);
        
        // Should detect network access in build
        assert!(report.issues.iter().any(|i| 
            i.description.contains("Network access")
        ));
    }
    
    #[test]
    fn test_parse_pkgbuild() {
        let content = r#"
pkgname=test-package
pkgver=1.2.3
pkgrel=1
pkgdesc="A test package"
arch=('x86_64' 'aarch64')
url="https://example.com"
license=('GPL')
depends=('glibc' 'gcc-libs')
makedepends=('cmake')
source=("https://example.com/test-1.2.3.tar.gz")
sha256sums=('1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef')
"#;
        
        let pkgbuild = parse_pkgbuild_content(content).unwrap();
        
        assert_eq!(pkgbuild.pkgname, vec!["test-package"]);
        assert_eq!(pkgbuild.pkgver, "1.2.3");
        assert_eq!(pkgbuild.pkgrel, "1");
        assert_eq!(pkgbuild.pkgdesc, "A test package");
        assert_eq!(pkgbuild.arch, vec!["x86_64", "aarch64"]);
        assert_eq!(pkgbuild.depends, vec!["glibc", "gcc-libs"]);
        assert_eq!(pkgbuild.makedepends, vec!["cmake"]);
    }
    
    #[test]
    fn test_parse_array() {
        let content = "depends=('foo' 'bar' 'baz')";
        let result = parse_array(content, "depends");
        assert_eq!(result, vec!["foo", "bar", "baz"]);
        
        let content = "pkgname=single";
        let result = parse_array(content, "pkgname");
        assert_eq!(result, vec!["single"]);
    }
}
