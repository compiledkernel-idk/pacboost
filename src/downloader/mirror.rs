/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 */

//! Mirror pool with intelligent selection and adaptive failover.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// Mirror with performance tracking
#[derive(Debug)]
pub struct Mirror {
    pub url: String,
    /// Total bytes downloaded from this mirror
    bytes_downloaded: AtomicU64,
    /// Number of successful requests
    successes: AtomicU64,
    /// Number of failed requests
    failures: AtomicU64,
    /// Cumulative download time in milliseconds
    total_time_ms: AtomicU64,
}

impl Mirror {
    pub fn new(url: String) -> Self {
        Self {
            url,
            bytes_downloaded: AtomicU64::new(0),
            successes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            total_time_ms: AtomicU64::new(0),
        }
    }

    /// Record a successful download
    pub fn record_success(&self, bytes: u64, time_ms: u64) {
        self.bytes_downloaded.fetch_add(bytes, Ordering::Relaxed);
        self.successes.fetch_add(1, Ordering::Relaxed);
        self.total_time_ms.fetch_add(time_ms, Ordering::Relaxed);
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Get throughput in bytes/second (0 if no data)
    pub fn throughput(&self) -> u64 {
        let bytes = self.bytes_downloaded.load(Ordering::Relaxed);
        let time_ms = self.total_time_ms.load(Ordering::Relaxed);
        if time_ms == 0 {
            return 0;
        }
        (bytes * 1000) / time_ms
    }

    /// Get success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        let successes = self.successes.load(Ordering::Relaxed);
        let failures = self.failures.load(Ordering::Relaxed);
        let total = successes + failures;
        if total == 0 {
            return 1.0; // Assume good until proven otherwise
        }
        successes as f64 / total as f64
    }

    /// Score for ranking (higher is better)
    pub fn score(&self) -> u64 {
        let throughput = self.throughput();
        let success_rate = self.success_rate();
        (throughput as f64 * success_rate) as u64
    }
}

/// Pool of mirrors with intelligent selection
#[derive(Debug)]
pub struct MirrorPool {
    mirrors: Vec<Arc<Mirror>>,
    /// Round-robin index for fair distribution
    next_index: AtomicUsize,
}

impl MirrorPool {
    pub fn new(urls: Vec<String>) -> Self {
        let mirrors = urls
            .into_iter()
            .map(|url| Arc::new(Mirror::new(url)))
            .collect();
        Self {
            mirrors,
            next_index: AtomicUsize::new(0),
        }
    }

    /// Get the next mirror using round-robin
    pub fn next(&self) -> Option<Arc<Mirror>> {
        if self.mirrors.is_empty() {
            return None;
        }
        let idx = self.next_index.fetch_add(1, Ordering::Relaxed) % self.mirrors.len();
        Some(self.mirrors[idx].clone())
    }

    /// Get the best performing mirror
    pub fn best(&self) -> Option<Arc<Mirror>> {
        self.mirrors.iter().max_by_key(|m| m.score()).cloned()
    }

    /// Get all mirrors sorted by score (best first)
    pub fn ranked(&self) -> Vec<Arc<Mirror>> {
        let mut mirrors: Vec<_> = self.mirrors.to_vec();
        mirrors.sort_by(|a, b| b.score().cmp(&a.score()));
        mirrors
    }

    /// Get N fastest mirrors for racing
    pub fn fastest(&self, n: usize) -> Vec<Arc<Mirror>> {
        self.ranked().into_iter().take(n).collect()
    }

    /// Get all mirror URLs
    pub fn all_urls(&self) -> Vec<String> {
        self.mirrors.iter().map(|m| m.url.clone()).collect()
    }

    /// Number of mirrors
    pub fn len(&self) -> usize {
        self.mirrors.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.mirrors.is_empty()
    }
}

impl Clone for MirrorPool {
    fn clone(&self) -> Self {
        Self {
            mirrors: self.mirrors.clone(),
            next_index: AtomicUsize::new(self.next_index.load(Ordering::Relaxed)),
        }
    }
}
