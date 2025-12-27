/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 */

//! Adaptive scheduler for parallel downloads.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Scheduler for adaptive parallelism based on throughput
#[derive(Debug)]
pub struct Scheduler {
    /// Current number of active connections
    active_connections: AtomicUsize,
    /// Maximum allowed connections
    max_connections: usize,
    /// Minimum connections
    min_connections: usize,
    /// Target connections (adjusted dynamically)
    target_connections: AtomicUsize,
    /// Total bytes downloaded (for throughput calculation)
    total_bytes: AtomicU64,
    /// Start time
    start_time: Instant,
    /// Last throughput measurement
    last_throughput: AtomicU64,
}

impl Scheduler {
    pub fn new(initial: usize, min: usize, max: usize) -> Self {
        let initial = initial.clamp(min, max);
        Self {
            active_connections: AtomicUsize::new(0),
            max_connections: max,
            min_connections: min,
            target_connections: AtomicUsize::new(initial),
            total_bytes: AtomicU64::new(0),
            start_time: Instant::now(),
            last_throughput: AtomicU64::new(0),
        }
    }

    /// Check if we can start a new connection
    pub fn can_start(&self) -> bool {
        let active = self.active_connections.load(Ordering::Relaxed);
        let target = self.target_connections.load(Ordering::Relaxed);
        active < target
    }

    /// Try to acquire a connection slot
    pub fn try_acquire(&self) -> bool {
        let target = self.target_connections.load(Ordering::Relaxed);
        loop {
            let current = self.active_connections.load(Ordering::Relaxed);
            if current >= target {
                return false;
            }
            if self
                .active_connections
                .compare_exchange_weak(current, current + 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }

    /// Release a connection slot
    pub fn release(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record downloaded bytes
    pub fn record_bytes(&self, bytes: u64) {
        self.total_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Get current throughput in bytes/second
    pub fn throughput(&self) -> u64 {
        let bytes = self.total_bytes.load(Ordering::Relaxed);
        let elapsed = self.start_time.elapsed().as_millis() as u64;
        if elapsed == 0 {
            return 0;
        }
        (bytes * 1000) / elapsed
    }

    /// Adjust parallelism based on throughput
    pub fn adjust(&self) {
        let current_throughput = self.throughput();
        let last_throughput = self
            .last_throughput
            .swap(current_throughput, Ordering::Relaxed);

        let current_target = self.target_connections.load(Ordering::Relaxed);

        // If throughput improved by >10%, try adding more connections
        if current_throughput > last_throughput.saturating_add(last_throughput / 10) {
            let new_target = (current_target + 1).min(self.max_connections);
            self.target_connections.store(new_target, Ordering::Relaxed);
        }
        // If throughput dropped by >20%, reduce connections
        else if current_throughput < last_throughput.saturating_sub(last_throughput / 5) {
            let new_target = current_target.saturating_sub(1).max(self.min_connections);
            self.target_connections.store(new_target, Ordering::Relaxed);
        }
    }

    /// Get current active connections
    pub fn active(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Get target connections
    pub fn target(&self) -> usize {
        self.target_connections.load(Ordering::Relaxed)
    }

    /// Get total bytes downloaded
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(4, 1, 16)
    }
}
