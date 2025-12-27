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

use anyhow::{anyhow, Result};
use console::style;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub async fn rank_mirrors(top: usize) -> Result<()> {
    let mirrorlist_path = "/etc/pacman.d/mirrorlist";
    println!("{}", style(":: reading mirrorlist...").bold());

    let content = fs::read_to_string(mirrorlist_path)
        .map_err(|e| anyhow!("failed to read mirrorlist: {}", e))?;

    let mut mirrors = Vec::new();
    for line in content.lines() {
        if line.trim().starts_with("Server =") {
            if let Some(url) = line.split('=').nth(1) {
                mirrors.push(url.trim().to_string());
            }
        }
    }

    if mirrors.is_empty() {
        return Err(anyhow!("no mirrors found in {}", mirrorlist_path));
    }

    println!(
        "{}",
        style(format!(":: processing {} mirrors...", mirrors.len())).bold()
    );

    // Phase 1: FAST Latency check (HEAD)
    // We check all mirrors to filter out dead ones and get a rough "closeness" metric
    let pb_latency = ProgressBar::new(mirrors.len() as u64);
    pb_latency.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [latency] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;

    let initial_results = Arc::new(Mutex::new(Vec::new()));

    let stream = stream::iter(mirrors.clone())
        .map(|url| {
            let client = client.clone();
            let pb = pb_latency.clone();
            let results = initial_results.clone();
            async move {
                let start = Instant::now();
                // Ensure we hit a file, not a directory listing which can be slow to generate
                let test_url = if url.contains("$repo") {
                    url.replace("$repo", "core").replace("$arch", "x86_64")
                } else {
                    // unexpected format, try appending
                    format!("{}/core/os/x86_64", url.trim_end_matches('/'))
                };
                let test_url = format!("{}/core.db", test_url.trim_end_matches('/'));

                if client.head(&test_url).send().await.is_ok() {
                    let duration = start.elapsed();
                    let mut r = results.lock().unwrap();
                    r.push((url, duration));
                }
                pb.inc(1);
            }
        })
        .buffer_unordered(50); // High concurrency for HEAD requests

    stream.collect::<Vec<_>>().await;
    pb_latency.finish_and_clear();

    let mut candidates = initial_results.lock().unwrap().clone();
    if candidates.is_empty() {
        return Err(anyhow!("all mirrors failed to respond"));
    }

    // Sort by latency to get the candidates for bandwidth testing
    candidates.sort_by_key(|k| k.1);

    // Phase 2: BANDWIDTH check (GET)
    // Take top 20 low-latency mirrors and test actual download speed
    // Latency != Bandwidth. A closer mirror might be overloaded.
    let pool_size = std::cmp::min(candidates.len(), 20);
    let candidates = &candidates[..pool_size];

    println!(
        "{}",
        style(format!(
            ":: benchmarking throughput on top {} candidates...",
            pool_size
        ))
        .bold()
    );

    let pb_speed = ProgressBar::new(pool_size as u64);
    pb_speed.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [speed]   [{bar:40.yellow/red}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Use a slightly longer timeout for download tests
    let dl_client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let final_results = Arc::new(Mutex::new(Vec::new()));

    let stream_dl = stream::iter(candidates.to_vec())
        .map(|(url, latency)| {
            let client = dl_client.clone();
            let pb = pb_speed.clone();
            let results = final_results.clone();
            async move {
                let test_url = if url.contains("$repo") {
                    url.replace("$repo", "core").replace("$arch", "x86_64")
                } else {
                    format!("{}/core/os/x86_64", url.trim_end_matches('/'))
                };
                let test_url = format!("{}/core.db", test_url.trim_end_matches('/'));

                let start = Instant::now();
                match client.get(&test_url).send().await {
                    Ok(resp) => {
                        if let Ok(bytes) = resp.bytes().await {
                            let duration = start.elapsed();
                            let size = bytes.len();
                            let speed = if duration.as_secs_f64() > 0.0 {
                                size as f64 / duration.as_secs_f64()
                            } else {
                                0.0
                            };
                            let mut r = results.lock().unwrap();
                            r.push((url, latency, speed));
                        }
                    }
                    Err(_) => {
                        // If download fails, drop it or treat as 0 speed
                    }
                }
                pb.inc(1);
            }
        })
        .buffer_unordered(5); // Lower concurrency specifically to avoid bandwidth contention affecting results

    stream_dl.collect::<Vec<_>>().await;
    pb_speed.finish_and_clear();

    let mut ranked = final_results.lock().unwrap().clone();

    // Mix score: heavily favor speed, but use latency as tiebreaker
    ranked.sort_by(|a, b| {
        // Sort DESCENDING by speed
        b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)
    });

    if ranked.is_empty() {
        return Err(anyhow!("bandwidth test failed for all candidates"));
    }

    println!("{}", style(":: top 5 fastest mirrors:").bold().green());
    for (i, (url, latency, speed)) in ranked.iter().take(5).enumerate() {
        println!(
            "   {}. {} (latency: {:?}, speed: {:.2} MB/s)",
            i + 1,
            url,
            latency,
            speed / 1024.0 / 1024.0
        );
    }

    // Backup
    let backup_path = format!("{}.backup", mirrorlist_path);
    // Ignore error if backup exists or fails, keep going
    let _ = fs::copy(mirrorlist_path, &backup_path);

    // Write new mirrorlist
    let mut new_content = String::from("## Generated by pacboost\n");
    for (url, _, _) in ranked.iter().take(top) {
        new_content.push_str(&format!("Server = {}\n", url));
    }

    // Check for root before writing
    if unsafe { libc::geteuid() } != 0 {
        return Err(anyhow!(
            "root privileges required to write to /etc/pacman.d/mirrorlist"
        ));
    }

    fs::write(mirrorlist_path, new_content)?;
    println!("{}", style(":: mirrorlist updated.").green().bold());

    Ok(())
}
