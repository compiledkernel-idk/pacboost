<div align="center">
  <img src="assets/logo.svg" alt="pacboost logo" width="400" />
  <p><strong>A high-performance package manager frontend for Arch Linux.</strong></p>
</div>

<hr />

## Why pacboost?

While `pacman` now supports parallel downloads, it remains a single-purpose tool. **pacboost** extends this foundation with integrated AUR support, system diagnostics, and intelligent automation, eliminating the need to juggle multiple package management tools.

### Core Advantages

*   **Unified Package Management:** Install both official repository packages and AUR packages through a single interface. No need to switch between `pacman`, `yay`, or `paru`.
*   **Automatic Repair:** Detects and resolves stale database locks and corrupted package database entries without manual intervention.
*   **Integrated News Reader:** Displays critical Arch Linux news directly in your terminal, helping you avoid system-breaking updates.
*   **System Diagnostics:** Built-in health checks for systemd services, disk space, and broken symlinks.
*   **Performance Optimized:** Multi-mirror failover, connection pooling, and parallel AUR fetching for maximum speed.

## Key Features

<ul>
  <li><strong>Multi-Mirror Downloads:</strong> Automatically races multiple mirrors with 3-second failover for optimal download speeds.</li>
  <li><strong>Native AUR Support:</strong> Search, inspect, and install AUR packages with automatic dependency resolution and privilege handling.</li>
  <li><strong>Mirror Ranking:</strong> Automatically test and rank mirrors by connection speed.</li>
  <li><strong>Orphan Management:</strong> Detect and remove packages that were installed as dependencies but are no longer required.</li>
  <li><strong>Package Inspection:</strong> View comprehensive metadata including dependencies, size, and build information.</li>
  <li><strong>System Health Checks:</strong> Instant diagnostics for systemd services, disk usage, and symlink integrity.</li>
  <li><strong>News Integration:</strong> Fetch the latest Arch Linux news RSS feed before system upgrades.</li>
  <li><strong>Transaction History:</strong> Review recent package installations, upgrades, and removals.</li>
  <li><strong>Smart Repair:</strong> Automatic detection and resolution of database locks and corrupted files.</li>
</ul>

## Installation

### Quick Install
Install the latest stable release:

```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/refs/tags/v1.2.0/install.sh | bash
```

### Build from Source
Requirements: `rust`, `base-devel`, `pkgconf`

```bash
git clone https://github.com/compiledkernel-idk/pacboost.git
cd pacboost
cargo build --release
sudo cp target/release/pacboost /usr/local/bin/
```

## Usage

Pacboost uses pacman-compatible flags for a familiar experience.

### System Updates
```bash
sudo pacboost -Syu
```

### Package Installation
Automatically searches official repositories first, then falls back to AUR:
```bash
sudo pacboost -S firefox spotify
```

### Package Search
```bash
pacboost -Ss <query>  # Search repositories and AUR
pacboost -A <query>   # Search AUR only
```

### Advanced Operations
```bash
sudo pacboost --rank-mirrors   # Test and rank mirrors by speed
sudo pacboost --clean-orphans  # Remove orphaned dependencies
pacboost --info <package>      # Display detailed package information
```

### System Utilities
```bash
pacboost --news       # Display Arch Linux news
pacboost --history    # View package transaction history
pacboost --health     # Run system health diagnostics
sudo pacboost --clean # Clear package cache
```

<hr />

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a full list of changes.

## License

**pacboost** is licensed under the **GNU General Public License v3.0**.  
Copyright (C) 2025 compiledkernel-idk, NacreousDawn596 and other pacboost contributors.
