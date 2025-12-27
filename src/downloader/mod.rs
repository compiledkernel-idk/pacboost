/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

//! High-performance download engine with segmented parallel downloads,
//! multi-mirror racing, adaptive parallelism, and turbo mode for 2x+ speeds.

mod benchmark;
pub mod cache;
mod engine;
mod mirror;
mod scheduler;
mod segment;
mod turbo;

// Legacy exports for backwards compatibility
pub use benchmark::run_benchmark;
pub use engine::DownloadTask;

// NEW: Turbo engine exports (2x+ faster)
pub use turbo::{TurboConfig, TurboEngine, TurboTask};

use std::time::Duration;

/// Configuration for the download engine (legacy)
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// Maximum concurrent connections per host
    pub max_connections: usize,
    /// Number of segments to split large files into
    pub segments: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Whether to enable HTTP/2
    pub http2: bool,
    /// Minimum file size for segmented downloads (bytes)
    pub segment_threshold: u64,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        // Get CPU core count for optimal parallelism
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Self {
            // Aggressive parallelism based on CPU cores
            max_connections: (cores * 8).max(32),
            segments: 16, // More segments for better parallelism
            connect_timeout: Duration::from_secs(3),
            request_timeout: Duration::from_secs(120),
            http2: true,
            segment_threshold: 512 * 1024, // 512 KB - segment even small files
        }
    }
}

impl DownloadConfig {
    /// Create a configuration optimized for fast networks
    pub fn turbo() -> Self {
        TurboConfig::default().into()
    }
}

impl From<TurboConfig> for DownloadConfig {
    fn from(turbo: TurboConfig) -> Self {
        Self {
            max_connections: turbo.max_total_connections,
            segments: turbo.segments_large,
            connect_timeout: turbo.connect_timeout,
            request_timeout: turbo.read_timeout,
            http2: turbo.http2_multiplexing,
            segment_threshold: turbo.medium_file_threshold,
        }
    }
}

/// Helper to create a TurboTask from a DownloadTask for migration
impl From<DownloadTask> for TurboTask {
    fn from(task: DownloadTask) -> Self {
        let mut turbo = TurboTask::new(task.mirrors, task.filename);
        if let Some(size) = task.expected_size {
            turbo = turbo.with_size(size);
        }
        turbo
    }
}
