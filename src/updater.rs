/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 */

use serde::Deserialize;
use console::style;
use std::fs;
use std::io::{self, Write};
use anyhow::Result;
use flate2::read::GzDecoder;
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
    println!("{}", style(":: starting automatic update...").bold().cyan());

    let current_pacboost = std::env::current_exe()?;

    if let Some(url) = info.pacboost_url {
        update_binary_from_tarball("pacboost", &url, &current_pacboost)?;
    }

    println!("{}", style(":: update completed successfully.").green().bold());
    Ok(())
}

fn update_binary_from_tarball(name: &str, url: &str, target: &std::path::Path) -> Result<()> {
    print!("   fetching {}... ", name);
    io::stdout().flush()?;

    let response = ureq::get(url).call().map_err(|e| anyhow::anyhow!("failed to download {}: {}", name, e))?;
    
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
            anyhow::anyhow!("permission denied: please run with sudo to update")
        } else {
            anyhow::anyhow!("failed to replace {}: {}", name, e)
        }
    })?;

    println!("{}", style("done").green());
    Ok(())
}