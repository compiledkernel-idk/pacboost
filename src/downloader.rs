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

use std::path::Path;
use tokio::process::Command;
use anyhow::{Context, Result, anyhow};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub async fn download_packages(
    urls: Vec<(String, String)>,
    cache_dir: &Path,
    mp: Option<MultiProgress>,
    concurrency: usize,
) -> Result<()> {
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();

    if !cache_dir.exists() {
        tokio::fs::create_dir_all(cache_dir).await.context("failed to create cache directory")?;
    }

    let mp = mp.unwrap_or_else(|| MultiProgress::new());
    
    let total_style = ProgressStyle::with_template(
        "{spinner:.cyan} {msg} [{bar:40.cyan/blue}] {pos}/{len}"
    ).unwrap()
    .progress_chars("=>-");
    
    let main_pb = mp.add(ProgressBar::new(urls.len() as u64));
    main_pb.set_style(total_style);
    main_pb.set_message("fetching");

    for (url, filename) in urls {
        let sem_clone = semaphore.clone();
        let cache_dir_owned = cache_dir.to_path_buf();
        let url_clone = url.clone();
        let filename_clone = filename.clone();
        let main_pb_clone = main_pb.clone();
        let mp_clone = mp.clone();
        
        let handle = tokio::spawn(async move {
            let _permit = sem_clone.acquire().await.unwrap();
            
            let pb = mp_clone.add(ProgressBar::new_spinner());
            pb.set_style(ProgressStyle::with_template("   {spinner:.blue} {msg}").unwrap());
            pb.set_message(format!("resolving: {}", filename_clone));
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let target_path = cache_dir_owned.join(&filename_clone);
            if target_path.exists() {
                let _ = tokio::fs::remove_file(&target_path).await;
            }

            let output = Command::new("/usr/local/bin/kdownload")
                .arg(&url_clone)
                .current_dir(&cache_dir_owned)
                .output()
                .await;

            match output {
                Ok(out) if out.status.success() => {
                    pb.finish_and_clear(); 
                    main_pb_clone.inc(1);
                    Ok(())
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    pb.finish_with_message(format!("error: {} (exit {})", filename_clone, out.status.code().unwrap_or(-1)));
                    Err(anyhow!("kdownload failed for {}: exit {:?}\nstderr: {}", url_clone, out.status.code(), stderr))
                }
                Err(e) => {
                    pb.finish_with_message(format!("fatal: {}", filename_clone));
                    Err(anyhow!("failed to execute kdownload: {}", e))
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }
    
    main_pb.finish_and_clear();
    Ok(())
}
