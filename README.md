<div align="center">
  <img src="assets/logo.svg" alt="pacboost logo" width="400" />
  <p><strong>A high-performance package manager frontend for Arch Linux.</strong></p>
  <p>
    <img src="https://img.shields.io/badge/version-2.0.0-blue" alt="Version 2.0.0" />
    <img src="https://img.shields.io/badge/license-GPL--3.0-green" alt="License GPL-3.0" />
    <img src="https://img.shields.io/badge/rust-1.70+-orange" alt="Rust 1.70+" />
  </p>
</div>

<hr />

## Why pacboost?

While `pacman` now supports parallel downloads, it remains a single-purpose tool. **pacboost** extends this foundation with integrated AUR support, external package managers, security scanning, and intelligent automation, eliminating the need to juggle multiple package management tools.

### Feature Comparison

| Capability | Pacboost | Pacman |
| :--- | :---: | :---: |
| Install Official Packages | ✅ | ✅ |
| Install AUR Packages | ✅ | ❌ |
| Flatpak/Snap/AppImage | ✅ | ❌ |
| Interactive TUI Dashboard | ✅ | ❌ |
| Segmented Downloads | ✅ | ❌ |
| Race Multiple Mirrors | ✅ | ❌ |
| CVE Vulnerability Scanning | ✅ | ❌ |
| Sandboxed AUR Builds | ✅ | ❌ |
| Btrfs Snapshots/Rollback | ✅ | ❌ |
| View Arch News | ✅ | ❌ |
| Auto-Repair DB Locks | ✅ | ❌ |
| Check System Health | ✅ | ❌ |

### Core Advantages

*   **Unified Package Management:** Install packages from official repos, AUR, Flatpak, Snap, and AppImage through a single interface.
*   **Interactive TUI:** Beautiful terminal dashboard with real-time system metrics, package browsing, and vim-style navigation.
*   **Security First:** CVE vulnerability checking, PKGBUILD malware scanning, and sandboxed builds.
*   **System Rollback:** Create Btrfs snapshots before operations and rollback if something goes wrong.
*   **Automatic Repair:** Detects and resolves stale database locks and corrupted package database entries.
*   **Performance Optimized:** Segmented parallel downloads, multi-mirror racing, and connection pooling for maximum speed.

### Performance Comparison

Benchmark downloading `cuda` (2.21 GB):

| Metric | Pacboost  | Pacman |
| :--- | :--- | :--- |
| **Time** | **9.3s** | 14.0s |
| **Speed** | **~245 MB/s** | ~158 MB/s |
| **Technology** | Segmented Parallel + Racing | Sequential Single-Stream |

**Key speed features:**
- 🚀 Up to 16 parallel connections per download
- 🏎️ Multi-mirror racing with 3-second failover
- 📦 HTTP/2 multiplexing
- 💾 Smart package caching with LRU eviction
- ⚡ Resume support for interrupted downloads

## Key Features

### 📺 Interactive TUI Dashboard
```bash
pacboost -T    # or --tui
```
- Real-time CPU, memory, and disk monitoring
- Package browser with search
- Download queue visualization
- Vim-style keyboard navigation (h/j/k/l)

### 📦 External Package Managers
```bash
# Flatpak
pacboost --flatpak-list           # List installed Flatpaks
pacboost --flatpak-install <app>  # Install Flatpak app
pacboost --flatpak-search <query> # Search Flatpak apps

# Snap
pacboost --snap-list              # List installed Snaps
pacboost --snap-install <name>    # Install Snap package

# AppImage
pacboost --appimage-list          # List installed AppImages
pacboost --appimage-install <url> # Install from URL
```

### 🔒 Security Features
```bash
pacboost --check-cve              # Check for CVE vulnerabilities
pacboost --security-scan PKGBUILD # Scan for malicious patterns
pacboost --sandbox -S package     # Sandboxed AUR build
```

### ⏪ System Rollback (Btrfs)
```bash
pacboost --snapshot               # Create snapshot before operation
pacboost --snapshots              # List all snapshots
pacboost --rollback-to 5          # Rollback to snapshot ID 5
```

### 📋 Reproducible Builds
```bash
pacboost --lock                   # Generate lock file
pacboost --lock-diff              # Compare to lock file
pacboost --cache-stats            # View cache statistics
```

### 🎯 Classic Operations
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

### From AUR (Recommended)

**Using an AUR helper:**
```bash
# Precompiled binary (fastest)
yay -S pacboost-bin

# Or build from source
yay -S pacboost
```

**Manual installation:**
```bash
# Precompiled binary
git clone https://aur.archlinux.org/pacboost-bin.git
cd pacboost-bin
makepkg -si

# Or build from source
git clone https://aur.archlinux.org/pacboost.git
cd pacboost
makepkg -si
```

### Quick Install Script
Install the latest stable release directly:

```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```

### Build from Source
Requirements: `rust` (1.70+), `base-devel`, `pkgconf`

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
pacboost --benchmark           # Benchmark mirror download speeds
```

### System Utilities
```bash
pacboost --news       # Display Arch Linux news
pacboost --history    # View package transaction history
pacboost --health     # Run system health diagnostics
sudo pacboost --clean # Clear package cache
```

## All CLI Options

```
Usage: pacboost [OPTIONS] [TARGETS]...

Core Options:
  -S, --sync                       Sync packages
  -R, --remove                     Remove packages
  -s, --search                     Search packages
  -y, --refresh                    Refresh databases
  -u, --sys-upgrade                System upgrade
  -A, --aur                        Search AUR
  -T, --tui                        Launch TUI dashboard

External Package Managers:
      --flatpak-install <APP>      Install Flatpak app
      --flatpak-remove <APP>       Remove Flatpak app
      --flatpak-search <QUERY>     Search Flatpak apps
      --flatpak-update             Update all Flatpaks
      --flatpak-list               List Flatpak apps
      --snap-install <NAME>        Install Snap package
      --snap-remove <NAME>         Remove Snap package
      --snap-search <QUERY>        Search Snaps
      --snap-refresh               Refresh all Snaps
      --snap-list                  List Snaps
      --appimage-install <URL>     Install AppImage
      --appimage-list              List AppImages
      --appimage-remove <NAME>     Remove AppImage

Security:
      --check-cve                  Check for CVE vulnerabilities
      --security-scan <PATH>       Scan PKGBUILD for threats
      --sandbox                    Enable sandboxed AUR builds

Rollback:
      --snapshot                   Create btrfs snapshot
      --snapshots                  List snapshots
      --rollback-to <ID>           Rollback to snapshot

Utilities:
      --benchmark                  Benchmark mirror speeds
      --rank-mirrors               Rank mirrors by speed
      --clean-orphans              Remove orphan packages
      --info                       Package information
      --news                       Arch Linux news
      --history                    Transaction history
      --health                     System health check
      --clean                      Clear package cache
      --cache-stats                Cache statistics
      --lock                       Generate lock file
      --lock-diff                  Lock file diff
```

<hr />

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a full list of changes.

See [PATCHNOTES.md](PATCHNOTES.md) for v2.0.0 release notes.

## License

**pacboost** is licensed under the **GNU General Public License v3.0**.  
Copyright (C) 2025 compiledkernel-idk, NacreousDawn596 and other pacboost contributors.
