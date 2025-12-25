# Feature packed Package Manager Frontend

[pacboost](https://github.com/compiledkernel-idk/pacboost) [pacboost-bin](https://aur.archlinux.org/packages/pacboost-bin)

## Description

Pacboost is a high-performance Arch Linux package manager frontend with advanced security features, TUI dashboard, and minimal interaction.

<img src="assets/logo.svg" alt="pacboost logo" width="400" />

## Installation

### AUR

```bash
yay -S pacboost-bin
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

*   **TUI Dashboard**: Launch the interactive dashboard with `pacboost -T`.
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
*   `pacboost -T` -- Launch Interactive TUI Dashboard.
*   `pacboost --check-cve` -- Audit system for known vulnerabilities.
*   `pacboost --benchmark` -- Benchmark mirror download speeds.
*   `pacboost --snapshot` -- Create a manual system snapshot.
*   `pacboost --flatpak-install <app_id>` -- Install a Flatpak application.
*   `pacboost --snap-install <name>` -- Install a Snap package.
*   `pacboost --appimage-install <url>` -- Install an AppImage from a URL.

## Debugging

Pacboost is not an official tool. If pacboost can't build a package, you should first check if `pacman` can successfully build the package. If it can't, then you should report the issue to the maintainer. Otherwise, it is likely an issue with pacboost and should be reported here.
