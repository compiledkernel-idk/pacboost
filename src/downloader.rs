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
use anyhow::{Context, Result, anyhow};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use tokio::sync::Semaphore;
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

pub async fn download_packages(
    urls: Vec<(String, String)>,
    cache_dir: &Path,
    mp: Option<MultiProgress>,
    concurrency: usize,
) -> Result<()> {
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let client = Arc::new(client);

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

    let mut handles = Vec::new();

    for (url, filename) in urls {
        let sem_clone = semaphore.clone();
        let client_clone = client.clone();
        let cache_dir_owned = cache_dir.to_path_buf();
        let url_clone = url.clone();
        let filename_clone = filename.clone();
        let main_pb_clone = main_pb.clone();
        let mp_clone = mp.clone();
        
        let handle = tokio::spawn(async move {
            let _permit = sem_clone.acquire().await.unwrap();
            
            let pb = mp_clone.add(ProgressBar::new(0));
            pb.set_style(ProgressStyle::with_template("   {spinner:.blue} {msg} [{bar:20.blue/cyan}] {bytes}/{total_bytes}").unwrap());
            pb.set_message(format!("downloading: {}", filename_clone));

            let target_path = cache_dir_owned.join(&filename_clone);
            
            let response = client_clone.get(&url_clone).send().await
                .map_err(|e| anyhow!("failed to send request for {}: {}", url_clone, e))?;

            if !response.status().is_success() {
                return Err(anyhow!("failed to download {}: status {}", url_clone, response.status()));
            }

            if let Some(content_length) = response.content_length() {
                pb.set_length(content_length);
            }

            let mut file = tokio::fs::File::create(&target_path).await
                .map_err(|e| anyhow!("failed to create file {}: {}", target_path.display(), e))?;

            let mut stream = response.bytes_stream();
            while let Some(item) = stream.next().await {
                let chunk = item.map_err(|e| anyhow!("error while downloading {}: {}", url_clone, e))?;
                file.write_all(&chunk).await
                    .map_err(|e| anyhow!("failed to write to file {}: {}", target_path.display(), e))?;
                pb.inc(chunk.len() as u64);
            }

            pb.finish_and_clear(); 
            main_pb_clone.inc(1);
            Ok(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }
    
    main_pb.finish_and_clear();
    Ok(())
}
