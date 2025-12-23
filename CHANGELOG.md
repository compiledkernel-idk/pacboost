# Changelog

All notable changes to this project will be documented in this file.

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
