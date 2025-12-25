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

//! Container integration for pacboost.
//!
//! Support for Docker and Podman container operations:
//! - Image management
//! - Container execution
//! - Package installation in containers

use anyhow::{Result, Context, anyhow};
use console::style;
use std::process::{Command, Stdio};
use serde::{Deserialize, Serialize};

/// Container runtime type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerRuntime {
    Docker,
    Podman,
}

impl ContainerRuntime {
    /// Get the command name
    pub fn command(&self) -> &'static str {
        match self {
            ContainerRuntime::Docker => "docker",
            ContainerRuntime::Podman => "podman",
        }
    }

    /// Check if this runtime is available
    pub fn is_available(&self) -> bool {
        which::which(self.command()).is_ok()
    }

    /// Detect the best available runtime
    pub fn detect() -> Option<Self> {
        if Self::Podman.is_available() {
            Some(Self::Podman)
        } else if Self::Docker.is_available() {
            Some(Self::Docker)
        } else {
            None
        }
    }
}

impl std::fmt::Display for ContainerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerRuntime::Docker => write!(f, "Docker"),
            ContainerRuntime::Podman => write!(f, "Podman"),
        }
    }
}

/// Container image information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerImage {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub created: String,
    pub size: String,
}

/// Running container information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub image: String,
    pub command: String,
    pub created: String,
    pub status: String,
    pub ports: String,
    pub names: String,
}

/// Container manager
pub struct ContainerManager {
    runtime: ContainerRuntime,
}

impl ContainerManager {
    /// Create a new container manager with auto-detected runtime
    pub fn new() -> Result<Self> {
        let runtime = ContainerRuntime::detect()
            .ok_or_else(|| anyhow!("No container runtime found. Install Docker or Podman."))?;
        
        Ok(Self { runtime })
    }

    /// Create with specific runtime
    pub fn with_runtime(runtime: ContainerRuntime) -> Result<Self> {
        if !runtime.is_available() {
            return Err(anyhow!("{} is not installed", runtime));
        }
        Ok(Self { runtime })
    }

    /// Get the runtime being used
    pub fn runtime(&self) -> ContainerRuntime {
        self.runtime
    }

    /// List images
    pub fn list_images(&self) -> Result<Vec<ContainerImage>> {
        let output = Command::new(self.runtime.command())
            .args(["images", "--format", "{{.ID}}\t{{.Repository}}\t{{.Tag}}\t{{.CreatedAt}}\t{{.Size}}"])
            .output()
            .context("Failed to list images")?;

        if !output.status.success() {
            return Err(anyhow!("Failed to list images: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut images = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 5 {
                images.push(ContainerImage {
                    id: parts[0].to_string(),
                    repository: parts[1].to_string(),
                    tag: parts[2].to_string(),
                    created: parts[3].to_string(),
                    size: parts[4].to_string(),
                });
            }
        }

        Ok(images)
    }

    /// List running containers
    pub fn list_containers(&self) -> Result<Vec<Container>> {
        let output = Command::new(self.runtime.command())
            .args(["ps", "--format", "{{.ID}}\t{{.Image}}\t{{.Command}}\t{{.CreatedAt}}\t{{.Status}}\t{{.Ports}}\t{{.Names}}"])
            .output()
            .context("Failed to list containers")?;

        if !output.status.success() {
            return Err(anyhow!("Failed to list containers"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut containers = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 7 {
                containers.push(Container {
                    id: parts[0].to_string(),
                    image: parts[1].to_string(),
                    command: parts[2].to_string(),
                    created: parts[3].to_string(),
                    status: parts[4].to_string(),
                    ports: parts[5].to_string(),
                    names: parts[6].to_string(),
                });
            }
        }

        Ok(containers)
    }

    /// Pull an image
    pub fn pull(&self, image: &str) -> Result<()> {
        println!("{} Pulling image: {}",
            style("::").cyan().bold(),
            style(image).yellow().bold());

        let status = Command::new(self.runtime.command())
            .args(["pull", image])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to pull image")?;

        if !status.success() {
            return Err(anyhow!("Failed to pull {}", image));
        }

        println!("{} {} pulled",
            style("::").green().bold(),
            style(image).white().bold());

        Ok(())
    }

    /// Run a container
    pub fn run(&self, image: &str, args: &[String], interactive: bool) -> Result<()> {
        let mut cmd_args = vec!["run"];
        
        if interactive {
            cmd_args.push("-it");
        }
        
        cmd_args.push("--rm");
        cmd_args.push(image);

        let status = Command::new(self.runtime.command())
            .args(&cmd_args)
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::inherit())
            .status()
            .context("Failed to run container")?;

        if !status.success() {
            return Err(anyhow!("Container exited with error"));
        }

        Ok(())
    }

    /// Execute command in existing container
    pub fn exec(&self, container: &str, command: &[String]) -> Result<()> {
        let mut args = vec!["exec", "-it", container];
        let cmd_strs: Vec<&str> = command.iter().map(|s| s.as_str()).collect();
        args.extend(cmd_strs);

        let status = Command::new(self.runtime.command())
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::inherit())
            .status()
            .context("Failed to exec in container")?;

        if !status.success() {
            return Err(anyhow!("Command failed in container"));
        }

        Ok(())
    }

    /// Stop a container
    pub fn stop(&self, container: &str) -> Result<()> {
        println!("{} Stopping container: {}",
            style("::").cyan().bold(),
            style(container).yellow().bold());

        let status = Command::new(self.runtime.command())
            .args(["stop", container])
            .stdout(Stdio::null())
            .status()
            .context("Failed to stop container")?;

        if !status.success() {
            return Err(anyhow!("Failed to stop container"));
        }

        Ok(())
    }

    /// Remove a container
    pub fn remove_container(&self, container: &str) -> Result<()> {
        let status = Command::new(self.runtime.command())
            .args(["rm", "-f", container])
            .stdout(Stdio::null())
            .status()
            .context("Failed to remove container")?;

        if !status.success() {
            return Err(anyhow!("Failed to remove container"));
        }

        Ok(())
    }

    /// Remove an image
    pub fn remove_image(&self, image: &str) -> Result<()> {
        println!("{} Removing image: {}",
            style("::").cyan().bold(),
            style(image).yellow().bold());

        let status = Command::new(self.runtime.command())
            .args(["rmi", image])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to remove image")?;

        if !status.success() {
            return Err(anyhow!("Failed to remove {}", image));
        }

        Ok(())
    }

    /// Build image from Dockerfile
    pub fn build(&self, path: &str, tag: &str) -> Result<()> {
        println!("{} Building image: {}",
            style("::").cyan().bold(),
            style(tag).yellow().bold());

        let status = Command::new(self.runtime.command())
            .args(["build", "-t", tag, path])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to build image")?;

        if !status.success() {
            return Err(anyhow!("Build failed"));
        }

        println!("{} {} built",
            style("::").green().bold(),
            style(tag).white().bold());

        Ok(())
    }

    /// Install Arch packages in a container
    pub fn install_packages_in_container(&self, container: &str, packages: &[String]) -> Result<()> {
        println!("{} Installing packages in container {}...",
            style("::").cyan().bold(),
            style(container).yellow().bold());

        // First update package database
        self.exec(container, &["pacman".to_string(), "-Sy".to_string()])?;

        // Install packages
        let mut install_args = vec!["pacman".to_string(), "-S".to_string(), "--noconfirm".to_string()];
        install_args.extend(packages.iter().cloned());

        self.exec(container, &install_args)?;

        println!("{} Packages installed in container",
            style("::").green().bold());

        Ok(())
    }

    /// Create an Arch Linux container
    pub fn create_arch_container(&self, name: &str) -> Result<()> {
        println!("{} Creating Arch Linux container: {}",
            style("::").cyan().bold(),
            style(name).yellow().bold());

        // Pull Arch Linux image
        self.pull("archlinux:latest")?;

        // Create container
        let status = Command::new(self.runtime.command())
            .args(["create", "--name", name, "-it", "archlinux:latest", "/bin/bash"])
            .stdout(Stdio::null())
            .status()
            .context("Failed to create container")?;

        if !status.success() {
            return Err(anyhow!("Failed to create container"));
        }

        println!("{} Container {} created",
            style("::").green().bold(),
            style(name).white().bold());

        Ok(())
    }

    /// Get system info
    pub fn info(&self) -> Result<String> {
        let output = Command::new(self.runtime.command())
            .args(["info"])
            .output()
            .context("Failed to get container info")?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Prune unused resources
    pub fn prune(&self) -> Result<()> {
        println!("{} Pruning unused container resources...",
            style("::").cyan().bold());

        let status = Command::new(self.runtime.command())
            .args(["system", "prune", "-f"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to prune")?;

        if !status.success() {
            return Err(anyhow!("Prune failed"));
        }

        Ok(())
    }
}

/// Display images in a table
pub fn display_images(images: &[ContainerImage]) {
    use comfy_table::{Table, Cell, Color, presets::UTF8_FULL};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(Color::Cyan),
        Cell::new("Repository").fg(Color::Cyan),
        Cell::new("Tag").fg(Color::Cyan),
        Cell::new("Created").fg(Color::Cyan),
        Cell::new("Size").fg(Color::Cyan),
    ]);

    for img in images {
        table.add_row(vec![
            Cell::new(&img.id[..12.min(img.id.len())]).fg(Color::Yellow),
            Cell::new(&img.repository).fg(Color::White),
            Cell::new(&img.tag).fg(Color::Green),
            Cell::new(&img.created).fg(Color::DarkGrey),
            Cell::new(&img.size).fg(Color::Magenta),
        ]);
    }

    println!("{}", table);
}

/// Display containers in a table
pub fn display_containers(containers: &[Container]) {
    use comfy_table::{Table, Cell, Color, presets::UTF8_FULL};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("ID").fg(Color::Cyan),
        Cell::new("Image").fg(Color::Cyan),
        Cell::new("Status").fg(Color::Cyan),
        Cell::new("Names").fg(Color::Cyan),
    ]);

    for cont in containers {
        let status_color = if cont.status.contains("Up") { Color::Green } else { Color::Yellow };

        table.add_row(vec![
            Cell::new(&cont.id[..12.min(cont.id.len())]).fg(Color::Yellow),
            Cell::new(&cont.image).fg(Color::White),
            Cell::new(&cont.status).fg(status_color),
            Cell::new(&cont.names).fg(Color::Blue),
        ]);
    }

    println!("{}", table);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_detect() {
        // This will return None if neither Docker nor Podman is installed
        let _ = ContainerRuntime::detect();
    }

    #[test]
    fn test_runtime_command() {
        assert_eq!(ContainerRuntime::Docker.command(), "docker");
        assert_eq!(ContainerRuntime::Podman.command(), "podman");
    }

    #[test]
    fn test_runtime_display() {
        assert_eq!(format!("{}", ContainerRuntime::Docker), "Docker");
        assert_eq!(format!("{}", ContainerRuntime::Podman), "Podman");
    }
}
