# PACBOOST v2.1.1 - PATCHNOTES

**Release Date:** December 25, 2025

---

## 📝 Documentation & Metadata Polish

This patch release focuses on refining the project's public presence and ensuring version consistency across all components.

---

## ✨ Enhancements

### Professional README Overhaul
The project README has been completely rewritten with a focus on technical clarity:
- **Architecture Deep-dive:** Clarified how the native Rust async engine (Tokio/Reqwest) replaces legacy `curl` bottlenecks.
*   **Speed Claims Explained:** Added a "Custom Project Scale" section to justify the 2x-8x speedups (parallel AUR fetching, segmented mirror racing).
*   **Tone Adjustment:** Removed excessive emojis and "AI-like" phrasing in favor of a professional, developer-centric tone.
*   **Visual Restoration:** Restored the project logo and centered branding while maintaining a clean aesthetic.

### Version Consistency
- Synchronized versioning across `Cargo.toml`, `src/main.rs`, `PKGBUILD`, and `install.sh`.
- Updated all badges and metadata to reflect the current state of the engine.

---

## 🔄 Upgrade

```bash
# From AUR (Recommended)
yay -S pacboost-bin

# Quick Install Script
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```
