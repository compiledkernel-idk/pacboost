# Patch Notes - v1.2.0-beta

## [1.2.0-beta] - 2025-12-23

### New Features
- **Mirror Ranking**: Added `--rank-mirrors` to automatically find and prioritize the fastest Arch Linux mirrors based on connection latency.
- **Orphan Cleaning**: Added `--clean-orphans` to detect and remove packages that were installed as dependencies but are no longer required.
- **Package Info**: Added `--info <package>` to view comprehensive metadata, including dependencies, size, and build information.

### Improvements & Fixes
- **Smart Updater**: Re-engineered the auto-updater to support `.tar.gz` archive extraction, ensuring smoother updates for future releases.
- **Code Refactoring**: Modularized internal logic by moving mirror ranking and updater functions into dedicated modules (`reflector.rs`, `updater.rs`).
- **CLI Robustness**: Fixed a bug where new flags would trigger the help menu instead of executing the command.
- **Improved Metadata**: Added `libc` dependency for native system calls.

---
*Thank you to all contributors and beta testers!*