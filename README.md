# pacboost

High-performance Arch Linux package manager frontend written in Rust.

## Overview

pacboost is a low-level frontend for libalpm focusing on speed, parallelization, and minimal output. It leverages kdownload for high-speed parallel fetching and provides a robust alternative to standard pacman operations.

## Features

- Parallel package and database synchronization.
- Automated stale lock handling and local database repair.
- Recursive removal and system upgrade support.
- Minimalist high-performance CLI interface.
- Checksum verification for cached packages.

## Usage

### System Upgrade
```bash
sudo pacboost -Syu
```

### Installation
```bash
sudo pacboost -S <package>
```

### Removal
```bash
sudo pacboost -Rs <package>
```

### Search
```bash
pacboost -Ss <query>
```

### Customization
Set the number of parallel download jobs:
```bash
sudo pacboost -S <package> --jobs 8
```

## Licensing

This project is licensed under the GNU General Public License v3.0.
See the LICENSE file for details.
