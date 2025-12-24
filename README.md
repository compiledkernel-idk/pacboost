<div align="center">
  <img src="assets/logo.svg" alt="pacboost logo" width="400" />
  <p><strong>The modern, battery-included package manager for Arch Linux.</strong></p>
</div>

<hr />

## Why pacboost?

You might ask: *"Standard pacman now supports parallel downloads, why do I need pacboost?"*

While `pacman` is powerful, **pacboost** was built to be the **complete, modern frontend** that Arch users have always wanted. It's not just about speed—it's about removing the friction from your daily Linux administration.

Where `pacman` stops, `pacboost` continues:

*   **Hybrid Power:** Seamlessly handle standard repository packages AND **AUR** packages in unified transactions. No more context switching between different tools.
*   **Self-Healing:** Ever successfully `rm /var/lib/pacman/db.lck`? Pacboost detects stale locks and corrupt database entries and **automatically repairs them** for you.
*   **Safety First:** Reads **Arch Linux News** directly in your terminal so you don't break your system by missing manual interventions.
*   **System Intelligence:** Built-in **health diagnostics** check for failed systemd services, disk space issues, and broken symlinks.
*   **Visual Excellence:** A beautiful, modern CLI with clear progress bars, tables, and colors that make reading package output a joy.

## Key Features

<ul>
  <li><strong>🚀 Unified Parallel Downloads:</strong> Blazing fast repository and database syncing using a native async engine (native rust).</li>
  <li><strong>📦 AUR Support:</strong> Search, inspect, and install AUR packages effortlessly (handling build dependencies and sudo privileges automatically).</li>
  <li><strong>🌐 Smart Mirrors:</strong> Automatically find, rank, and use the fastest mirrors for your connection.</li>
  <li><strong>🧹 Smart Cleaning:</strong> Detect and remove orphaned dependencies to keep your system bloat-free.</li>
  <li><strong>🔍 Deep Inspection:</strong> View comprehensive extended details about any package.</li>
  <li><strong>🩺 System Health:</strong> Instant diagnostics for systemd services, disk capacity, and hygiene check for /usr/bin symlinks.</li>
  <li><strong>📰 Arch News:</strong> Fetch the latest critical news/RSS feeds before you upgrade.</li>
  <li><strong>📜 Package History:</strong> An easy-to-read log of your recent installations, upgrades, and removals.</li>
  <li><strong>⚡ Auto-Repair:</strong> Smart detection and resolution of database locks and corrupted files.</li>
</ul>

## Installation

### Quick Install
Install the latest stable release with a single command:

```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```

### Build from Source
If you are a Rustacean or prefer manual builds (requires `rust`, `base-devel`, `pkgconf`):

```bash
git clone https://github.com/compiledkernel-idk/pacboost.git
cd pacboost
cargo build --release
sudo cp target/release/pacboost /usr/local/bin/
```

## How to use it

Pacboost uses flags similar to pacman, so you already know how to use it.

### Update System
```bash
sudo pacboost -Syu
```

### Install Packages (Repo + AUR)
Pacboost automatically checks repositories first, then falls back to AUR if not found.
```bash
sudo pacboost -S firefox spotify
```

### Search
```bash
pacboost -Ss <query> # Global search (Repo + AUR)
pacboost -A <query>  # AUR specific search
```

### Advanced Features
```bash
sudo pacboost --rank-mirrors   # Find fastest mirrors
sudo pacboost --clean-orphans  # Remove unused dependencies
pacboost --info <package>      # View package details
```

### System Utilities
The tools you didn't know you needed, until now:
```bash
pacboost --news      # Check critical Arch News
pacboost --history   # See what you installed last week
pacboost --health    # Sanity check your system state
sudo pacboost --clean # Clean cached packages to free space
```

<hr />

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a full list of changes.

## Licensing

**pacboost** is licensed under the **GNU General Public License v3.0**.  
Copyright (C) 2025 compiledkernel-idk, NacreousDawn596 and other pacboost contributors.
