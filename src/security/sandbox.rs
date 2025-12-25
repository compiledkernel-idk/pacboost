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

//! Sandboxed execution for PKGBUILDs.

use anyhow::{Result, Context, anyhow};
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::fs;

/// Sandbox configuration
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Enable network access
    pub network: bool,
    /// Read-only paths
    pub readonly_paths: Vec<PathBuf>,
    /// Read-write paths (bind mounts)
    pub readwrite_paths: Vec<PathBuf>,
    /// Environment variables to pass
    pub env_vars: Vec<(String, String)>,
    /// Memory limit in MB
    pub memory_limit_mb: Option<u64>,
    /// CPU limit (number of cores)
    pub cpu_limit: Option<u32>,
    /// Timeout in seconds
    pub timeout_secs: Option<u64>,
    /// Disable user namespace (for compatibility)
    pub no_user_ns: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            network: false,
            readonly_paths: vec![
                PathBuf::from("/usr"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
                PathBuf::from("/etc"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
            ],
            readwrite_paths: Vec::new(),
            env_vars: Vec::new(),
            memory_limit_mb: Some(4096),
            cpu_limit: None,
            timeout_secs: Some(3600),
            no_user_ns: false,
        }
    }
}

impl SandboxConfig {
    /// Create a config for building packages
    pub fn for_build(build_dir: &Path) -> Self {
        Self {
            network: false, // No network during build
            readonly_paths: vec![
                PathBuf::from("/usr"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
                PathBuf::from("/etc"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
            ],
            readwrite_paths: vec![
                build_dir.to_path_buf(),
            ],
            env_vars: vec![
                ("HOME".to_string(), "/tmp/build".to_string()),
                ("USER".to_string(), "nobody".to_string()),
            ],
            memory_limit_mb: Some(8192),
            cpu_limit: None,
            timeout_secs: Some(3600),
            no_user_ns: false,
        }
    }

    /// Create a config for prepare phase (allows network)
    pub fn for_prepare(build_dir: &Path) -> Self {
        let mut config = Self::for_build(build_dir);
        config.network = true;
        config
    }
}

/// Sandboxed execution environment
pub struct Sandbox {
    config: SandboxConfig,
    backend: SandboxBackend,
}

#[derive(Debug, Clone, Copy)]
enum SandboxBackend {
    Bubblewrap,
    Firejail,
    None,
}

impl Sandbox {
    /// Create a new sandbox with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(SandboxConfig::default())
    }

    /// Create a sandbox with custom configuration
    pub fn with_config(config: SandboxConfig) -> Result<Self> {
        let backend = Self::detect_backend();
        Ok(Self { config, backend })
    }

    /// Detect available sandbox backend
    fn detect_backend() -> SandboxBackend {
        if which::which("bwrap").is_ok() {
            SandboxBackend::Bubblewrap
        } else if which::which("firejail").is_ok() {
            SandboxBackend::Firejail
        } else {
            SandboxBackend::None
        }
    }

    /// Check if sandboxing is available
    pub fn is_available() -> bool {
        !matches!(Self::detect_backend(), SandboxBackend::None)
    }

    /// Get the sandbox backend name
    pub fn backend_name(&self) -> &'static str {
        match self.backend {
            SandboxBackend::Bubblewrap => "bubblewrap",
            SandboxBackend::Firejail => "firejail",
            SandboxBackend::None => "none",
        }
    }

    /// Execute a command in the sandbox
    pub fn execute(&self, cmd: &str, args: &[&str], work_dir: &Path) -> Result<SandboxResult> {
        match self.backend {
            SandboxBackend::Bubblewrap => self.execute_bwrap(cmd, args, work_dir),
            SandboxBackend::Firejail => self.execute_firejail(cmd, args, work_dir),
            SandboxBackend::None => self.execute_unsandboxed(cmd, args, work_dir),
        }
    }

    /// Execute using bubblewrap
    fn execute_bwrap(&self, cmd: &str, args: &[&str], work_dir: &Path) -> Result<SandboxResult> {
        let mut bwrap_args = vec![
            "--die-with-parent".to_string(),
            "--unshare-pid".to_string(),
            "--unshare-ipc".to_string(),
        ];

        if !self.config.no_user_ns {
            bwrap_args.push("--unshare-user".to_string());
        }

        if !self.config.network {
            bwrap_args.push("--unshare-net".to_string());
        }

        // Mount proc and tmp
        bwrap_args.extend_from_slice(&[
            "--proc".to_string(), "/proc".to_string(),
            "--dev".to_string(), "/dev".to_string(),
            "--tmpfs".to_string(), "/tmp".to_string(),
        ]);

        // Add readonly mounts
        for path in &self.config.readonly_paths {
            if path.exists() {
                bwrap_args.push("--ro-bind".to_string());
                bwrap_args.push(path.to_string_lossy().to_string());
                bwrap_args.push(path.to_string_lossy().to_string());
            }
        }

        // Add read-write mounts
        for path in &self.config.readwrite_paths {
            if path.exists() {
                bwrap_args.push("--bind".to_string());
                bwrap_args.push(path.to_string_lossy().to_string());
                bwrap_args.push(path.to_string_lossy().to_string());
            }
        }

        // Set working directory
        bwrap_args.push("--chdir".to_string());
        bwrap_args.push(work_dir.to_string_lossy().to_string());

        // Add the command
        bwrap_args.push(cmd.to_string());
        bwrap_args.extend(args.iter().map(|s| s.to_string()));

        let mut command = Command::new("bwrap");
        command.args(&bwrap_args);

        // Set environment variables
        for (key, value) in &self.config.env_vars {
            command.env(key, value);
        }

        // Apply timeout if configured
        if let Some(timeout) = self.config.timeout_secs {
            command = self.wrap_with_timeout(command, timeout)?;
        }

        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute sandboxed command")?;

        Ok(SandboxResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
        })
    }

    /// Execute using firejail
    fn execute_firejail(&self, cmd: &str, args: &[&str], work_dir: &Path) -> Result<SandboxResult> {
        let mut firejail_args = vec!["--quiet".to_string()];

        if !self.config.network {
            firejail_args.push("--net=none".to_string());
        }

        // Add readonly paths
        for path in &self.config.readonly_paths {
            if path.exists() {
                firejail_args.push(format!("--read-only={}", path.display()));
            }
        }

        // Add read-write paths as whitelist
        for path in &self.config.readwrite_paths {
            if path.exists() {
                firejail_args.push(format!("--whitelist={}", path.display()));
            }
        }

        // Add the command
        firejail_args.push(cmd.to_string());
        firejail_args.extend(args.iter().map(|s| s.to_string()));

        let mut command = Command::new("firejail");
        command.args(&firejail_args);
        command.current_dir(work_dir);

        // Set environment variables
        for (key, value) in &self.config.env_vars {
            command.env(key, value);
        }

        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute sandboxed command")?;

        Ok(SandboxResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
        })
    }

    /// Execute without sandbox (fallback)
    fn execute_unsandboxed(&self, cmd: &str, args: &[&str], work_dir: &Path) -> Result<SandboxResult> {
        tracing::warn!("No sandbox available, running command unsandboxed");

        let mut command = Command::new(cmd);
        command.args(args);
        command.current_dir(work_dir);

        for (key, value) in &self.config.env_vars {
            command.env(key, value);
        }

        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to execute command")?;

        Ok(SandboxResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
        })
    }

    /// Wrap command with timeout
    fn wrap_with_timeout(&self, mut cmd: Command, timeout_secs: u64) -> Result<Command> {
        // For now, just use the command as-is. In a real implementation,
        // we'd wrap with `timeout` or use async with tokio timeout.
        Ok(cmd)
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            config: SandboxConfig::default(),
            backend: SandboxBackend::None,
        })
    }
}

/// Result of sandboxed execution
#[derive(Debug, Clone)]
pub struct SandboxResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl SandboxResult {
    pub fn is_success(&self) -> bool {
        self.success
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert!(!config.network);
        assert!(config.readonly_paths.len() > 0);
    }

    #[test]
    fn test_sandbox_backend_detection() {
        let backend = Sandbox::detect_backend();
        // Just verify it doesn't panic
        println!("Detected backend: {:?}", backend);
    }

    #[test]
    fn test_sandbox_is_available() {
        let _ = Sandbox::is_available();
    }

    #[test]
    fn test_sandbox_config_for_build() {
        let config = SandboxConfig::for_build(Path::new("/tmp/build"));
        assert!(!config.network);
        assert!(config.readwrite_paths.contains(&PathBuf::from("/tmp/build")));
    }

    #[test]
    fn test_sandbox_config_for_prepare() {
        let config = SandboxConfig::for_prepare(Path::new("/tmp/build"));
        assert!(config.network);
    }
}
