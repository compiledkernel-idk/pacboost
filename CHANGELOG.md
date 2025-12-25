# Changelog

All notable changes to this project will be documented in this file.

## [2.0.0] - 2025-12-25 🎄

### MAJOR RELEASE - Complete Feature Expansion (~9,500 new lines)

#### New Features

- **Interactive TUI Dashboard** (`-T`, `--tui`)
  - Real-time system metrics (CPU, memory, disk)
  - Package browser with search
  - Download queue visualization
  - Settings panel
  - Vim-style keyboard navigation

- **External Package Manager Integration**
  - **Flatpak**: `--flatpak-install`, `--flatpak-remove`, `--flatpak-search`, `--flatpak-update`, `--flatpak-list`
  - **Snap**: `--snap-install`, `--snap-remove`, `--snap-search`, `--snap-refresh`, `--snap-list`
  - **AppImage**: `--appimage-install`, `--appimage-list`, `--appimage-remove`
  - **Docker/Podman**: Container management module

- **Security Hardening**
  - `--check-cve`: Check installed packages for known vulnerabilities (Arch Security)
  - `--security-scan <PKGBUILD>`: Advanced malware detection with 30+ threat patterns
  - `--sandbox`: Sandboxed AUR builds with bubblewrap/firejail
  - Maintainer trust scoring system

- **System Rollback (Btrfs)**
  - `--snapshot`: Create system snapshot before operations
  - `--snapshots`: List all snapshots
  - `--rollback-to <ID>`: Rollback to previous snapshot

- **Dependency Management**
  - Dependency graph with topological sorting
  - Conflict detection
  - `--lock`: Generate lock file for reproducible builds
  - `--lock-diff`: Show differences from lock file

- **Download Enhancements**
  - Smart package cache with LRU eviction
  - SHA256 deduplication
  - `--cache-stats`: View cache statistics
  - Rate limiting support

#### New Modules (21 files)

```
src/flatpak/mod.rs, remote.rs     (~750 lines)
src/snap/mod.rs, store.rs         (~650 lines)
src/appimage/mod.rs               (~400 lines)
src/containers/mod.rs             (~420 lines)
src/tui/mod.rs, app.rs, ui.rs, widgets.rs, events.rs  (~1,600 lines)
src/security/mod.rs, malware.rs, sandbox.rs, cve.rs, trust.rs  (~1,600 lines)
src/deps/mod.rs, lockfile.rs, solver.rs  (~760 lines)
src/rollback/mod.rs               (~350 lines)
src/downloader/cache.rs           (~350 lines)
```

#### Tests
- 71 unit tests, all passing
- Comprehensive test coverage for new modules

---

## v1.6.0
- **New Feature**: Insanely fast segmented parallel downloader
- **New Feature**: Multi-mirror racing with intelligent failover
- **New Feature**: Adaptive parallelism (up to 16x connections)
- **New Feature**: `--benchmark` flag to test mirror speeds
- **Improvement**: HTTP/2 multiplexing support
- **Improvement**: Resume support for interrupted downloads

## [1.5.2] - 2025-12-24

### Fixed
- **Custom Repository Support**: Fixed critical bug where pacboost only detected standard repos (core, extra, multilib) and ignored custom repositories like liquorix, endeavouros, etc.
- **Repository Detection**: Now properly parses `/etc/pacman.conf` to detect ALL configured repositories and their mirrors.
- **Mirror Support**: Correctly handles both direct `Server` entries and `Include` directives for mirrorlists.

### Changed
- **ALPM Manager**: Replaced hardcoded repository list with dynamic parsing from pacman configuration.

## [1.5.1] - 2025-12-24

### Changed
- **Easter Egg**: Updated the  easter egg message to be more sarcastic and funny.

## [1.5.0] - 2025-12-24

### Added
- **Easter Egg**: Added a fun message when users try to install pacboost using pacboost itself (`sudo pacboost -S pacboost`).
- **AUR Publication**: Published both `pacboost` and `pacboost-bin` to the Arch User Repository.
  - `pacboost`: Builds from latest master branch automatically.
  - `pacboost-bin`: Downloads precompiled binaries from latest GitHub release.

### Changed
- **PKGBUILD Updates**: Both AUR packages now automatically pull from latest sources (no version pinning).
- **README**: Updated installation instructions to prioritize AUR as the recommended method.

## [1.4.3] - 2025-12-24

### Fixed
- **Self-Updater**: Fixed an issue where the automatic updater couldn't find the binary in the release archive due to a naming mismatch.
- **Improved UI**: Notification message for new versions is now more descriptive.

## [1.4.1] - 2025-12-24

### Added
- **Legal Notices**: Added a brief copyright and warranty disclaimer to the `--version` output, as encouraged by the GPL.

## [1.4.0] - 2025-12-24

### Added
- **Complete AUR Subsystem Rewrite**: New modular architecture with 8 new source files (~2,000 lines).
- **Dependency Resolution**: Topological sorting ensures correct build order for AUR packages.
- **Circular Dependency Detection**: Detects and reports dependency cycles before building.
- **PKGBUILD Security Scanning**: Analyzes PKGBUILDs for malicious patterns with severity levels.
- **Automatic PGP Key Importing**: Imports missing GPG keys from multiple keyservers.
- **Configuration System**: TOML-based config at `~/.config/pacboost/config.toml` with environment overrides.
- **Structured Logging**: Tracing integration for debugging with optional file logging.
- **Error Recovery Strategies**: Hierarchical error types with suggested recovery actions.
- **Enhanced UI**: Rich tables, progress bars, download speeds, timing stats, and colored output.

### New Modules
- `src/error.rs` - 15+ error types with recovery strategies
- `src/config.rs` - TOML configuration with validation
- `src/logging.rs` - Tracing integration
- `src/aur/mod.rs` - AUR module structure
- `src/aur/client.rs` - AUR RPC v5 API with LRU caching
- `src/aur/resolver.rs` - Dependency graph with BFS discovery
- `src/aur/pkgbuild.rs` - Security validation with pattern matching
- `src/aur/builder.rs` - Build management with privilege handling

### Fixed
- **Database Lock Handling**: Properly releases locks and removes stale lock files.
- **Virtual Package Resolution**: Dependencies like `mime-types` are now handled correctly.
- **PGP Signature Issues**: Uses `--skippgpcheck` fallback when key import fails.
- **Official Package Detection**: Properly registers sync databases for package lookups.

### Changed
- AUR packages now show votes, popularity, maintainer, and out-of-date status.
- Build process shows MAKEFLAGS, PKGEXT, and per-package timing.
- Install process displays built package sizes.

## [1.3.0] - 2025-12-24

### Added
- **Multi-Mirror Racing**: Download engine now accepts multiple mirror URLs per file and races them with automatic failover.
- **Connection Pooling**: Shared HTTP client with persistent connections eliminates TCP handshake overhead.
- **Parallel AUR Fetching**: All AUR packages are now downloaded and extracted simultaneously before building.
- **Multi-Core Compilation**: AUR builds automatically use all CPU cores via MAKEFLAGS injection.
- **Fast Packaging**: Disabled package compression for local AUR installs to eliminate unnecessary CPU cycles.

### Changed
- **Downloader Architecture**: Completely rewritten download engine with mirror failover and 3-second timeout per mirror.
- **ALPM Integration**: Database sync and package downloads now utilize all available mirrors instead of just the first one.
- **AUR Workflow**: Split AUR installation into separate fetch and build phases for maximum parallelism.

### Performance
- Database sync speed improved through multi-mirror racing.
- AUR installation speed dramatically improved through parallel fetching and multi-core compilation.
- Reduced network latency via connection pooling and keep-alive.

## [1.2.0-beta] - 2025-12-23

### Added
- **Mirror Ranking**: `--rank-mirrors` to sort mirrors by speed.
- **Orphan Cleaning**: `--clean-orphans` to remove unused dependencies.
- **Package Info**: `--info` for detailed package metadata.

### Fixed
- **Updater**: Fixed updater to handle `.tar.gz` archives.
- **CLI**: Fixed argument parsing logic for new flags.

## [1.1.0] - 2025-12-23

### Fixed
- **Updater Loop**: Fixed an issue where the updater looked for a raw binary instead of a tarball.

## [1.0.0] - 2025-12-23

### Added
- **AUR Support**: Search the Arch User Repository directly with `-A` or `--aur`.
- **Package History**: View recent package transactions with `--history`.
- **System Health Check**: Run a quick diagnostic of your system with `--health`.
- **Arch News Reader**: Stay updated with the latest Arch Linux news using `--news`.
- **Cache Cleaner**: Quickly clear the pacman package cache with `--clean`.
- **GitHub Actions**: Automated release workflow for building and publishing binaries.

### Changed
- **Internal Downloader**: Replaced external `kdownload` with a native Rust async downloader using `reqwest` for better performance and reliability.
- **Parallel Syncing**: Optimized database syncing to use multiple concurrent connections.
- **Async Architecture**: Migrated the core logic to use `tokio` for high-concurrency operations.
- **CLI Improvements**: Updated CLI flags and help messages for better usability.

### Fixed
- Improved database lock handling and corruption detection.
