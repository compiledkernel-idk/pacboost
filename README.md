# Feature packed Package Manager Frontend

[pacboost](https://github.com/compiledkernel-idk/pacboost) [pacboost-bin](https://aur.archlinux.org/packages/pacboost-bin)

## Description

Pacboost is a high performance Arch Linux package manager frontend with a powerful Turbo Download Engine, advanced security features, and minimal interaction. Download speeds are typically **2x or more faster** than standard pacman.

It supports official pacman, AUR, Flatpak, Snap, AppImage, Docker and Podman.

- **Turbo Download Engine**: 2x faster than pacman (~240 MB/s vs 100 MB/s) with segmented parallel racing and multi-mirror failover.
- **Unified Management**: Official, AUR, Flatpak, Snap, and AppImage support.
- **Safety First**: Automatic system snapshots, malware scanning, and CVE audits.
- **Detailed Reporting**: Comprehensive transaction summaries with repository info, licenses, and live hook/extraction monitoring.
- **Host Awareness**: Displays system context for transaction transparency.

<img src="assets/logo.svg" alt="pacboost logo" width="400" />

## Installation

### AUR

```bash
yay -S pacboost-bin # or paru
```

### Script

```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```

### Manual

```bash
sudo pacman -S --needed base-devel
git clone https://github.com/compiledkernel-idk/pacboost.git
cd pacboost
makepkg -si
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## General Tips

*   **Security Scanning**: Pacboost can scan PKGBUILDs for malware. Use `pacboost --security-scan <pkgbuild>`.
*   **Benchmarks**: Test mirror speeds with `pacboost --benchmark`.
*   **Snapshots**: Create system snapshots before operations with `pacboost --snapshot`.
*   **Integrations**: Pacboost supports Flatpak, Snap, and AppImages.
*   **CVE Check**: Audit system for known vulnerabilities with `pacboost --check-cve`.
*   **Pacman Fallback**: Pacboost is a frontend wrapper. Your system's `pacman` remains completely untouched and valid as a reliable fallback for any operation.

## Examples

*   `pacboost <target>` -- Interactively search and install `<target>`.
*   `pacboost -S <target>` -- Install a specific package.
*   `pacboost -Syu` -- Synchronize and upgrade (Official + AUR).
*   `pacboost --check-cve` -- Audit system for known vulnerabilities.
*   `pacboost --benchmark` -- Benchmark mirror download speeds.
*   `pacboost --snapshot` -- Create a manual system snapshot.
*   `pacboost --flatpak-install <app_id>` -- Install a Flatpak application.
*   `pacboost --snap-install <name>` -- Install a Snap package.
*   `pacboost --appimage-install <url>` -- Install an AppImage from a URL.

## Debugging

Pacboost is not an official tool. If pacboost can't build a package, you should first check if `pacman` can successfully build the package. If it can't, then you should report the issue to the maintainer. Otherwise, it is likely an issue with pacboost and should be reported [here](https://github.com/compiledkernel-idk/pacboost/issues).
