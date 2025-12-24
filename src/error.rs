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

//! Hierarchical error types with context and recovery strategies.

use std::fmt;
use thiserror::Error;

/// Main error type for Pacboost operations
#[derive(Debug, Error)]
pub enum PacboostError {
    /// Database-related errors (ALPM operations)
    #[error("Database error: {context}")]
    Database {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Network errors during downloads or API calls
    #[error("Network error for {url}: {message}")]
    Network {
        url: String,
        message: String,
        #[source]
        source: Option<reqwest::Error>,
    },

    /// AUR package not found
    #[error("AUR package '{package}' not found")]
    AurPackageNotFound { package: String },

    /// AUR dependency resolution failures
    #[error("AUR dependency resolution failed for '{package}': {reason}")]
    AurDependencyError { package: String, reason: String },

    /// Circular dependency detected in AUR packages
    #[error("Circular dependency detected: {}", .cycle.join(" -> "))]
    CircularDependency { cycle: Vec<String> },

    /// PKGBUILD validation errors
    #[error("PKGBUILD validation failed: {reason}")]
    PkgbuildInvalid { reason: String },

    /// PKGBUILD security issues detected
    #[error("PKGBUILD security issue: {reason}")]
    PkgbuildSecurityIssue { 
        reason: String,
        severity: SecuritySeverity,
    },

    /// Build failures during AUR package compilation
    #[error("Build failed for '{package}': {reason}")]
    BuildFailed { 
        package: String, 
        reason: String,
        exit_code: Option<i32>,
    },

    /// Build timeout
    #[error("Build timed out for '{package}' after {timeout_secs} seconds")]
    BuildTimeout {
        package: String,
        timeout_secs: u64,
    },

    /// Transaction failures
    #[error("Transaction failed: {reason}")]
    TransactionFailed { reason: String },

    /// Transaction rollback required
    #[error("Transaction requires rollback: {reason}")]
    TransactionRollbackRequired {
        reason: String,
        checkpoint_id: Option<String>,
    },

    /// Permission denied
    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    /// File system errors
    #[error("File system error for '{path}': {message}")]
    FileSystem {
        path: String,
        message: String,
        #[source]
        source: Option<std::io::Error>,
    },

    /// Mirror-related errors
    #[error("All mirrors failed for '{file}': {attempts} attempts")]
    MirrorExhausted {
        file: String,
        attempts: usize,
        last_error: String,
    },

    /// Signature verification failure
    #[error("Signature verification failed for '{package}'")]
    SignatureInvalid { package: String },

    /// Checksum mismatch
    #[error("Checksum mismatch for '{file}': expected {expected}, got {actual}")]
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },

    /// Version conflict
    #[error("Version conflict: {package} requires {required} but {installed} is installed")]
    VersionConflict {
        package: String,
        required: String,
        installed: String,
    },

    /// Package provides conflict
    #[error("Package conflict: {package1} and {package2} both provide {provides}")]
    ProvidesConflict {
        package1: String,
        package2: String,
        provides: String,
    },

    /// Lock file error
    #[error("Database is locked by process {pid}")]
    DatabaseLocked { pid: i32 },

    /// Interrupted operation
    #[error("Operation interrupted")]
    Interrupted,

    /// Generic/wrapped error for backward compatibility
    #[error("{0}")]
    Other(String),
}

/// Security severity levels for PKGBUILD issues
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecuritySeverity {
    /// Informational - style issues
    Info,
    /// Low - minor concerns
    Low,
    /// Medium - should be reviewed
    Medium,
    /// High - likely malicious or dangerous
    High,
    /// Critical - definitely malicious
    Critical,
}

impl fmt::Display for SecuritySeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecuritySeverity::Info => write!(f, "INFO"),
            SecuritySeverity::Low => write!(f, "LOW"),
            SecuritySeverity::Medium => write!(f, "MEDIUM"),
            SecuritySeverity::High => write!(f, "HIGH"),
            SecuritySeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Recovery strategy for errors
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry the operation with exponential backoff
    Retry { 
        max_attempts: u32, 
        initial_delay_ms: u64,
    },
    /// Try an alternative approach
    Fallback { 
        alternative: String,
    },
    /// Abort and cleanup
    Abort,
    /// Prompt user for decision
    UserPrompt { 
        message: String, 
        options: Vec<String>,
    },
    /// No recovery possible
    Fatal,
}

impl PacboostError {
    /// Get the recommended recovery strategy for this error
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            PacboostError::Network { .. } => RecoveryStrategy::Retry {
                max_attempts: 3,
                initial_delay_ms: 1000,
            },
            PacboostError::MirrorExhausted { .. } => RecoveryStrategy::Fallback {
                alternative: "Try updating mirrorlist".to_string(),
            },
            PacboostError::DatabaseLocked { .. } => RecoveryStrategy::Retry {
                max_attempts: 5,
                initial_delay_ms: 2000,
            },
            PacboostError::BuildFailed { .. } => RecoveryStrategy::UserPrompt {
                message: "Build failed".to_string(),
                options: vec!["Retry".to_string(), "Skip".to_string(), "Abort".to_string()],
            },
            PacboostError::PkgbuildSecurityIssue { severity, .. } => {
                if *severity == SecuritySeverity::Critical {
                    RecoveryStrategy::Abort
                } else {
                    RecoveryStrategy::UserPrompt {
                        message: "Security issue detected".to_string(),
                        options: vec!["Continue anyway".to_string(), "Abort".to_string()],
                    }
                }
            },
            PacboostError::PermissionDenied { .. } => RecoveryStrategy::Fatal,
            PacboostError::CircularDependency { .. } => RecoveryStrategy::Fatal,
            _ => RecoveryStrategy::Abort,
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.recovery_strategy(),
            RecoveryStrategy::Retry { .. }
        )
    }

    /// Create a database error
    pub fn database<E: std::error::Error + Send + Sync + 'static>(context: impl Into<String>, source: E) -> Self {
        PacboostError::Database {
            context: context.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a network error
    pub fn network(url: impl Into<String>, message: impl Into<String>) -> Self {
        PacboostError::Network {
            url: url.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Create a filesystem error
    pub fn filesystem<E: Into<std::io::Error>>(path: impl Into<String>, message: impl Into<String>, source: E) -> Self {
        PacboostError::FileSystem {
            path: path.into(),
            message: message.into(),
            source: Some(source.into()),
        }
    }
}

/// Result type alias for Pacboost operations
pub type PacboostResult<T> = std::result::Result<T, PacboostError>;

/// Extension trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn context(self, context: impl Into<String>) -> PacboostResult<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ErrorContext<T> for Result<T, E> {
    fn context(self, context: impl Into<String>) -> PacboostResult<T> {
        self.map_err(|e| PacboostError::Other(format!("{}: {}", context.into(), e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PacboostError::AurPackageNotFound { 
            package: "test-pkg".to_string() 
        };
        assert_eq!(format!("{}", err), "AUR package 'test-pkg' not found");
    }

    #[test]
    fn test_circular_dependency_display() {
        let err = PacboostError::CircularDependency { 
            cycle: vec!["a".to_string(), "b".to_string(), "c".to_string(), "a".to_string()]
        };
        assert_eq!(format!("{}", err), "Circular dependency detected: a -> b -> c -> a");
    }

    #[test]
    fn test_recovery_strategy() {
        let network_err = PacboostError::network("http://test", "timeout");
        assert!(network_err.is_retryable());

        let perm_err = PacboostError::PermissionDenied { 
            operation: "write".to_string() 
        };
        assert!(!perm_err.is_retryable());
    }

    #[test]
    fn test_security_severity_display() {
        assert_eq!(format!("{}", SecuritySeverity::Critical), "CRITICAL");
        assert_eq!(format!("{}", SecuritySeverity::Low), "LOW");
    }
}
