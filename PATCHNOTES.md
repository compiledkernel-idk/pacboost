# PACBOOST v2.0.0 - PATCHNOTES 🎄

**Release Date:** December 25, 2025

---

## 🚀 THE BIG ONE - Complete Feature Expansion

This is the largest update to PACBOOST ever, adding **~9,500 lines** of new Rust code across **21 new source files**.

---

## ✨ New Features

### 📺 Interactive TUI Dashboard
Launch with `pacboost -T` or `pacboost --tui`

- Real-time system monitoring (CPU, memory, disk usage)
- Package browser with search functionality
- Download queue with progress visualization
- Settings configuration panel
- Vim-style keyboard navigation (h/j/k/l)
- Tab-based navigation between views

### 📦 External Package Manager Integration

#### Flatpak Support
```bash
pacboost --flatpak-list           # List installed Flatpaks
pacboost --flatpak-install <app>  # Install Flatpak app
pacboost --flatpak-remove <app>   # Remove Flatpak app
pacboost --flatpak-search <query> # Search Flatpak apps
pacboost --flatpak-update         # Update all Flatpaks
```

#### Snap Support
```bash
pacboost --snap-list              # List installed Snaps
pacboost --snap-install <name>    # Install Snap
pacboost --snap-remove <name>     # Remove Snap
pacboost --snap-search <query>    # Search Snaps
pacboost --snap-refresh           # Refresh all Snaps
```

#### AppImage Support
```bash
pacboost --appimage-list          # List installed AppImages
pacboost --appimage-install <url> # Install AppImage from URL
pacboost --appimage-remove <name> # Remove AppImage
```

### 🔒 Security Hardening

#### CVE Vulnerability Checking
```bash
pacboost --check-cve              # Check for known vulnerabilities
```
Integrates with Arch Security advisories to check installed packages.

#### PKGBUILD Security Scanning
```bash
pacboost --security-scan /path/to/PKGBUILD
```
Advanced malware detection with 30+ threat patterns:
- Remote code execution detection
- Cryptominer detection
- Backdoor patterns
- Data exfiltration attempts
- Obfuscation analysis

#### Sandboxed AUR Builds
```bash
pacboost --sandbox -S aur-package
```
Builds AUR packages in isolated environments using bubblewrap or firejail.

### ⏪ System Rollback (Btrfs)

```bash
pacboost --snapshot               # Create snapshot before operation
pacboost --snapshots              # List all snapshots
pacboost --rollback-to 5          # Rollback to snapshot ID 5
```

### 📋 Lock Files for Reproducible Builds

```bash
pacboost --lock                   # Generate lock file
pacboost --lock-diff              # Compare current state to lock
```

### 📊 Smart Caching

```bash
pacboost --cache-stats            # View cache statistics
```
- LRU eviction
- SHA256 deduplication
- Hit rate tracking

---

## 🏗️ Architecture

### New Modules
| Module | Files | Description |
|--------|-------|-------------|
| `flatpak/` | 2 | Flatpak client and remote management |
| `snap/` | 2 | Snap client and store API |
| `appimage/` | 1 | AppImage manager with desktop integration |
| `containers/` | 1 | Docker/Podman support |
| `tui/` | 5 | Interactive dashboard |
| `security/` | 5 | Malware, CVE, sandbox, trust scoring |
| `deps/` | 3 | Dependency graph and lock files |
| `rollback/` | 1 | Btrfs snapshot management |
| `downloader/cache.rs` | 1 | Smart package caching |

### Statistics
- **New files:** 21
- **New lines:** ~9,500
- **New CLI flags:** 25+
- **Unit tests:** 71 (all passing)

---

## 🔧 Technical Details

### Dependencies Added
- `ratatui` - TUI framework
- `crossterm` - Terminal handling
- `sysinfo` - System metrics
- `sha2` - Cryptographic hashing
- `chrono` - Date/time handling
- `governor` - Rate limiting
- `which` - Binary detection
- `hex` - Hex encoding

### Requirements
- Rust 1.70+
- Arch Linux (or derivatives)
- Optional: `flatpak`, `snap`, `bwrap`/`firejail` for respective features

---

## 📝 Upgrade Instructions

### From AUR
```bash
yay -S pacboost
# or
yay -S pacboost-bin
```

### Quick Install
```bash
curl -sSL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```

---

## 🎁 Holiday Release

This version was released on **Christmas Day 2025** 🎄

Thank you for using PACBOOST!
