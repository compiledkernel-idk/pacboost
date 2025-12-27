# Pacboost

<img src="assets/logo.svg" alt="Pacboost logo" width="200">

Blazing fast pacman wrapper

`pacboost` `pacboost-bin`

## Description

Pacboost is a blazing fast pacman wrapper that saturates your bandwidth. It downloads packages from multiple mirrors simultaneously, pulling chunks of the same file in parallel. Think of it as pacman on steroids.

Built in Rust with a custom download engine. Your official packages are still verified with the same GPG keys pacman uses.

## Installation

```bash
# installl script (fastest)
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash


```

### Build from source
```bash
sudo pacman -S --needed base-devel rust
git clone https://github.com/compiledkernel-idk/pacboost.git
cd pacboost
cargo build --release
sudo install -Dm755 target/release/pacboost /usr/bin/pacboost
```

## Examples

`pacboost <target>` — Search and install `<target>` (repos then AUR).

`pacboost` — Alias for `pacboost -Syu`.

`pacboost -S <target>` — Install a specific package.

`pacboost -Syu` — Full system upgrade (official repos + AUR).

`pacboost -Syy` — Force refresh all databases (sync repositories).

`pacboost -R <target>` — Remove a package.

`pacboost -Rr <target>` — Remove a package and its orphaned dependencies.

`pacboost --benchmark` — Test your mirror speeds.

`pacboost --security-scan <file>` — Scan a PKGBUILD for malware.

`pacboost --check-cve` — Check your system for known vulnerabilities.

`pacboost --snapshots` — List available system snapshots.

`pacboost --rollback-to <id>` — Restore to a previous snapshot.

## How It Works

Pacboost uses two levels of parallelism:

1. **Chunk-level**: For each package, it splits the download into segments and pulls them from different mirrors at the same time.

2. **Package-level**: All packages in your install queue download in parallel.

This means if you're installing 50 packages, all 50 start downloading immediately, each one racing across multiple mirrors. Your bandwidth gets saturated.

## Features

- **Mirror Racing** — Pulls file segments from multiple mirrors simultaneously
- **AUR Support** — Builds AUR packages with automatic dependency resolution
- **Malware Scanner** — Parses PKGBUILDs for suspicious code before you build
- **CVE Auditing** — Scans your installed packages for known vulnerabilities
- **Snapshots** — Automatically backs up your system before updates (btrfs)
- **Flatpak/Snap/AppImage** — Manage alternative package formats

## General Tips

- **Drop-in replacement**: Pacboost uses the same flags as pacman. 

- **Safety first**: This is a frontend for libalpm. Your system stays 100% compatible with standard pacman. If something breaks, just use `pacman` directly.

- **AUR packages**: Pacboost automatically drops privileges when building AUR packages. Run it with `sudo`.



## Debugging

Pacboost is not an official Arch tool. If pacboost can't build a package, first check if `makepkg` can build it successfully. If makepkg fails, report the issue to the package maintainer. Otherwise, it's likely a pacboost issue and should be reported here.



## License

GPL-3.0
