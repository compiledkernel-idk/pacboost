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
use std::time::Duration;

/// Shared HTTP client with connection pooling for maximum performance
fn create_shared_client() -> reqwest::Client {
    reqwest::Client::builder()
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(300))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client")
}

/// Try to download from multiple mirrors with failover
/// Returns the successful response or error if all mirrors failed
async fn download_with_failover(
    client: &reqwest::Client,
    mirrors: &[String],
    timeout_per_mirror: Duration,
) -> Result<reqwest::Response> {
    let mut last_error = None;
    
    for (i, url) in mirrors.iter().enumerate() {
        match tokio::time::timeout(
            timeout_per_mirror,
            client.get(url).send()
        ).await {
            Ok(Ok(response)) if response.status().is_success() => {
                return Ok(response);
            }
            Ok(Ok(response)) => {
                last_error = Some(anyhow!("mirror {} returned status {}", i + 1, response.status()));
            }
            Ok(Err(e)) => {
                last_error = Some(anyhow!("mirror {} request failed: {}", i + 1, e));
            }
            Err(_) => {
                last_error = Some(anyhow!("mirror {} timed out", i + 1));
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| anyhow!("no mirrors provided")))
}

/// Download packages with multi-mirror failover support
/// Each file can have multiple candidate URLs (mirrors)
pub async fn download_packages(
    urls: Vec<(Vec<String>, String)>,
    cache_dir: &Path,
    mp: Option<MultiProgress>,
    concurrency: usize,
) -> Result<()> {
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let client = Arc::new(create_shared_client());

    if !cache_dir.exists() {
        tokio::fs::create_dir_all(cache_dir).await.context("failed to create cache directory")?;
    }

    let mp = mp.unwrap_or_else(MultiProgress::new);
    
    let total_style = ProgressStyle::with_template(
        "{spinner:.cyan} {msg} [{bar:40.cyan/blue}] {pos}/{len}"
    ).unwrap()
    .progress_chars("=>-");
    
    let main_pb = mp.add(ProgressBar::new(urls.len() as u64));
    main_pb.set_style(total_style);
    main_pb.set_message("fetching");

    let mut handles = Vec::new();
    let timeout_per_mirror = Duration::from_secs(3);

    for (mirrors, filename) in urls {
        let sem_clone = semaphore.clone();
        let client_clone = client.clone();
        let cache_dir_owned = cache_dir.to_path_buf();
        let filename_clone = filename.clone();
        let main_pb_clone = main_pb.clone();
        let mp_clone = mp.clone();
        
        let handle: tokio::task::JoinHandle<Result<()>> = tokio::spawn(async move {
            let _permit = sem_clone.acquire().await.unwrap();
            
            let pb = mp_clone.add(ProgressBar::new(0));
            pb.set_style(ProgressStyle::with_template("   {spinner:.blue} {msg} [{bar:20.blue/cyan}] {bytes}/{total_bytes}").unwrap());
            pb.set_message(format!("downloading: {}", filename_clone));

            let target_path = cache_dir_owned.join(&filename_clone);
            
            // Try mirrors with failover
            let response = download_with_failover(&client_clone, &mirrors, timeout_per_mirror).await
                .map_err(|e| anyhow!("all mirrors failed for {}: {}", filename_clone, e))?;

            if let Some(content_length) = response.content_length() {
                pb.set_length(content_length);
            }

            let mut file = tokio::fs::File::create(&target_path).await
                .map_err(|e| anyhow!("failed to create file {}: {}", target_path.display(), e))?;

            let mut stream = response.bytes_stream();
            while let Some(item) = stream.next().await {
                let chunk = item.map_err(|e| anyhow!("error while downloading {}: {}", filename_clone, e))?;
                file.write_all(&chunk).await
                    .map_err(|e| anyhow!("failed to write to file {}: {}", target_path.display(), e))?;
                pb.inc(chunk.len() as u64);
            }

            pb.finish_and_clear(); 
            main_pb_clone.inc(1);
            Ok::<(), anyhow::Error>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }
    
    main_pb.finish_and_clear();
    Ok(())
}

/// Legacy single-URL download function for backward compatibility
pub async fn download_packages_single(
    urls: Vec<(String, String)>,
    cache_dir: &Path,
    mp: Option<MultiProgress>,
    concurrency: usize,
) -> Result<()> {
    let multi_urls: Vec<(Vec<String>, String)> = urls
        .into_iter()
        .map(|(url, filename)| (vec![url], filename))
        .collect();
    download_packages(multi_urls, cache_dir, mp, concurrency).await
}
