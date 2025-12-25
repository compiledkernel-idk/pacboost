# PACBOOST v2.1.0 - PATCHNOTES

**Release Date:** December 25, 2025

---

##  Bug Fixes

### Btrfs Snapshot Improvements

Fixed critical issue with snapshot creation on systems using snapper or other snapshot managers.

**The Problem:**
- Pacboost would always try to create snapshot ID 1
- On systems with existing snapshots (from snapper), this caused "File exists" errors
- Error messages were unclear about what went wrong

**The Fix:**
- Now scans ALL directories in `/.snapshots` to find the highest existing ID
- Creates new snapshots with the next available ID (e.g., ID 28 after existing 1-27)
- Better error messages with actionable guidance

### Improved Error Messages

Before:
```
Error: Read-only file system (os error 30)
```

After:
```
Cannot create snapshot: filesystem is read-only.
Your btrfs setup may not support snapshots from the running system.
Consider using snapper or timeshift for btrfs snapshots.
```

---

##  Enhancements

### Root Subvolume Detection
```
:: Detected root subvolume: /@
```
Now displays the actual btrfs subvolume path when creating snapshots.

### Preflight Checks
Before creating a snapshot, pacboost now verifies:
- Btrfs filesystem is present on root
- `/.snapshots` directory exists and is writable
- `btrfs-progs` is installed

---

##  Full Changes

- Fixed snapshot ID detection to scan existing directories
- Added `check_snapshot_setup()` preflight validation
- Added `detect_root_subvolume()` to identify mount configuration
- Improved error messages with specific remediation steps
- Better compatibility with snapper-managed btrfs systems

---

##  Upgrade

```bash
# From AUR
yay -S pacboost-bin

# Or quick install
curl -sL https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh | bash
```
