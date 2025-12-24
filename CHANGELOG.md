# Changelog

All notable changes to this project will be documented in this file.

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
