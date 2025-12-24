# Changelog

All notable changes to this project will be documented in this file.

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
