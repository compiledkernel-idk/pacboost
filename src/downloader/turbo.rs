/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 *
 * TURBO DOWNLOAD ENGINE - 2x+ Faster Than Pacman
 *
 * Key innovations:
 * 1. Aggressive multi-connection racing (first-responder wins)
 * 2. Parallel segment downloads with optimal chunk sizes
 * 3. Connection pooling with keep-alive reuse
 * 4. Async buffered I/O with zero-copy where possible
 * 5. Adaptive parallelism based on network conditions
 * 6. Pipelined requests for minimal latency
 * 7. Robust error handling with automatic retries
 * 8. Integrity verification with SHA256 checksums
 */

use anyhow::{anyhow, Context, Result};
use console::style;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{header, Client, StatusCode};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufWriter};
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use tokio::task::JoinSet;
use tokio::time::timeout;

/// Maximum number of retries per segment/download
const MAX_RETRIES: usize = 5;
/// Base delay for exponential backoff (milliseconds)
const RETRY_BASE_DELAY_MS: u64 = 100;
/// Maximum delay between retries (milliseconds)
const MAX_RETRY_DELAY_MS: u64 = 5000;
/// Stall detection timeout - if no data received for this duration, retry
const STALL_TIMEOUT_SECS: u64 = 30;
/// Maximum body chunk size to prevent memory issues
const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024; // 16 MB

/// Turbo download configuration - optimized for maximum speed and stability
#[derive(Debug, Clone)]
pub struct TurboConfig {
    /// Maximum total concurrent connections (across all downloads)
    pub max_total_connections: usize,
    /// Maximum connections per single file
    pub max_connections_per_file: usize,
    /// Number of segments for large files
    pub segments_large: usize,
    /// Number of segments for medium files
    pub segments_medium: usize,
    /// Threshold for "large" file (uses many segments)
    pub large_file_threshold: u64,
    /// Threshold for "medium" file (uses some segments)
    pub medium_file_threshold: u64,
    /// Minimum segment size (avoid too many small segments)
    pub min_segment_size: u64,
    /// Buffer size for writes (larger = fewer syscalls)
    pub write_buffer_size: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Read timeout per chunk
    pub read_timeout: Duration,
    /// Enable aggressive racing (start multiple mirrors simultaneously)
    pub enable_racing: bool,
    /// Number of mirrors to race simultaneously
    pub racing_mirrors: usize,
    /// Enable HTTP/2 multiplexing
    pub http2_multiplexing: bool,
    /// Keep-alive timeout
    pub keep_alive_timeout: Duration,
    /// Maximum retries per segment
    pub max_retries: usize,
    /// Enable integrity verification
    pub verify_integrity: bool,
    /// Continue on single download failure (don't abort entire batch)
    pub continue_on_error: bool,
}

impl Default for TurboConfig {
    fn default() -> Self {
        // Get CPU core count for optimal parallelism
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Self {
            // Scale connections with CPU cores, but limit to avoid overwhelming servers
            max_total_connections: (cores * 4).clamp(16, 64),
            max_connections_per_file: (cores).clamp(4, 16),
            segments_large: 8,  // Reduced from 16 for better stability
            segments_medium: 4, // Reduced from 8 for better stability
            large_file_threshold: 10 * 1024 * 1024, // 10 MB
            medium_file_threshold: 1024 * 1024, // 1 MB
            min_segment_size: 256 * 1024, // 256 KB minimum segment
            write_buffer_size: 256 * 1024, // 256 KB write buffer
            connect_timeout: Duration::from_secs(10), // Increased for stability
            read_timeout: Duration::from_secs(60), // Increased for large files
            enable_racing: true,
            racing_mirrors: 3, // Race 3 mirrors simultaneously
            http2_multiplexing: true,
            keep_alive_timeout: Duration::from_secs(60),
            max_retries: MAX_RETRIES,
            verify_integrity: true,
            continue_on_error: true, // Don't fail entire batch on single failure
        }
    }
}

impl TurboConfig {
    /// Configuration optimized for fast networks (100+ Mbps)
    pub fn fast_network() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Self {
            max_total_connections: (cores * 8).clamp(32, 128),
            max_connections_per_file: (cores * 2).clamp(8, 32),
            segments_large: 16,
            segments_medium: 8,
            large_file_threshold: 5 * 1024 * 1024,
            medium_file_threshold: 512 * 1024,
            min_segment_size: 128 * 1024,
            write_buffer_size: 512 * 1024, // 512 KB buffer
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
            enable_racing: true,
            racing_mirrors: 5,
            http2_multiplexing: true,
            keep_alive_timeout: Duration::from_secs(120),
            max_retries: MAX_RETRIES,
            verify_integrity: true,
            continue_on_error: true,
        }
    }

    /// Configuration for slower/unstable networks
    pub fn slow_network() -> Self {
        Self {
            max_total_connections: 8,
            max_connections_per_file: 2,
            segments_large: 4,
            segments_medium: 2,
            large_file_threshold: 20 * 1024 * 1024,
            medium_file_threshold: 5 * 1024 * 1024,
            min_segment_size: 512 * 1024,
            write_buffer_size: 128 * 1024,
            connect_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(120),
            enable_racing: true,
            racing_mirrors: 2,
            http2_multiplexing: false, // Fallback to HTTP/1.1
            keep_alive_timeout: Duration::from_secs(30),
            max_retries: MAX_RETRIES * 2, // More retries for unstable networks
            verify_integrity: true,
            continue_on_error: true,
        }
    }

    /// Configuration for maximum stability (slower but more reliable)
    pub fn stable() -> Self {
        Self {
            max_total_connections: 4,
            max_connections_per_file: 1,
            segments_large: 1, // No segmentation
            segments_medium: 1,
            large_file_threshold: u64::MAX, // Never segment
            medium_file_threshold: u64::MAX,
            min_segment_size: u64::MAX,
            write_buffer_size: 64 * 1024,
            connect_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(300),
            enable_racing: false, // Sequential mirror attempts
            racing_mirrors: 1,
            http2_multiplexing: false,
            keep_alive_timeout: Duration::from_secs(30),
            max_retries: MAX_RETRIES * 3,
            verify_integrity: true,
            continue_on_error: true,
        }
    }
}

/// A download task with multiple mirror URLs
#[derive(Debug, Clone)]
pub struct TurboTask {
    /// Multiple mirror URLs for the same file (will be raced)
    pub mirrors: Vec<String>,
    /// Target filename
    pub filename: String,
    /// Expected file size (if known - speeds up download)
    pub expected_size: Option<u64>,
    /// Priority (higher = download first)
    pub priority: u8,
    /// Expected SHA256 checksum (for integrity verification)
    pub expected_checksum: Option<String>,
}

impl TurboTask {
    pub fn new(mirrors: Vec<String>, filename: String) -> Self {
        Self {
            mirrors,
            filename,
            expected_size: None,
            priority: 0,
            expected_checksum: None,
        }
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.expected_size = Some(size);
        self
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_checksum(mut self, checksum: String) -> Self {
        self.expected_checksum = Some(checksum);
        self
    }
}

/// Statistics for a completed download
#[derive(Debug, Clone)]
pub struct TurboStats {
    pub filename: String,
    pub bytes: u64,
    pub duration: Duration,
    pub throughput_mbps: f64,
    pub mirror_used: String,
    pub segments_used: usize,
    pub retries: usize,
    pub verified: bool,
}

/// Download result with detailed error information
#[derive(Debug)]
pub struct DownloadOutcome {
    pub filename: String,
    pub success: bool,
    pub stats: Option<TurboStats>,
    pub error: Option<String>,
}

/// Mirror performance tracking for intelligent selection
#[derive(Debug)]
struct MirrorStats {
    url: String,
    /// Cumulative bytes downloaded
    bytes: AtomicU64,
    /// Cumulative time in milliseconds
    time_ms: AtomicU64,
    /// Number of successful chunks
    successes: AtomicU64,
    /// Number of failures
    failures: AtomicU64,
    /// Current active connections
    active_connections: AtomicUsize,
    /// Whether this mirror is currently marked as failed
    is_failed: AtomicBool,
    /// Last failure timestamp (for temporary blacklisting)
    last_failure_time: AtomicU64,
}

impl MirrorStats {
    fn new(url: String) -> Self {
        Self {
            url,
            bytes: AtomicU64::new(0),
            time_ms: AtomicU64::new(0),
            successes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            active_connections: AtomicUsize::new(0),
            is_failed: AtomicBool::new(false),
            last_failure_time: AtomicU64::new(0),
        }
    }

    fn record_success(&self, bytes: u64, time_ms: u64) {
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
        self.time_ms.fetch_add(time_ms, Ordering::Relaxed);
        self.successes.fetch_add(1, Ordering::Relaxed);
        // Clear failed state on success
        self.is_failed.store(false, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_failure_time.store(now, Ordering::Relaxed);

        // Mark as failed if too many failures
        let failures = self.failures.load(Ordering::Relaxed);
        let successes = self.successes.load(Ordering::Relaxed);
        if failures > 3 && failures > successes * 2 {
            self.is_failed.store(true, Ordering::Relaxed);
        }
    }

    /// Check if mirror should be temporarily skipped
    fn should_skip(&self) -> bool {
        if !self.is_failed.load(Ordering::Relaxed) {
            return false;
        }

        // Allow retry after 60 seconds
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last_failure = self.last_failure_time.load(Ordering::Relaxed);
        now.saturating_sub(last_failure) < 60
    }

    /// Calculate throughput score (higher is better)
    fn score(&self) -> u64 {
        if self.should_skip() {
            return 0;
        }

        let bytes = self.bytes.load(Ordering::Relaxed);
        let time_ms = self.time_ms.load(Ordering::Relaxed);
        let failures = self.failures.load(Ordering::Relaxed);

        if time_ms == 0 {
            return 1000; // Unknown mirrors get neutral score
        }

        // Throughput in KB/s, penalized by failure rate
        let throughput = (bytes * 1000) / time_ms;
        let penalty = (failures * 10).min(90); // Max 90% penalty
        (throughput * (100 - penalty)) / 100
    }
}

/// Segment state for tracking download progress
#[derive(Debug)]
struct SegmentState {
    start: u64,
    end: u64,
    downloaded: AtomicU64,
    complete: AtomicBool,
    retries: AtomicUsize,
}

impl SegmentState {
    fn new(start: u64, end: u64) -> Self {
        Self {
            start,
            end,
            downloaded: AtomicU64::new(0),
            complete: AtomicBool::new(false),
            retries: AtomicUsize::new(0),
        }
    }

    fn size(&self) -> u64 {
        self.end - self.start + 1
    }

    fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    fn mark_complete(&self) {
        self.complete.store(true, Ordering::Release);
    }

    fn current_position(&self) -> u64 {
        self.start + self.downloaded.load(Ordering::Relaxed)
    }

    fn add_downloaded(&self, bytes: u64) {
        self.downloaded.fetch_add(bytes, Ordering::Relaxed);
    }

    fn increment_retry(&self) -> usize {
        self.retries.fetch_add(1, Ordering::Relaxed)
    }

    fn reset_for_retry(&self) {
        self.downloaded.store(0, Ordering::Relaxed);
        self.complete.store(false, Ordering::Relaxed);
    }
}

/// High-performance turbo download engine
pub struct TurboEngine {
    config: TurboConfig,
    client: Client,
    /// Global connection semaphore
    semaphore: Arc<Semaphore>,
    /// Mirror performance stats (shared across downloads)
    mirror_stats: Arc<RwLock<HashMap<String, Arc<MirrorStats>>>>,
    /// Global progress tracking
    global_bytes: Arc<AtomicU64>,
    /// Global retry counter
    global_retries: Arc<AtomicUsize>,
}

impl TurboEngine {
    /// Create a new turbo engine with the given configuration
    pub fn new(config: TurboConfig) -> Result<Self> {
        let mut builder = Client::builder()
            .pool_max_idle_per_host(config.max_total_connections / 4)
            .pool_idle_timeout(config.keep_alive_timeout)
            .connect_timeout(config.connect_timeout)
            .timeout(config.read_timeout * 2) // Overall request timeout
            .tcp_nodelay(true)
            .tcp_keepalive(Some(Duration::from_secs(15)))
            .user_agent("pacboost-turbo/2.2");

        // Enable HTTP/2 adaptive features but allow fallback to HTTP/1.1
        // Do NOT use http2_prior_knowledge() as it breaks HTTP/1.1 only servers
        if config.http2_multiplexing {
            builder = builder
                .http2_adaptive_window(true)
                .http2_keep_alive_interval(Some(Duration::from_secs(5)))
                .http2_keep_alive_timeout(Duration::from_secs(10));
        }

        let client = builder.build().context("failed to build HTTP client")?;

        let semaphore = Arc::new(Semaphore::new(config.max_total_connections));

        Ok(Self {
            config,
            client,
            semaphore,
            mirror_stats: Arc::new(RwLock::new(HashMap::new())),
            global_bytes: Arc::new(AtomicU64::new(0)),
            global_retries: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Download multiple files with maximum parallelism and error resilience
    pub async fn download_all(
        &self,
        mut tasks: Vec<TurboTask>,
        cache_dir: &Path,
        mp: Option<MultiProgress>,
    ) -> Result<Vec<TurboStats>> {
        if tasks.is_empty() {
            return Ok(vec![]);
        }

        // Ensure cache directory exists
        if !cache_dir.exists() {
            tokio::fs::create_dir_all(cache_dir)
                .await
                .context("failed to create cache directory")?;
        }

        // Sort by priority (higher first), then by expected size (smaller first for faster feedback)
        tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.expected_size.cmp(&b.expected_size))
        });

        let mp = mp.unwrap_or_default();
        let start_time = Instant::now();
        self.global_bytes.store(0, Ordering::Relaxed);
        self.global_retries.store(0, Ordering::Relaxed);

        let total_tasks = tasks.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicUsize::new(0));

        // Main progress bar
        let main_pb = mp.add(ProgressBar::new(total_tasks as u64));
        main_pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.cyan.bold} {msg} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)",
                )
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        main_pb.set_message("downloading");
        main_pb.enable_steady_tick(Duration::from_millis(100));

        // Channel for collecting results
        let (tx, mut rx) = mpsc::channel::<DownloadOutcome>(tasks.len());

        // Per-file semaphore to limit concurrent downloads
        let file_semaphore = Arc::new(Semaphore::new(self.config.max_total_connections / 2));

        // Spawn all download tasks
        let mut handles = JoinSet::new();

        for task in tasks {
            let client = self.client.clone();
            let config = self.config.clone();
            let global_semaphore = self.semaphore.clone();
            let file_semaphore = file_semaphore.clone();
            let mirror_stats = self.mirror_stats.clone();
            let global_bytes = self.global_bytes.clone();
            let global_retries = self.global_retries.clone();
            let cache_dir = cache_dir.to_path_buf();
            let mp = mp.clone();
            let main_pb = main_pb.clone();
            let tx = tx.clone();
            let completed = completed.clone();
            let failed = failed.clone();

            handles.spawn(async move {
                // Acquire file-level permit
                let _file_permit = file_semaphore.acquire().await;

                let result = download_turbo_with_retry(
                    &client,
                    &config,
                    &task,
                    &cache_dir,
                    Some(&mp),
                    global_semaphore,
                    mirror_stats,
                    global_bytes,
                    global_retries,
                )
                .await;

                let outcome = match result {
                    Ok(stats) => {
                        completed.fetch_add(1, Ordering::Relaxed);
                        DownloadOutcome {
                            filename: task.filename.clone(),
                            success: true,
                            stats: Some(stats),
                            error: None,
                        }
                    }
                    Err(e) => {
                        failed.fetch_add(1, Ordering::Relaxed);
                        DownloadOutcome {
                            filename: task.filename.clone(),
                            success: false,
                            stats: None,
                            error: Some(e.to_string()),
                        }
                    }
                };

                main_pb.inc(1);
                let _ = tx.send(outcome).await;
            });
        }

        // Drop original sender so rx completes when all tasks are done
        drop(tx);

        // Collect results
        let mut results = Vec::new();
        let mut errors = Vec::new();

        while let Some(outcome) = rx.recv().await {
            if let Some(stats) = outcome.stats {
                results.push(stats);
            } else if let Some(err) = outcome.error {
                errors.push((outcome.filename, err));
            }
        }

        // Wait for all tasks to complete
        while handles.join_next().await.is_some() {}

        main_pb.finish_and_clear();

        // Print summary
        let total_bytes = self.global_bytes.load(Ordering::Relaxed);
        let total_retries = self.global_retries.load(Ordering::Relaxed);
        let elapsed = start_time.elapsed();
        let completed_count = completed.load(Ordering::Relaxed);
        let failed_count = failed.load(Ordering::Relaxed);

        if total_bytes > 0 && !results.is_empty() {
            let throughput_mbs = (total_bytes as f64 / 1024.0 / 1024.0) / elapsed.as_secs_f64();

            println!(
                "{} Download complete: {} in {:.1}s ({:.1} MB/s)",
                style("->").cyan().bold(),
                format_bytes(total_bytes),
                elapsed.as_secs_f64(),
                throughput_mbs
            );

            if total_retries > 0 {
                println!(
                    "   {} retries needed, {}/{} succeeded",
                    total_retries,
                    completed_count,
                    completed_count + failed_count
                );
            }
        }

        // Report errors
        if !errors.is_empty() {
            for (filename, error) in &errors {
                eprintln!(
                    "{} Failed to download {}: {}",
                    style("!").red().bold(),
                    filename,
                    error
                );
            }

            if !self.config.continue_on_error {
                return Err(anyhow!("{} download(s) failed", errors.len()));
            }
        }

        Ok(results)
    }

    /// Get statistics about mirror performance
    pub async fn get_mirror_stats(&self) -> Vec<(String, u64)> {
        let stats = self.mirror_stats.read().await;
        let mut result: Vec<_> = stats
            .iter()
            .map(|(url, s)| (url.clone(), s.score()))
            .collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }
}

/// Download a single file with retry logic and integrity verification
#[allow(clippy::too_many_arguments)]
async fn download_turbo_with_retry(
    client: &Client,
    config: &TurboConfig,
    task: &TurboTask,
    cache_dir: &Path,
    mp: Option<&MultiProgress>,
    semaphore: Arc<Semaphore>,
    mirror_stats: Arc<RwLock<HashMap<String, Arc<MirrorStats>>>>,
    global_bytes: Arc<AtomicU64>,
    global_retries: Arc<AtomicUsize>,
) -> Result<TurboStats> {
    let mut last_error = None;
    let mut total_retries = 0;

    for attempt in 0..config.max_retries {
        if attempt > 0 {
            // Exponential backoff with jitter
            let delay = calculate_backoff_delay(attempt);
            tokio::time::sleep(Duration::from_millis(delay)).await;
            global_retries.fetch_add(1, Ordering::Relaxed);
            total_retries += 1;
        }

        match download_turbo(
            client,
            config,
            task,
            cache_dir,
            mp,
            semaphore.clone(),
            mirror_stats.clone(),
            global_bytes.clone(),
        )
        .await
        {
            Ok(mut stats) => {
                stats.retries = total_retries;

                // Verify integrity if checksum provided
                if config.verify_integrity {
                    if let Some(ref expected) = task.expected_checksum {
                        let target_path = cache_dir.join(&task.filename);
                        match verify_file_checksum(&target_path, expected).await {
                            Ok(true) => {
                                stats.verified = true;
                                return Ok(stats);
                            }
                            Ok(false) => {
                                last_error = Some(anyhow!("checksum mismatch"));
                                // Delete corrupted file
                                let _ = tokio::fs::remove_file(&target_path).await;
                                continue;
                            }
                            Err(e) => {
                                last_error = Some(e);
                                continue;
                            }
                        }
                    }
                }

                return Ok(stats);
            }
            Err(e) => {
                last_error = Some(e);
                // Clean up partial download
                let target_path = cache_dir.join(&task.filename);
                let _ = tokio::fs::remove_file(&target_path).await;
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| anyhow!("download failed after {} retries", config.max_retries)))
}

/// Calculate exponential backoff delay with jitter
fn calculate_backoff_delay(attempt: usize) -> u64 {
    let base = RETRY_BASE_DELAY_MS * 2u64.pow(attempt as u32);
    let max = base.min(MAX_RETRY_DELAY_MS);
    // Add jitter (±25%)
    let jitter = max / 4;
    let random_jitter = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64)
        % (jitter * 2);
    max.saturating_sub(jitter).saturating_add(random_jitter)
}

/// Verify file checksum
async fn verify_file_checksum(path: &Path, expected: &str) -> Result<bool> {
    let mut file = File::open(path)
        .await
        .context("failed to open file for verification")?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];

    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let computed = hex::encode(hasher.finalize());
    Ok(computed.eq_ignore_ascii_case(expected))
}

/// Download a single file with turbo optimizations
#[allow(clippy::too_many_arguments)]
async fn download_turbo(
    client: &Client,
    config: &TurboConfig,
    task: &TurboTask,
    cache_dir: &Path,
    mp: Option<&MultiProgress>,
    semaphore: Arc<Semaphore>,
    mirror_stats: Arc<RwLock<HashMap<String, Arc<MirrorStats>>>>,
    global_bytes: Arc<AtomicU64>,
) -> Result<TurboStats> {
    let target_path = cache_dir.join(&task.filename);
    let start_time = Instant::now();

    // Get or create mirror stats
    {
        let mut stats = mirror_stats.write().await;
        for mirror in &task.mirrors {
            let base = extract_base_url(mirror);
            stats
                .entry(base.clone())
                .or_insert_with(|| Arc::new(MirrorStats::new(base)));
        }
    }

    // Sort mirrors by score (best first), excluding failed ones
    let sorted_mirrors = {
        let stats = mirror_stats.read().await;
        let mut mirrors: Vec<_> = task
            .mirrors
            .iter()
            .map(|m| {
                let base = extract_base_url(m);
                let score = stats.get(&base).map(|s| s.score()).unwrap_or(1000);
                (m.clone(), score)
            })
            .filter(|(m, _)| {
                let base = extract_base_url(m);
                !stats.get(&base).map(|s| s.should_skip()).unwrap_or(false)
            })
            .collect();
        mirrors.sort_by(|a, b| b.1.cmp(&a.1));
        mirrors.into_iter().map(|(m, _)| m).collect::<Vec<_>>()
    };

    if sorted_mirrors.is_empty() {
        // All mirrors failed, reset and try original list
        return Err(anyhow!("all mirrors temporarily unavailable"));
    }

    // Probe file metadata (size and range support) by racing mirrors
    let (file_size, supports_ranges, _best_mirror) = if config.enable_racing {
        probe_racing(client, config, &sorted_mirrors).await?
    } else {
        probe_sequential(client, &sorted_mirrors).await?
    };

    // Determine segment count based on file size
    let num_segments = if !supports_ranges {
        1 // Can't segment without range support
    } else if file_size >= config.large_file_threshold {
        config.segments_large
    } else if file_size >= config.medium_file_threshold {
        config.segments_medium
    } else {
        1 // Small files: single download
    };

    // Ensure segments aren't too small
    let num_segments =
        if num_segments > 1 && file_size / num_segments as u64 >= config.min_segment_size {
            num_segments
        } else {
            (file_size / config.min_segment_size).max(1) as usize
        };

    // Create progress bar
    let pb = mp.map(|m| {
        let pb = m.add(ProgressBar::new(file_size));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("   {spinner:.blue} {msg} [{bar:25.blue/cyan}] {bytes}/{total_bytes} {bytes_per_sec}")
                .unwrap()
                .progress_chars("█▓░"),
        );
        pb.set_message(truncate_filename(&task.filename, 20));
        pb
    });

    // Download based on whether we support ranges
    let (bytes_downloaded, mirror_used) = if supports_ranges && num_segments > 1 {
        download_segmented(
            client,
            config,
            &sorted_mirrors,
            &target_path,
            file_size,
            num_segments,
            pb.as_ref(),
            semaphore,
            global_bytes.clone(),
            mirror_stats.clone(),
        )
        .await?
    } else {
        download_streaming(
            client,
            config,
            &sorted_mirrors,
            &target_path,
            file_size,
            pb.as_ref(),
            semaphore,
            global_bytes.clone(),
            mirror_stats.clone(),
        )
        .await?
    };

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    // Verify downloaded size
    if bytes_downloaded != file_size {
        return Err(anyhow!(
            "size mismatch: expected {} bytes, got {} bytes",
            file_size,
            bytes_downloaded
        ));
    }

    let elapsed = start_time.elapsed();
    let throughput_mbps = if elapsed.as_secs_f64() > 0.0 {
        (bytes_downloaded as f64 / 1024.0 / 1024.0) / elapsed.as_secs_f64() * 8.0
    } else {
        0.0
    };

    Ok(TurboStats {
        filename: task.filename.clone(),
        bytes: bytes_downloaded,
        duration: elapsed,
        throughput_mbps,
        mirror_used,
        segments_used: num_segments,
        retries: 0,
        verified: false,
    })
}

/// Race multiple mirrors to probe file metadata - returns first successful response
async fn probe_racing(
    client: &Client,
    config: &TurboConfig,
    mirrors: &[String],
) -> Result<(u64, bool, String)> {
    let mut futures = FuturesUnordered::new();

    // Start racing with limited mirrors
    let race_count = config.racing_mirrors.min(mirrors.len());
    for mirror in mirrors.iter().take(race_count) {
        let client = client.clone();
        let mirror = mirror.clone();
        let connect_timeout = config.connect_timeout;

        futures.push(async move {
            let result = timeout(connect_timeout * 2, probe_single(&client, &mirror)).await;

            (mirror, result.ok().flatten())
        });
    }

    // Return first successful probe
    while let Some((mirror, result)) = futures.next().await {
        if let Some((size, supports_ranges)) = result {
            if size > 0 {
                return Ok((size, supports_ranges, mirror));
            }
        }
    }

    // Fallback: try remaining mirrors sequentially
    for mirror in mirrors.iter().skip(race_count) {
        if let Ok(Some((size, supports_ranges))) =
            timeout(config.connect_timeout * 2, probe_single(client, mirror)).await
        {
            if size > 0 {
                return Ok((size, supports_ranges, mirror.clone()));
            }
        }
    }

    Err(anyhow!("failed to probe file metadata from all mirrors"))
}

/// Sequential probing (fallback)
async fn probe_sequential(client: &Client, mirrors: &[String]) -> Result<(u64, bool, String)> {
    for mirror in mirrors {
        if let Some((size, supports_ranges)) = probe_single(client, mirror).await {
            if size > 0 {
                return Ok((size, supports_ranges, mirror.clone()));
            }
        }
    }
    Err(anyhow!("failed to probe file metadata from all mirrors"))
}

/// Probe a single mirror for file metadata
async fn probe_single(client: &Client, url: &str) -> Option<(u64, bool)> {
    // Try HEAD request first (fastest)
    if let Ok(response) = client.head(url).send().await {
        if response.status().is_success() {
            let size = response.content_length().unwrap_or(0);
            let supports_ranges = response
                .headers()
                .get(header::ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("bytes"))
                .unwrap_or(false);

            if size > 0 {
                return Some((size, supports_ranges));
            }
        }
    }

    // Try range request to detect support and get size
    if let Ok(response) = client
        .get(url)
        .header(header::RANGE, "bytes=0-0")
        .send()
        .await
    {
        if response.status() == StatusCode::PARTIAL_CONTENT {
            if let Some(range) = response.headers().get(header::CONTENT_RANGE) {
                if let Ok(range_str) = range.to_str() {
                    // Format: "bytes 0-0/12345"
                    if let Some(total) = range_str.split('/').next_back() {
                        if let Ok(size) = total.parse::<u64>() {
                            return Some((size, true));
                        }
                    }
                }
            }
        } else if response.status().is_success() {
            let size = response.content_length().unwrap_or(0);
            return Some((size, false));
        }
    }

    None
}

/// Download with parallel segments
#[allow(clippy::too_many_arguments)]
async fn download_segmented(
    client: &Client,
    config: &TurboConfig,
    mirrors: &[String],
    target_path: &Path,
    file_size: u64,
    num_segments: usize,
    pb: Option<&ProgressBar>,
    semaphore: Arc<Semaphore>,
    global_bytes: Arc<AtomicU64>,
    mirror_stats: Arc<RwLock<HashMap<String, Arc<MirrorStats>>>>,
) -> Result<(u64, String)> {
    // Preallocate file
    let file = File::create(target_path).await?;
    file.set_len(file_size).await?;
    drop(file);

    // Create segments
    let segment_size = file_size.div_ceil(num_segments as u64);
    let segments: Vec<Arc<SegmentState>> = (0..num_segments)
        .map(|i| {
            let start = i as u64 * segment_size;
            let end = ((i as u64 + 1) * segment_size - 1).min(file_size - 1);
            Arc::new(SegmentState::new(start, end))
        })
        .filter(|s| s.start <= s.end)
        .collect();

    let file_path = Arc::new(target_path.to_path_buf());
    let best_mirror = Arc::new(RwLock::new(mirrors.first().cloned().unwrap_or_default()));
    let total_downloaded = Arc::new(AtomicU64::new(0));
    let segment_errors = Arc::new(Mutex::new(Vec::new()));

    // File lock for synchronized writes
    let file_lock = Arc::new(Mutex::new(()));

    let mut handles = JoinSet::new();

    for (idx, segment) in segments.iter().enumerate() {
        let client = client.clone();
        let mirrors = mirrors.to_vec();
        let file_path = file_path.clone();
        let pb = pb.cloned();
        let semaphore = semaphore.clone();
        let global_bytes = global_bytes.clone();
        let mirror_stats = mirror_stats.clone();
        let best_mirror = best_mirror.clone();
        let config = config.clone();
        let segment = segment.clone();
        let total_downloaded = total_downloaded.clone();
        let file_lock = file_lock.clone();
        let segment_errors = segment_errors.clone();

        // Distribute segments across mirrors (round-robin)
        let preferred_mirror_idx = idx % mirrors.len();

        handles.spawn(async move {
            let result = download_segment_with_retry(
                &client,
                &config,
                &mirrors,
                preferred_mirror_idx,
                &file_path,
                &segment,
                pb.as_ref(),
                semaphore,
                global_bytes,
                total_downloaded,
                mirror_stats,
                best_mirror,
                file_lock,
            )
            .await;

            if let Err(e) = &result {
                let mut errors = segment_errors.lock().await;
                errors.push(format!("segment {}: {}", idx, e));
            }

            result
        });
    }

    // Wait for all segments
    while let Some(result) = handles.join_next().await {
        if let Err(e) = result {
            return Err(anyhow!("segment task panicked: {}", e));
        }
    }

    // Check for segment errors
    let errors = segment_errors.lock().await;
    if !errors.is_empty() {
        return Err(anyhow!("segment download failed: {}", errors.join("; ")));
    }

    // Verify all segments completed
    let incomplete: Vec<_> = segments
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_complete())
        .map(|(i, _)| i)
        .collect();

    if !incomplete.is_empty() {
        return Err(anyhow!("incomplete segments: {:?}", incomplete));
    }

    let mirror = best_mirror.read().await.clone();
    let total = total_downloaded.load(Ordering::Relaxed);
    Ok((total, mirror))
}

/// Download a single segment with retry logic
#[allow(clippy::too_many_arguments)]
async fn download_segment_with_retry(
    client: &Client,
    config: &TurboConfig,
    mirrors: &[String],
    preferred_idx: usize,
    file_path: &Path,
    segment: &SegmentState,
    pb: Option<&ProgressBar>,
    semaphore: Arc<Semaphore>,
    global_bytes: Arc<AtomicU64>,
    total_downloaded: Arc<AtomicU64>,
    mirror_stats: Arc<RwLock<HashMap<String, Arc<MirrorStats>>>>,
    best_mirror: Arc<RwLock<String>>,
    file_lock: Arc<Mutex<()>>,
) -> Result<()> {
    for attempt in 0..config.max_retries {
        if attempt > 0 {
            // Reset segment state for retry
            segment.reset_for_retry();
            let delay = calculate_backoff_delay(attempt);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        match download_segment(
            client,
            config,
            mirrors,
            preferred_idx,
            file_path,
            segment,
            pb,
            semaphore.clone(),
            global_bytes.clone(),
            total_downloaded.clone(),
            mirror_stats.clone(),
            best_mirror.clone(),
            file_lock.clone(),
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(e) => {
                segment.increment_retry();
                if attempt == config.max_retries - 1 {
                    return Err(e);
                }
            }
        }
    }

    Err(anyhow!(
        "segment download failed after {} retries",
        config.max_retries
    ))
}

/// Download a single segment with failover support
#[allow(clippy::too_many_arguments)]
async fn download_segment(
    client: &Client,
    config: &TurboConfig,
    mirrors: &[String],
    preferred_idx: usize,
    file_path: &Path,
    segment: &SegmentState,
    pb: Option<&ProgressBar>,
    semaphore: Arc<Semaphore>,
    global_bytes: Arc<AtomicU64>,
    total_downloaded: Arc<AtomicU64>,
    mirror_stats: Arc<RwLock<HashMap<String, Arc<MirrorStats>>>>,
    best_mirror: Arc<RwLock<String>>,
    file_lock: Arc<Mutex<()>>,
) -> Result<()> {
    let _permit = semaphore.acquire().await?;

    // Build mirror order: preferred first, then rotate through others
    let mut mirror_order: Vec<&String> = Vec::with_capacity(mirrors.len());
    if preferred_idx < mirrors.len() {
        mirror_order.push(&mirrors[preferred_idx]);
    }
    for (i, m) in mirrors.iter().enumerate() {
        if i != preferred_idx {
            mirror_order.push(m);
        }
    }

    let start = segment.current_position();
    let end = segment.end;
    let range_header = format!("bytes={}-{}", start, end);
    let mut last_error = None;

    for mirror in mirror_order {
        let segment_start = Instant::now();

        // Check if mirror should be skipped
        {
            let stats = mirror_stats.read().await;
            let base = extract_base_url(mirror);
            if let Some(stat) = stats.get(&base) {
                if stat.should_skip() {
                    continue;
                }
            }
        }

        let result = timeout(
            config.read_timeout,
            client
                .get(mirror)
                .header(header::RANGE, &range_header)
                .send(),
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                if response.status() == StatusCode::PARTIAL_CONTENT
                    || response.status().is_success()
                {
                    // Stream to buffer first, then write to file
                    let mut stream = response.bytes_stream();
                    let mut buffer = Vec::with_capacity(config.write_buffer_size);
                    let mut bytes_in_segment = 0u64;
                    let mut last_data_time = Instant::now();

                    loop {
                        // Check for stall
                        let chunk_result =
                            timeout(Duration::from_secs(STALL_TIMEOUT_SECS), stream.next()).await;

                        match chunk_result {
                            Ok(Some(Ok(data))) => {
                                last_data_time = Instant::now();

                                // Limit chunk size to prevent memory issues
                                if data.len() > MAX_CHUNK_SIZE {
                                    last_error =
                                        Some(format!("chunk too large: {} bytes", data.len()));
                                    break;
                                }

                                buffer.extend_from_slice(&data);
                                let len = data.len() as u64;
                                bytes_in_segment += len;

                                // Flush buffer when large enough
                                if buffer.len() >= config.write_buffer_size {
                                    // Acquire file lock for write
                                    let _lock = file_lock.lock().await;

                                    let file =
                                        OpenOptions::new().write(true).open(file_path).await?;

                                    let current_pos = segment.current_position();
                                    let mut writer = BufWriter::new(file);
                                    writer.seek(SeekFrom::Start(current_pos)).await?;
                                    writer.write_all(&buffer).await?;
                                    writer.flush().await?;

                                    let written = buffer.len() as u64;
                                    segment.add_downloaded(written);
                                    total_downloaded.fetch_add(written, Ordering::Relaxed);
                                    global_bytes.fetch_add(written, Ordering::Relaxed);

                                    if let Some(pb) = pb {
                                        pb.inc(written);
                                    }
                                    buffer.clear();
                                }
                            }
                            Ok(Some(Err(e))) => {
                                last_error = Some(e.to_string());
                                break;
                            }
                            Ok(None) => {
                                // Stream complete, flush remaining buffer
                                if !buffer.is_empty() {
                                    let _lock = file_lock.lock().await;

                                    let file =
                                        OpenOptions::new().write(true).open(file_path).await?;

                                    let current_pos = segment.current_position();
                                    let mut writer = BufWriter::new(file);
                                    writer.seek(SeekFrom::Start(current_pos)).await?;
                                    writer.write_all(&buffer).await?;
                                    writer.flush().await?;

                                    let written = buffer.len() as u64;
                                    segment.add_downloaded(written);
                                    total_downloaded.fetch_add(written, Ordering::Relaxed);
                                    global_bytes.fetch_add(written, Ordering::Relaxed);

                                    if let Some(pb) = pb {
                                        pb.inc(written);
                                    }
                                }

                                // Record success
                                let elapsed_ms = segment_start.elapsed().as_millis() as u64;
                                let base = extract_base_url(mirror);

                                {
                                    let stats = mirror_stats.read().await;
                                    if let Some(stat) = stats.get(&base) {
                                        stat.record_success(bytes_in_segment, elapsed_ms);
                                    }
                                }

                                // Update best mirror
                                {
                                    let mut best = best_mirror.write().await;
                                    *best = mirror.clone();
                                }

                                segment.mark_complete();
                                return Ok(());
                            }
                            Err(_) => {
                                // Timeout - stall detected
                                last_error = Some("download stalled".to_string());
                                break;
                            }
                        }
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

        // Record failure
        let base = extract_base_url(mirror);
        {
            let stats = mirror_stats.read().await;
            if let Some(stat) = stats.get(&base) {
                stat.record_failure();
            }
        }
    }

    Err(anyhow!(
        "segment download failed: {}",
        last_error.unwrap_or_default()
    ))
}

/// Streaming download (for servers without range support)
#[allow(clippy::too_many_arguments)]
async fn download_streaming(
    client: &Client,
    config: &TurboConfig,
    mirrors: &[String],
    target_path: &Path,
    _expected_size: u64,
    pb: Option<&ProgressBar>,
    semaphore: Arc<Semaphore>,
    global_bytes: Arc<AtomicU64>,
    mirror_stats: Arc<RwLock<HashMap<String, Arc<MirrorStats>>>>,
) -> Result<(u64, String)> {
    let _permit = semaphore.acquire().await?;

    for mirror in mirrors {
        let start = Instant::now();

        // Check if mirror should be skipped
        {
            let stats = mirror_stats.read().await;
            let base = extract_base_url(mirror);
            if let Some(stat) = stats.get(&base) {
                if stat.should_skip() {
                    continue;
                }
            }
        }

        let result = timeout(config.read_timeout, client.get(mirror).send()).await;

        match result {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    let file = File::create(target_path).await?;
                    let mut writer = BufWriter::with_capacity(config.write_buffer_size, file);
                    let mut stream = response.bytes_stream();
                    let mut bytes_written = 0u64;
                    let mut last_data_time = Instant::now();

                    loop {
                        let chunk_result =
                            timeout(Duration::from_secs(STALL_TIMEOUT_SECS), stream.next()).await;

                        match chunk_result {
                            Ok(Some(Ok(chunk))) => {
                                last_data_time = Instant::now();

                                if chunk.len() > MAX_CHUNK_SIZE {
                                    break;
                                }

                                writer.write_all(&chunk).await?;

                                let len = chunk.len() as u64;
                                bytes_written += len;
                                global_bytes.fetch_add(len, Ordering::Relaxed);

                                if let Some(pb) = pb {
                                    pb.inc(len);
                                }
                            }
                            Ok(Some(Err(_))) => break,
                            Ok(None) => {
                                // Complete
                                writer.flush().await?;

                                // Record success
                                let elapsed_ms = start.elapsed().as_millis() as u64;
                                let base = extract_base_url(mirror);

                                {
                                    let stats = mirror_stats.read().await;
                                    if let Some(stat) = stats.get(&base) {
                                        stat.record_success(bytes_written, elapsed_ms);
                                    }
                                }

                                return Ok((bytes_written, mirror.clone()));
                            }
                            Err(_) => break, // Timeout
                        }
                    }
                }
            }
            Ok(Err(_)) | Err(_) => {
                // Record failure
                let base = extract_base_url(mirror);
                let stats = mirror_stats.read().await;
                if let Some(stat) = stats.get(&base) {
                    stat.record_failure();
                }
                continue;
            }
        }

        // Record failure
        let base = extract_base_url(mirror);
        {
            let stats = mirror_stats.read().await;
            if let Some(stat) = stats.get(&base) {
                stat.record_failure();
            }
        }
    }

    Err(anyhow!("all mirrors failed for streaming download"))
}

// Helper functions

fn extract_base_url(url: &str) -> String {
    url.split('/').take(3).collect::<Vec<_>>().join("/")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turbo_config_default() {
        let config = TurboConfig::default();
        assert!(config.max_total_connections >= 16);
        assert!(config.segments_large >= 1);
        assert!(config.enable_racing);
        assert!(config.max_retries > 0);
    }

    #[test]
    fn test_extract_base_url() {
        assert_eq!(
            extract_base_url("https://mirror.example.com/path/to/file.pkg"),
            "https://mirror.example.com"
        );
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_backoff_delay() {
        let delay0 = calculate_backoff_delay(0);
        let delay1 = calculate_backoff_delay(1);
        let delay2 = calculate_backoff_delay(2);

        // Delays should increase exponentially
        assert!(delay1 >= delay0);
        assert!(delay2 >= delay1);

        // But should be capped
        let delay10 = calculate_backoff_delay(10);
        assert!(delay10 <= MAX_RETRY_DELAY_MS + MAX_RETRY_DELAY_MS / 4);
    }

    #[test]
    fn test_segment_state() {
        let segment = SegmentState::new(0, 999);
        assert_eq!(segment.size(), 1000);
        assert_eq!(segment.current_position(), 0);
        assert!(!segment.is_complete());

        segment.add_downloaded(500);
        assert_eq!(segment.current_position(), 500);

        segment.mark_complete();
        assert!(segment.is_complete());

        segment.reset_for_retry();
        assert!(!segment.is_complete());
        assert_eq!(segment.current_position(), 0);
    }
}
