<div align="center">
  <h1>pacboost</h1>
  <p><strong>The fastest way to install packages on Arch Linux.</strong></p>
</div>

<hr />

## Why pacboost?

Standard pacman downloads packages one by one. If you have a fast internet connection, you are wasting time waiting for serial downloads. 

<strong>pacboost</strong> changes that. It parallelizes everything. It utilizes the [kdownload download manager](https://github.com/compiledkernel-idk/kdownload) to download multiple packages and databases at the same time, making it <strong>2x to 8x faster</strong> than standard pacman.

## Key Features

<ul>
  <li><strong>Parallel Downloads:</strong> Maximum speed for every update using a native async downloader.</li>
  <li><strong>AUR Support:</strong> Search and install packages from the Arch User Repository.</li>
  <li><strong>Mirror Ranking:</strong> (Beta) Automatically find and sort the fastest mirrors.</li>
  <li><strong>Orphan Cleaning:</strong> (Beta) Detect and remove unused dependencies.</li>
  <li><strong>Detailed Info:</strong> (Beta) View comprehensive package details.</li>
  <li><strong>System Health:</strong> Built-in diagnostics for systemd services, disk space, and symlinks.</li>
  <li><strong>Arch News:</strong> Read the latest Arch Linux news directly from your terminal.</li>
  <li><strong>Package History:</strong> Quickly view your recent installation and upgrade history.</li>
  <li><strong>Auto-Repair:</strong> Automatically fixes database locks and corrupted files.</li>
  <li><strong>Simple UI:</strong> Clean progress bars and easy-to-read tables.</li>
</ul>

> ⚠️ **Beta Branch**: You are viewing the beta branch. Features here are experimental. Use with caution.

## Installation

### Stable Release
```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```

### Beta Release (Experimental)
```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/beta/install-beta.sh | bash
```

### Build from Source
If you prefer to build it yourself, ensure you have `rust`, `base-devel`, and `pkgconf` installed:

```bash
git clone -b beta https://github.com/compiledkernel-idk/pacboost.git
cd pacboost
cargo build --release
sudo cp target/release/pacboost /usr/local/bin/
```

## How to use it

Use it just like pacman. It supports all the main commands:

### Sync databases and update system
```bash
sudo pacboost -Syu
```

### Install a package
```bash
sudo pacboost -S <package_name> # Sync DB, if not found, it'll switch to Aur
```

### Search for a package (Sync DB + AUR)
```bash
pacboost -Ss <query> # Sync DB + Aur
pacboost -A <query>  # Aur only
```

### Advanced Features (Beta)
```bash
sudo pacboost --rank-mirrors   # Find fastest mirrors
sudo pacboost --clean-orphans  # Remove unused dependencies
pacboost --info <package>      # View package details
```

### System Utilities
```bash
pacboost --news      # Read Arch News
pacboost --history   # View package history
pacboost --health    # Run system health check
sudo pacboost --clean # Clean package cache
```

<hr />

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a full list of changes.

## Licensing

**pacboost** is licensed under the **GNU General Public License v3.0**.  
Copyright (C) 2025 compiledkernel-idk, NacreousDawn596 and other pacboost contributors.
