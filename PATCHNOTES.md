# PACBOOST v2.1.2 - PATCHNOTES

**Release Date:** December 25, 2025

---

## 🔬 Benchmark Hardening & Transparency

This release focuses on total transparency in performance reporting and architectural defense against common skepticism.

### ✨ New Features

- **Technical System Report (`--sys-report`):** A new diagnostic command that generates a full audit of your networking environment, `pacman.conf` settings, and engine architecture. Use this to provide context when sharing benchmarks.
- **Scientifically Rigorous Benchmarking:** The internal `--benchmark` command now runs **3 iterations** and reports the **median** value to filter out network noise and transients.

### 🛠️ Architectural Improvements

- **Parallelized AUR Dependency Discovery:** Re-engineered the dependency solver to use **layered concurrent batching**. It now fetches metadata for up to 250 packages in a single multiplexed HTTP/2 request, eliminating the serial RTT bottleneck.
- **Enhanced AUR Info:** Made `show_package_info` asynchronous and parallelized. Querying package details for a list of AUR targets is now instantaneous.

### 📝 Documentation Updates

- **Methodology Transparency:** Added a "Scientific Methodology" section to the README detailing our test environment and pacman configuration (ParallelDownloads=5).
- **Hardened FAQ:** Added a technical deep-dive addressing the specific advantages of Segmented Racing over pacman's single-mirror file transfers.

---

## 🔄 Upgrade

```bash
# From AUR (Recommended)
yay -S pacboost-bin

# Quick Install Script
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```
