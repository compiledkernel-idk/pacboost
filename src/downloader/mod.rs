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
//! multi-mirror racing, and adaptive parallelism.

mod engine;
mod mirror;
mod segment;
mod scheduler;
mod benchmark;
pub mod cache;

pub use engine::{DownloadEngine, DownloadTask, DownloadResult};
pub use benchmark::run_benchmark;

use std::time::Duration;

/// Configuration for the download engine
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
        Self {
            max_connections: 16,
            segments: 8,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(300),
            http2: true,
            segment_threshold: 1024 * 1024, // 1 MiB
        }
    }
}
