# pacboost

[![Version](https://img.shields.io/badge/version-2.1.0-blue)](https://github.com/compiledkernel-idk/pacboost/releases)
[![License](https://img.shields.io/badge/license-GPL--3.0-green)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange)](https://www.rust-lang.org/)

A high-performance package manager frontend for Arch Linux.

---

## Architectural Philosophy

Pacboost was developed to address limitations in standard package management workflows on Arch Linux. While `pacman` is highly reliable, its network stack often underutilizes modern high-speed connections by relying on single-stream sequential downloads from a single mirror at a time.

### Custom Project Scale

This project represents a significant engineering effort with **13,609 lines of custom Rust code** across **35 source files**.

| Category | Lines | Key Components |
| :--- | :--- | :--- |
| **AUR & Resolution** | 2,085 | Custom dependency graph, topological solver, PGP automation |
| **Security Suite** | 1,879 | 30+ malware threat patterns, entropy analysis, CVE tracker |
| **TUI Framework** | 1,794 | Custom widgets (sparklines, circular progress), async event loop |
| **Download Engine** | 1,601 | Segmented mirror racing, connection pooling, LRU cache |
| **Integrations** | 2,677 | Flatpak, Snap, AppImage, Docker/Podman native wrappers |

---

## Custom Implementation Highlights

### 1. High-Performance Download Engine
Unlike wrapper-based tools, pacboost implements a native asynchronous downloader in Rust.
*   **Segmented Racing:** Large files are transparently split into segments and requested from multiple mirrors simultaneously.
*   **Intelligent Failover:** Mirrors that fail to respond within 3 seconds are automatically deprioritized and replaced in real-time.
*   **Deduplicated Cache:** A custom LRU cache with SHA256 integrity checks prevents redundant network traffic.

### 2. Advanced Security Scanning
The security module (`src/security/malware.rs`) implements a sophisticated validation engine for PKGBUILDs.
*   **Threat Patterns:** Scans for obfuscated code, remote execution patterns, cryptominers, and unauthorized data exfiltration.
*   **Maintainer Trust Scoring:** A custom algorithm calculates maintainer reputation based on historical data and package volume.
*   **Sandboxing:** Native integration for bubblewrap and firejail for isolated build environments.

### 3. Interactive TUI System
The dashboard is built using a custom event-handling architecture that keeps the UI responsive during heavy IO.
*   **Non-blocking Metrics:** Real-time system monitoring (CPU/RAM/Disk/Uptime) runs on a dedicated background thread.
*   **Rich Widgets:** Custom-built terminal widgets for data visualization, including resource sparklines and circular progress indicators.

### 4. Btrfs Snapshot & Rollback
Developed to provide enterprise-grade safety for system updates.
*   **Subvolume Detection:** Automatically identifies btrfs subvolume layouts (e.g., `/@`, `@root`) to ensure correct snapshot placement.
*   **Compatibility:** Designed to work alongside existing snapshots from managers like Snapper or Timeshift without ID conflicts.

---

## Technical Features

### Integrated Package Management
Unified interface for multiple package formats and repositories:
*   **AUR:** Native implementation with dependency resolution and automated GPG key retrieval.
*   **Flatpak:** Remote management, installation, and updates.
*   **Snap:** Store integration for search, install, and refresh operations.
*   **AppImage:** Installation and desktop integration from arbitrary URLs.
*   **Containers:** Native Docker and Podman image/container management.

### CLI Usage Examples
```bash
sudo pacboost -Syu        # Synchronize and upgrade (Official + AUR)
sudo pacboost -S <pkg>    # Install from repositories or AUR
pacboost -T               # Launch Interactive TUI Dashboard
pacboost --check-cve      # Audit system for known vulnerabilities
pacboost --security-scan  # Scan PKGBUILD for malicious patterns
pacboost --snapshot       # Create Btrfs snapshot before operation
```

---

## Installation

### AUR
```bash
# Precompiled binary
yay -S pacboost-bin

# Source build
yay -S pacboost
```

### Script
```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```

---

## Legal and Versioning
*   [CHANGELOG.md](CHANGELOG.md)
*   [PATCHNOTES.md](PATCHNOTES.md)

**License:** GNU General Public License v3.0  
Copyright (C) 2025 compiledkernel-idk and pacboost contributors.
