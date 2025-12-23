<div align="center">
  <h1>pacboost</h1>
  <p><strong>High-Performance Arch Linux Package Management</strong></p>
  <p>A low-level, parallelized frontend for libalpm engineered for maximum throughput.</p>
</div>

<hr />

## Performance Engineering

pacboost is built for users who demand more from their package manager. By utilizing the <strong>Rust</strong> systems language and the multi-stream execution engine of <strong>kdownload</strong>, pacboost achieves synchronization and download speeds <strong>2 to 8 times faster</strong> than standard pacman.

Standard package managers often leave your high-speed bandwidth underutilized. pacboost eliminates these bottlenecks by parallelizing database synchronization and package fetching, turning a sluggish system update into a near-instant operation.

## Core Capabilities

<ul>
  <li><strong>Extreme Parallelization:</strong> Simultaneous database sync and package fetching via the kdownload engine.</li>
  <li><strong>Automated Self-Repair:</strong> Intelligent detection and removal of stale locks and corrupted local database entries.</li>
  <li><strong>Modern UX:</strong> 
  <li><strong>Continuous Delivery:</strong> Integrated GitHub API tracking for seamless, one-click binary updates.</li>
  <li><strong>Full Compatibility:</strong> Complete replacement for standard installation, upgrade, removal, and search workflows.</li>
</ul>

## Quick Start

Deploy the latest optimized binary to your system with a single command:

```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```

### Build from Source
```bash
cargo build --release
sudo install -Dm755 target/release/pacboost /usr/local/bin/pacboost
```

## Commands

### System Synchronization & Upgrade
```bash
sudo pacboost -Syu
```

### Package Installation
```bash
sudo pacboost -S <package>
```

### Recursive Removal
```bash
sudo pacboost -Rs <package>
```

### Database Search
```bash
pacboost -Ss <query>
```

<hr />

## Licensing

This project is licensed under the <strong>GNU General Public License v3.0</strong>.
Copyright (C) 2025 compiledkernel-idk and pacboost contributors.
