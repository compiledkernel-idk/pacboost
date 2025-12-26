pacboost
pacboost | AUR: pacboost-bin
A Rust-based pacman frontend designed to saturate high-bandwidth connections.
pacman downloads packages sequentially from a single mirror. Pacboost uses a custom async engine (Tokio/Reqwest) to "race" mirrors against each other, downloading segments of the same file from multiple sources simultaneously. This typically results in 2x-8x faster downloads by bypassing per-mirror speed caps.
It acts as a wrapper for libalpm, preserving your official database integrity while adding features pacman lacks, specifically for the AUR.
Why use this?
 * Saturate your Bandwidth: If you have gigabit internet but pacman only gives you 5MB/s, this fixes it. It uses segmented downloading to max out your connection.
 * AUR Security: Standard helpers like yay blindly execute PKGBUILDs. Pacboost includes a heuristic malware scanner that parses scripts for suspicious patterns (obfuscation, network calls) before makepkg runs.
 * Unified Wrapper: Handles Official repos, AUR, Flatpak, Snap, and AppImages through a single CLI.
<img src="assets/logo.svg" alt="pacboost logo" width="300" />
Installation
AUR (Recommended)
yay -S pacboost-bin

From Source
Requires Rust and base-devel.
sudo pacman -S --needed base-devel
git clone [https://github.com/compiledkernel-idk/pacboost.git](https://github.com/compiledkernel-idk/pacboost.git)
cd pacboost
makepkg -si

Script (curl | bash)
curl -sL [https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh](https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh) | bash

Core Features
 * Segmented Racing Engine: Splits files into chunks and downloads them from the fastest available mirrors concurrently.
 * PKGBUILD Scanner: pacboost --security-scan <pkgbuild> performs static analysis to detect common malware vectors in AUR packages.
 * System Snapshots: Optional automatic snapshots before major transactions (--snapshot).
 * CVE Auditing: Checks your installed packages against known vulnerability databases (--check-cve).
Common Usage
| Task | Command |
|---|---|
| Install / Search | pacboost <package> |
| Sync & Update | pacboost -Syu |
| Check for CVEs | pacboost --check-cve |
| Benchmark Mirrors | pacboost --benchmark |
| Manual Snapshot | pacboost --snapshot |
Architecture & Debugging
Pacboost is a frontend wrapper. It handles the networking and UI, but hands off the final package installation to libalpm.
 * If a build fails: Check if pacman or makepkg fails on the same package. If pacman works but pacboost doesn't, please open an issue.
 * Database Safety: Since libalpm handles the locking and database writing, your system remains compatible with standard pacman at all times.
Contributing
Pull requests are welcome, especially for improving the malware scanner heuristics or adding new mirror protocols. See CONTRIBUTING.md.