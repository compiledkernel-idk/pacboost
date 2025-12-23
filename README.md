<div align="center">
  <h1>pacboost</h1>
  <p><strong>The fastest way to install packages on Arch Linux.</strong></p>
</div>

<hr />

## Why pacboost?

Standard pacman downloads packages one by one. If you have a fast internet connection, you are wasting time waiting for serial downloads. 

<strong>pacboost</strong> changes that. It parallelizes everything. It utilizes the [kdownload download manager](https://github.com/compiledkernel-idk/kdownload) to download multiple packages and databases at the same time, making it <strong>2x to 8x faster</strong> than standard pacman.

## Key Features

<ul>
  <li><strong>Parallel Downloads:</strong> Maximum speed for every update.</li>
  <li><strong>Auto-Repair:</strong> Automatically fixes database locks and corrupted files.</li>
  <li><strong>Simple UI:</strong> Clean progress bars and easy-to-read tables.</li>
  <li><strong>Self-Updating:</strong> Checks GitHub automatically so you always have the latest version.</li>
</ul>

## Installation

Install pacboost with a single command:

```bash
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```
No further installation needed because it updates automatically.
## How to use it

Use it just like pacman. It supports all the main commands:

### Update your whole system
```bash
sudo pacboost -Syu
```

### Install a package
```bash
sudo pacboost -S <package_name>
```

### Search for a package
```bash
pacboost -Ss <query>
```

### Remove a package
```bash
sudo pacboost -Rs <package_name>
```

<hr />

## Licensing

**pacboost** is licensed under the **GNU General Public License v3.0**.  
Copyright (C) 2025 compiledkernel-idk and pacboost contributors.

The integrated **kdownload** engine is licensed under the **MIT License**.  
Copyright (C) 2025 compiledkernel-idk.
[kdownload Repository](https://github.com/compiledkernel-idk/kdownload)
