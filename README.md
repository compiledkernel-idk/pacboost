# pacboost
## Install
### Using yay
```bash
yay -S pacboost-bin
```
# Or use the script
```bash
curl -sL [https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh](https://raw.githubusercontent.com/compiledkernel-idk/pacboost/master/install.sh) | bash
```

## What is this?
I made this because pacman is slow. It downloads one file at a time from one mirror. If you have a fast connection, you get capped by the mirror speed.
Pacboost uses a custom Rust engine to "race" mirrors. It pulls chunks of the same file from multiple mirrors at once. It is built to saturate your bandwidth.
It is a frontend for libalpm. Your official packages are still verified by the same GPG keys pacman uses.
Why 17,000 lines?
This isn't a 100-line python script that just calls curl. It is a full engine written in Rust. It needs that code to handle:
 * Mirror Racing: Pulling file segments from different sources at the same time.
 * AUR Malware Scanner: It actually parses PKGBUILDs for bad code or hidden network calls before you build them.
 * CVE Audit: Scans your system for known vulnerabilities.
 * Snapshots: Automatically backs up your system before an update.
Commands
 * pacboost -Syu : Update your system (Official + AUR)
 * pacboost <name> : Search and install anything
 * pacboost --security-scan <file> : Run the malware scanner
 * pacboost --check-cve : Scan for vulnerabilities
 * pacboost --benchmark : Test your mirror speeds
Safety
This is a wrapper. It uses the official libalpm to talk to your database. Your system stays 100% compatible with standard pacman. If pacboost fails, just use pacman.