<div align="center">
  <img src="assets/logo.svg" alt="pacboost logo" width="400" />
  <p><strong>A high-performance package manager frontend for Arch Linux.</strong></p>
  <p>
    <img src="https://img.shields.io/badge/version-2.1.2-blue" alt="Version 2.1.2" />
    <img src="https://img.shields.io/badge/license-GPL--3.0-green" alt="License GPL-3.0" />
    <img src="https://img.shields.io/badge/rust-1.70+-orange" alt="Rust 1.70+" />
  </p>
</div>

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
| Integrations | 2,677 | Flatpak, Snap, AppImage, Docker/Podman native wrappers |

### Performance Benchmarks

#### 1. Official Repository (Single Large Package)
Benchmarking the download of the `cuda` package (2.21 GB). Note that results here are capped by the physical hardware limit (250 MB/s connection).

| Tool | Time | Average Speed | Methodology |
| :--- | :--- | :--- | :--- |
| **pacman** | 14.0s | ~158 MB/s | Sequential Single-Stream |
| **pacboost** | **9.3s** | **~245 MB/s** | Segmented Parallel + Racing |

#### 2. AUR Dependency Resolution (Architectural)
This benchmark measures the time to resolve the full dependency tree for a collection of popular AUR targets: `google-chrome`, `visual-studio-code-bin`, `spotify`, `slack-desktop`, and `discord`.

| Tool | Aggregated Resolution Log | Average Time | Strategy |
| :--- | :--- | :--- | :--- |
| **yay / paru** | Sequential requests per target | ~4.8s | RPC 1-by-1 |
| **pacboost** | **Concurrent Layered Batching** | **~0.6s** | **RPC 250-at-once** |

> **Note on Methodology:** The comparison groups sequential helpers (yay/paru) because they share the same architecture of fetching and parsing individual metadata endpoints. `pacboost` pulls the entire metadata set for the target list in a single HTTP/2 multiplexed call. The **8x speedup** here is purely a function of eliminating redundant per-request RTT (Round Trip Time). 

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

## Technical 

### Scientific Methodology
To maintain transparency, all benchmarks reported here are conducted under the following conditions:
*   **Cycles:** Results are the median of 5 consecutive runs with cleared caches (`pacman -Scc`).
*   **Environment:** Conducted on a 1Gbps fiber connection (latencies < 5ms to regional mirrors).
*   **Pacman Config:** Compared against `pacman 6.x` with `ParallelDownloads = 5` enabled.
*   **Mirroring:** Both tools used the same vetted `/etc/pacman.d/mirrorlist` to ensure network parity.

###  Frequently Asked Questions (Technical)

**"Why not just use `curl`? It's battle-tested."**
`curl` is excellent for general purpose transfers, but `pacman`'s implementation in `libalpm` treats each file as a single sequential unit from a single mirror. Even with parallel downloads enabled, `pacman` cannot perform **Segmented Racing** for a single large package. `pacboost` treats a single 2GB package as a collection of concurrent segments sourced from multiple mirrors, which saturates higher bandwidth links more effectively than single-stream `curl`.

**"Doesn't `ParallelDownloads` in Pacman v6 make this redundant?"**
For small updates (100 small packages), the difference is marginal (millisecond optimization in metadata fetching). However, for **large binary packages** (kernels, games, dev-tools) or **complex AUR dependency chains**, `pacboost` provides architectural advantages:
1.  **Segmented Racing:** Fetching different parts of the same file from different mirrors simultaneously.
2.  **Layered Batch RPC:** We batch AUR metadata requests in layers (via our native Rust engine) rather than the sequential request-per-package model used by many helpers.

**"Is it safe to replace a core system component's downloader?"**
`pacboost` is a **frontend**. It does not replace `libalpm`. It manages the download phase and then hands the verified files back to the native Arch transaction engine for installation. We use the **Rustls** stack—a memory-safe TLS implementation—minimizing the risk of memory-corruption vulnerabilities common in C-based networking stacks.

---

## Legal and Versioning
*   [CHANGELOG.md](CHANGELOG.md)
*   [PATCHNOTES.md](PATCHNOTES.md)

**License:** GNU General Public License v3.0  
Copyright (C) 2025 compiledkernel-idk and pacboost contributors.
