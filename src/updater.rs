/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use anyhow::Result;
use console::style;
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};
use tar::Archive;

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

pub struct UpdateInfo {
    pub version: String,
    pub pacboost_url: Option<String>,
}

pub fn check_for_updates(current_version: &str) -> Option<UpdateInfo> {
    let url = "https://api.github.com/repos/compiledkernel-idk/pacboost/releases/latest";

    let response = ureq::get(url)
        .set("User-Agent", "pacboost-updater")
        .timeout(std::time::Duration::from_secs(2))
        .call();

    if let Ok(res) = response {
        if let Ok(release) = res.into_json::<GithubRelease>() {
            let latest = release.tag_name.trim_start_matches('v');
            if latest != current_version {
                // Simple SemVer comparison to prevent downgrades
                let parse_ver = |v: &str| -> Option<(u32, u32, u32)> {
                    let parts: Vec<&str> = v.split('.').collect();
                    if parts.len() >= 3 {
                        Some((
                            parts[0].parse().unwrap_or(0),
                            parts[1].parse().unwrap_or(0),
                            parts[2].parse().unwrap_or(0),
                        ))
                    } else {
                        None
                    }
                };

                // Only update if remote is newer
                if let (Some((r_maj, r_min, r_pat)), Some((c_maj, c_min, c_pat))) =
                    (parse_ver(latest), parse_ver(current_version))
                {
                    if r_maj < c_maj {
                        return None;
                    }
                    if r_maj == c_maj && r_min < c_min {
                        return None;
                    }
                    if r_maj == c_maj && r_min == c_min && r_pat <= c_pat {
                        return None;
                    }
                }

                let mut info = UpdateInfo {
                    version: latest.to_string(),
                    pacboost_url: None,
                };
                for asset in release.assets {
                    if asset.name.contains("linux") && asset.name.ends_with(".tar.gz") {
                        info.pacboost_url = Some(asset.browser_download_url);
                    }
                }
                return Some(info);
            }
        }
    }
    None
}

pub fn perform_update(info: UpdateInfo) -> Result<()> {
    println!("{}", style(":: starting update...").bold().cyan());

    let current_pacboost = std::env::current_exe()?;

    if let Some(url) = info.pacboost_url {
        update_binary_from_tarball("pacboost", &url, &current_pacboost)?;
    }

    println!("{}", style(":: update complete.").green().bold());
    Ok(())
}

fn update_binary_from_tarball(name: &str, url: &str, target: &std::path::Path) -> Result<()> {
    print!("   fetching {}... ", name);
    io::stdout().flush()?;

    let response = ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("failed to download {}: {}", name, e))?;

    // Read the response body into a buffer
    let mut reader = response.into_reader();
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    let tar = GzDecoder::new(std::io::Cursor::new(buffer));
    let mut archive = Archive::new(tar);

    // Find the binary in the archive
    let mut found = false;
    let temp_target = target.with_extension("tmp");

    for file in archive.entries()? {
        let mut file = file?;
        let path = file.path()?;
        if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            file.unpack(&temp_target)?;
            found = true;
            break;
        }
    }

    if !found {
        return Err(anyhow::anyhow!("binary not found in update archive"));
    }

    // Set permissions (executable)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&temp_target)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&temp_target, perms)?;
    }

    // Replace binary
    fs::rename(&temp_target, target).map_err(|e| {
        if e.kind() == io::ErrorKind::PermissionDenied {
            anyhow::anyhow!("permission denied: run with sudo to update")
        } else {
            anyhow::anyhow!("failed to replace {}: {}", name, e)
        }
    })?;

    println!("{}", style("done").green());
    Ok(())
}
