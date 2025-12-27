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

//! Configuration management with validation and defaults.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Main configuration structure for Pacboost
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Number of concurrent downloads
    pub download_concurrency: usize,

    /// Download timeout in seconds
    pub download_timeout_secs: u64,

    /// Timeout per mirror in seconds
    pub mirror_timeout_secs: u64,

    /// Enable colored output
    pub color: bool,

    /// AUR-specific configuration
    pub aur: AurConfig,

    /// Mirror configuration
    pub mirrors: MirrorConfig,

    /// Cache configuration
    pub cache: CacheConfig,

    /// Logging configuration
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            download_concurrency: 4,
            download_timeout_secs: 300,
            mirror_timeout_secs: 3,
            color: true,
            aur: AurConfig::default(),
            mirrors: MirrorConfig::default(),
            cache: CacheConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

/// AUR-specific configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AurConfig {
    /// Enable security scanning of PKGBUILDs
    pub security_scan: bool,

    /// Minimum security score to proceed (0-100)
    pub min_security_score: u32,

    /// Automatically install AUR dependencies
    pub auto_deps: bool,

    /// Show PKGBUILD diff before building
    pub show_diff: bool,

    /// Clean build directory after installation
    pub clean_build: bool,

    /// Use ccache for faster rebuilds
    pub use_ccache: bool,

    /// Maximum build time in seconds (0 = unlimited)
    pub build_timeout_secs: u64,

    /// Number of parallel make jobs (0 = auto-detect)
    pub make_jobs: usize,

    /// Disable compression for faster local builds
    pub disable_compression: bool,

    /// AUR RPC base URL
    pub rpc_url: String,

    /// Build directory path
    pub build_dir: PathBuf,
}

impl Default for AurConfig {
    fn default() -> Self {
        Self {
            security_scan: true,
            min_security_score: 50,
            auto_deps: true,
            show_diff: false,
            clean_build: true,
            use_ccache: false,
            build_timeout_secs: 0,
            make_jobs: 0, // Auto-detect
            disable_compression: true,
            rpc_url: "https://aur.archlinux.org/rpc/".to_string(),
            build_dir: PathBuf::from("/tmp/pacboost-aur"),
        }
    }
}

/// Mirror configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MirrorConfig {
    /// Maximum number of mirrors to try per download
    pub max_mirrors: usize,

    /// Track mirror health statistics
    pub track_health: bool,

    /// Blacklist slow mirrors after this many failures
    pub blacklist_threshold: u32,

    /// Re-test blacklisted mirrors after this many seconds
    pub blacklist_duration_secs: u64,
}

impl Default for MirrorConfig {
    fn default() -> Self {
        Self {
            max_mirrors: 5,
            track_health: true,
            blacklist_threshold: 3,
            blacklist_duration_secs: 3600,
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Package cache directory
    pub package_dir: PathBuf,

    /// AUR metadata cache size (number of entries)
    pub aur_cache_size: usize,

    /// Enable build cache for AUR packages
    pub build_cache: bool,

    /// Maximum cache size in MB (0 = unlimited)
    pub max_size_mb: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            package_dir: PathBuf::from("/var/cache/pacman/pkg"),
            aur_cache_size: 500,
            build_cache: true,
            max_size_mb: 0,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Log file path (empty = no file logging)
    pub file: Option<PathBuf>,

    /// Enable structured JSON logging
    pub json: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            json: false,
        }
    }
}

impl Config {
    /// Load configuration from multiple sources with precedence:
    /// 1. /etc/pacboost/pacboost.toml (system-wide)
    /// 2. ~/.config/pacboost/config.toml (user)
    /// 3. Environment variables (PACBOOST_*)
    pub fn load() -> Self {
        let mut config = Config::default();

        // Try system-wide config
        let system_config = Path::new("/etc/pacboost/pacboost.toml");
        if system_config.exists() {
            if let Ok(content) = fs::read_to_string(system_config) {
                if let Ok(parsed) = toml::from_str::<Config>(&content) {
                    config = config.merge(parsed);
                }
            }
        }

        // Try user config
        if let Some(config_dir) = dirs::config_dir() {
            let user_config = config_dir.join("pacboost").join("config.toml");
            if user_config.exists() {
                if let Ok(content) = fs::read_to_string(user_config) {
                    if let Ok(parsed) = toml::from_str::<Config>(&content) {
                        config = config.merge(parsed);
                    }
                }
            }
        }

        // Apply environment overrides
        config = config.apply_env_overrides();

        config
    }

    /// Merge another config into this one (other takes precedence for non-default values)
    fn merge(mut self, other: Config) -> Self {
        // Only override if the other value differs from default
        let default = Config::default();

        if other.download_concurrency != default.download_concurrency {
            self.download_concurrency = other.download_concurrency;
        }
        if other.download_timeout_secs != default.download_timeout_secs {
            self.download_timeout_secs = other.download_timeout_secs;
        }
        if other.mirror_timeout_secs != default.mirror_timeout_secs {
            self.mirror_timeout_secs = other.mirror_timeout_secs;
        }
        if other.color != default.color {
            self.color = other.color;
        }

        // Merge sub-configs
        self.aur = self.aur.merge(other.aur);

        self
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(mut self) -> Self {
        if let Ok(val) = std::env::var("PACBOOST_CONCURRENCY") {
            if let Ok(n) = val.parse() {
                self.download_concurrency = n;
            }
        }

        if let Ok(val) = std::env::var("PACBOOST_AUR_SECURITY_SCAN") {
            self.aur.security_scan = val == "1" || val.to_lowercase() == "true";
        }

        if let Ok(val) = std::env::var("PACBOOST_AUR_AUTO_DEPS") {
            self.aur.auto_deps = val == "1" || val.to_lowercase() == "true";
        }

        if let Ok(val) = std::env::var("PACBOOST_LOG_LEVEL") {
            self.logging.level = val;
        }

        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.download_concurrency == 0 {
            return Err("download_concurrency must be at least 1".to_string());
        }
        if self.download_concurrency > 64 {
            return Err("download_concurrency must be at most 64".to_string());
        }
        if self.aur.min_security_score > 100 {
            return Err("min_security_score must be at most 100".to_string());
        }
        Ok(())
    }

    /// Get the number of make jobs, auto-detecting if set to 0
    pub fn get_make_jobs(&self) -> usize {
        if self.aur.make_jobs == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        } else {
            self.aur.make_jobs
        }
    }
}

impl AurConfig {
    fn merge(mut self, other: AurConfig) -> Self {
        let default = AurConfig::default();

        if other.security_scan != default.security_scan {
            self.security_scan = other.security_scan;
        }
        if other.min_security_score != default.min_security_score {
            self.min_security_score = other.min_security_score;
        }
        if other.auto_deps != default.auto_deps {
            self.auto_deps = other.auto_deps;
        }
        if other.build_timeout_secs != default.build_timeout_secs {
            self.build_timeout_secs = other.build_timeout_secs;
        }
        if other.make_jobs != default.make_jobs {
            self.make_jobs = other.make_jobs;
        }
        if other.rpc_url != default.rpc_url {
            self.rpc_url = other.rpc_url;
        }
        if other.build_dir != default.build_dir {
            self.build_dir = other.build_dir;
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.download_concurrency, 4);
        assert!(config.aur.security_scan);
        assert!(config.aur.auto_deps);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        config.download_concurrency = 0;
        assert!(config.validate().is_err());

        config.download_concurrency = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_get_make_jobs() {
        let config = Config::default();
        assert!(config.get_make_jobs() >= 1);
    }
}
