/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 */

//! Core download engine with segmented parallel downloads.

use super::{
    mirror::MirrorPool,
    segment::{Segment, SegmentManager},
    DownloadConfig,
};
use anyhow::{anyhow, Context, Result};
use console::style;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{header, Client, StatusCode};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// A download task with multiple mirror URLs
#[derive(Debug, Clone)]
pub struct DownloadTask {
    /// Multiple mirror URLs for the same file
    pub mirrors: Vec<String>,
    /// Target filename
    pub filename: String,
    /// Expected file size (if known)
    pub expected_size: Option<u64>,
}

impl DownloadTask {
    pub fn new(mirrors: Vec<String>, filename: String) -> Self {
        Self {
            mirrors,
            filename,
            expected_size: None,
        }
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.expected_size = Some(size);
        self
    }
}

/// Result of a download
#[derive(Debug)]
pub struct DownloadResult {
    pub filename: String,
    pub bytes: u64,
    pub duration: Duration,
    pub throughput_mbps: f64,
}

/// High-performance download engine
pub struct DownloadEngine {
    config: DownloadConfig,
    client: Client,
}

impl DownloadEngine {
    /// Create a new download engine
    pub fn new(config: DownloadConfig) -> Result<Self> {
        let mut builder = Client::builder()
            .pool_max_idle_per_host(config.max_connections)
            .pool_idle_timeout(Duration::from_secs(90))
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .tcp_nodelay(true)
            .user_agent("pacboost/1.6");

        if config.http2 {
            builder = builder
                .http2_adaptive_window(true)
                .http2_keep_alive_interval(Some(Duration::from_secs(10)))
                .http2_keep_alive_timeout(Duration::from_secs(20));
        }

        let client = builder.build().context("failed to build HTTP client")?;

        Ok(Self { config, client })
    }

    /// Download multiple files in parallel
    pub async fn download_all(
        &self,
        tasks: Vec<DownloadTask>,
        cache_dir: &Path,
        mp: Option<MultiProgress>,
    ) -> Result<Vec<DownloadResult>> {
        if tasks.is_empty() {
            return Ok(vec![]);
        }

        // Ensure cache directory exists
        if !cache_dir.exists() {
            tokio::fs::create_dir_all(cache_dir).await?;
        }

        let mp = mp.unwrap_or_default();
        let semaphore = Arc::new(Semaphore::new(self.config.max_connections));
        let total_progress = Arc::new(AtomicU64::new(0));
        let start_time = Instant::now();

        // Main progress bar
        let main_pb = mp.add(ProgressBar::new(tasks.len() as u64));
        main_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.cyan} {msg} [{bar:40.cyan/blue}] {pos}/{len}")
                .unwrap()
                .progress_chars("=>-"),
        );
        main_pb.set_message("downloading");

        let mut join_set: JoinSet<Result<DownloadResult>> = JoinSet::new();

        for task in tasks {
            let client = self.client.clone();
            let config = self.config.clone();
            let semaphore = semaphore.clone();
            let cache_dir = cache_dir.to_path_buf();
            let mp = mp.clone();
            let main_pb = main_pb.clone();
            let total_progress = total_progress.clone();

            join_set.spawn(async move {
                let _permit = semaphore.acquire().await?;

                let result = download_file_segmented(
                    &client,
                    &config,
                    &task,
                    &cache_dir,
                    Some(&mp),
                    total_progress.clone(),
                )
                .await;

                main_pb.inc(1);
                result
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(download_result)) => results.push(download_result),
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(anyhow!("task panic: {}", e)),
            }
        }

        main_pb.finish_and_clear();

        let total_bytes = total_progress.load(Ordering::Relaxed);
        let elapsed = start_time.elapsed();

        if total_bytes > 0 && !results.is_empty() {
            let throughput = (total_bytes as f64 / 1024.0 / 1024.0) / elapsed.as_secs_f64();
            println!(
                "{} {} downloaded in {:.1}s ({:.2} MB/s)",
                style("::").cyan().bold(),
                format_bytes(total_bytes),
                elapsed.as_secs_f64(),
                throughput
            );
        }

        Ok(results)
    }

    /// Get the HTTP client for advanced usage
    pub fn client(&self) -> &Client {
        &self.client
    }
}

/// Download a file with segmented parallel downloads
async fn download_file_segmented(
    client: &Client,
    config: &DownloadConfig,
    task: &DownloadTask,
    cache_dir: &Path,
    mp: Option<&MultiProgress>,
    total_progress: Arc<AtomicU64>,
) -> Result<DownloadResult> {
    let target_path = cache_dir.join(&task.filename);
    let mirrors = MirrorPool::new(task.mirrors.clone());
    let start_time = Instant::now();

    // Probe for file size and range support
    let (size, supports_ranges) = probe_file_metadata(client, &mirrors).await?;

    // Decide whether to use segmented download
    let use_segments = supports_ranges && size >= config.segment_threshold;

    // Create progress bar
    let pb = if let Some(mp) = mp {
        let pb = mp.add(ProgressBar::new(size));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("   {spinner:.blue} {msg} [{bar:25.blue/cyan}] {bytes}/{total_bytes} {bytes_per_sec}")
                .unwrap()
                .progress_chars("=>-"),
        );
        pb.set_message(truncate_filename(&task.filename, 20));
        Some(pb)
    } else {
        None
    };

    let bytes_downloaded = if use_segments {
        download_with_segments(
            client,
            config,
            &mirrors,
            &target_path,
            size,
            pb.as_ref(),
            total_progress.clone(),
        )
        .await?
    } else {
        download_streaming(
            client,
            &mirrors,
            &target_path,
            size,
            pb.as_ref(),
            total_progress.clone(),
        )
        .await?
    };

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    let elapsed = start_time.elapsed();
    let throughput = if elapsed.as_secs_f64() > 0.0 {
        (bytes_downloaded as f64 / 1024.0 / 1024.0) / elapsed.as_secs_f64() * 8.0
    } else {
        0.0
    };

    Ok(DownloadResult {
        filename: task.filename.clone(),
        bytes: bytes_downloaded,
        duration: elapsed,
        throughput_mbps: throughput,
    })
}

/// Probe file metadata from mirrors
async fn probe_file_metadata(client: &Client, mirrors: &MirrorPool) -> Result<(u64, bool)> {
    for mirror in mirrors.all_urls() {
        // Try HEAD request first
        if let Ok(response) = client.head(&mirror).send().await {
            if response.status().is_success() {
                let size = response.content_length().unwrap_or(0);
                let supports_ranges = response
                    .headers()
                    .get(header::ACCEPT_RANGES)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.contains("bytes"))
                    .unwrap_or(false);

                if size > 0 {
                    return Ok((size, supports_ranges));
                }
            }
        }

        // Try range probe
        if let Ok(response) = client
            .get(&mirror)
            .header(header::RANGE, "bytes=0-0")
            .send()
            .await
        {
            if response.status() == StatusCode::PARTIAL_CONTENT {
                if let Some(range) = response.headers().get(header::CONTENT_RANGE) {
                    if let Ok(range_str) = range.to_str() {
                        if let Some(total) = range_str.split('/').next_back() {
                            if let Ok(size) = total.parse::<u64>() {
                                return Ok((size, true));
                            }
                        }
                    }
                }
            } else if response.status().is_success() {
                let size = response.content_length().unwrap_or(0);
                return Ok((size, false));
            }
        }
    }

    Err(anyhow!("failed to probe file metadata from all mirrors"))
}

/// Download with parallel segments
async fn download_with_segments(
    client: &Client,
    config: &DownloadConfig,
    mirrors: &MirrorPool,
    target_path: &Path,
    total_size: u64,
    pb: Option<&ProgressBar>,
    total_progress: Arc<AtomicU64>,
) -> Result<u64> {
    // Preallocate file
    let file = File::create(target_path).await?;
    file.set_len(total_size).await?;
    drop(file);

    let segments = SegmentManager::new(total_size, config.segments);
    let segments_arc = Arc::new(segments);
    let file_path = Arc::new(target_path.to_path_buf());

    let mut join_set: JoinSet<Result<u64>> = JoinSet::new();
    let local_progress = Arc::new(AtomicU64::new(0));

    // Get mirror URLs for round-robin distribution
    let mirror_urls = mirrors.all_urls();
    let num_mirrors = mirror_urls.len().max(1);

    // Start segment downloads with round-robin mirror assignment
    for (idx, segment) in segments_arc.all().iter().enumerate() {
        let client = client.clone();
        let mirrors = mirrors.clone();
        let file_path = file_path.clone();
        let progress = local_progress.clone();
        let total = total_progress.clone();
        let pb = pb.cloned();
        let preferred_mirror = idx % num_mirrors;
        let segment = segment.clone();

        join_set.spawn(async move {
            download_segment(
                &client,
                &mirrors,
                &file_path,
                segment,
                progress,
                total,
                pb,
                preferred_mirror,
            )
            .await
        });
    }

    let mut total_bytes = 0u64;
    while let Some(result) = join_set.join_next().await {
        total_bytes += result??;
    }

    Ok(total_bytes)
}

/// Download a single segment with timeout and retry support
#[allow(clippy::too_many_arguments)]
async fn download_segment(
    client: &Client,
    mirrors: &MirrorPool,
    file_path: &Path,
    segment: Arc<Segment>,
    local_progress: Arc<AtomicU64>,
    total_progress: Arc<AtomicU64>,
    pb: Option<ProgressBar>,
    preferred_mirror: usize,
) -> Result<u64> {
    use tokio::time::timeout;

    const STALL_TIMEOUT: Duration = Duration::from_secs(30);
    const MAX_RETRIES: usize = 3;

    segment.mark_in_progress();

    // Try mirrors with failover, starting with preferred mirror (round-robin)
    let mut last_error = None;
    let all_mirrors = mirrors.all_urls();
    let num_mirrors = all_mirrors.len();

    // Rotate mirror list to start with preferred mirror
    let mirror_order: Vec<String> = if num_mirrors > 0 {
        (0..num_mirrors)
            .map(|i| all_mirrors[(preferred_mirror + i) % num_mirrors].clone())
            .collect()
    } else {
        all_mirrors
    };

    for retry in 0..MAX_RETRIES {
        if retry > 0 {
            // Exponential backoff
            tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(retry as u32))).await;
        }

        for mirror_url in &mirror_order {
            let range = segment.range_header();

            let result = timeout(
                Duration::from_secs(60),
                client.get(mirror_url).header(header::RANGE, &range).send(),
            )
            .await;

            match result {
                Ok(Ok(response)) => {
                    if response.status() == StatusCode::PARTIAL_CONTENT
                        || response.status().is_success()
                    {
                        // Stream to file with buffered I/O
                        let mut file = File::options().write(true).open(file_path).await?;

                        file.seek(std::io::SeekFrom::Start(segment.current_position()))
                            .await?;

                        let mut stream = response.bytes_stream();
                        let mut bytes_written = 0u64;
                        let mut buffer = Vec::with_capacity(256 * 1024); // 256KB buffer
                        let mut stream_error = None;

                        loop {
                            let chunk_result = timeout(STALL_TIMEOUT, stream.next()).await;

                            match chunk_result {
                                Ok(Some(Ok(chunk))) => {
                                    buffer.extend_from_slice(&chunk);

                                    // Flush buffer when it's large enough
                                    if buffer.len() >= 128 * 1024 {
                                        file.write_all(&buffer).await?;
                                        let len = buffer.len() as u64;
                                        bytes_written += len;
                                        segment.add_progress(len);
                                        local_progress.fetch_add(len, Ordering::Relaxed);
                                        total_progress.fetch_add(len, Ordering::Relaxed);

                                        if let Some(ref pb) = pb {
                                            pb.inc(len);
                                        }
                                        buffer.clear();
                                    }
                                }
                                Ok(Some(Err(e))) => {
                                    stream_error = Some(e.to_string());
                                    break;
                                }
                                Ok(None) => {
                                    // Stream complete
                                    break;
                                }
                                Err(_) => {
                                    stream_error = Some("download stalled".to_string());
                                    break;
                                }
                            }
                        }

                        if stream_error.is_some() {
                            last_error = stream_error;
                            continue;
                        }

                        // Flush remaining buffer
                        if !buffer.is_empty() {
                            file.write_all(&buffer).await?;
                            let len = buffer.len() as u64;
                            bytes_written += len;
                            segment.add_progress(len);
                            local_progress.fetch_add(len, Ordering::Relaxed);
                            total_progress.fetch_add(len, Ordering::Relaxed);

                            if let Some(ref pb) = pb {
                                pb.inc(len);
                            }
                        }

                        segment.mark_complete();
                        return Ok(bytes_written);
                    }
                }
                Ok(Err(e)) => {
                    last_error = Some(e.to_string());
                }
                Err(_) => {
                    last_error = Some("connection timeout".to_string());
                }
            }
        }
    }

    segment.mark_failed();
    Err(anyhow!(
        "segment {} failed: {}",
        segment.id,
        last_error.unwrap_or_default()
    ))
}

/// Download without segments (streaming) with timeout and retry support
async fn download_streaming(
    client: &Client,
    mirrors: &MirrorPool,
    target_path: &Path,
    _expected_size: u64,
    pb: Option<&ProgressBar>,
    total_progress: Arc<AtomicU64>,
) -> Result<u64> {
    use tokio::time::timeout;

    const STALL_TIMEOUT: Duration = Duration::from_secs(30);
    const MAX_RETRIES: usize = 3;

    let mut last_error = None;

    for retry in 0..MAX_RETRIES {
        if retry > 0 {
            tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(retry as u32))).await;
        }

        for mirror_url in mirrors.all_urls() {
            let result = timeout(Duration::from_secs(60), client.get(&mirror_url).send()).await;

            match result {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        let mut file = File::create(target_path).await?;
                        let mut stream = response.bytes_stream();
                        let mut bytes_written = 0u64;
                        let mut stream_error = None;

                        loop {
                            let chunk_result = timeout(STALL_TIMEOUT, stream.next()).await;

                            match chunk_result {
                                Ok(Some(Ok(chunk))) => {
                                    file.write_all(&chunk).await?;

                                    let len = chunk.len() as u64;
                                    bytes_written += len;
                                    total_progress.fetch_add(len, Ordering::Relaxed);

                                    if let Some(pb) = pb {
                                        pb.inc(len);
                                    }
                                }
                                Ok(Some(Err(e))) => {
                                    stream_error = Some(e.to_string());
                                    break;
                                }
                                Ok(None) => {
                                    // Stream complete
                                    return Ok(bytes_written);
                                }
                                Err(_) => {
                                    stream_error = Some("download stalled".to_string());
                                    break;
                                }
                            }
                        }

                        if let Some(err) = stream_error {
                            last_error = Some(err);
                            continue;
                        }
                    }
                }
                Ok(Err(e)) => {
                    last_error = Some(e.to_string());
                }
                Err(_) => {
                    last_error = Some("connection timeout".to_string());
                }
            }
        }
    }

    Err(anyhow!(
        "all mirrors failed for streaming download: {}",
        last_error.unwrap_or_default()
    ))
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn truncate_filename(name: &str, max: usize) -> String {
    if name.len() <= max {
        name.to_string()
    } else {
        format!("{}...", &name[..max - 3])
    }
}
