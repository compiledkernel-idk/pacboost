/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 */

//! Segment management for parallel downloads.

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;

/// State of a download segment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentState {
    Pending,
    InProgress,
    Complete,
    Failed,
}

/// A segment of a file to download
#[derive(Debug)]
pub struct Segment {
    pub id: usize,
    /// Start byte offset (inclusive)
    pub start: u64,
    /// End byte offset (inclusive)
    pub end: u64,
    /// Bytes downloaded so far
    downloaded: AtomicU64,
    /// Whether this segment is complete
    complete: AtomicBool,
    /// Whether this segment is currently being downloaded
    in_progress: AtomicBool,
}

impl Segment {
    pub fn new(id: usize, start: u64, end: u64) -> Self {
        Self {
            id,
            start,
            end,
            downloaded: AtomicU64::new(0),
            complete: AtomicBool::new(false),
            in_progress: AtomicBool::new(false),
        }
    }

    /// Total size of this segment
    pub fn size(&self) -> u64 {
        self.end - self.start + 1
    }

    /// Bytes remaining to download
    pub fn remaining(&self) -> u64 {
        let downloaded = self.downloaded.load(Ordering::Relaxed);
        self.size().saturating_sub(downloaded)
    }

    /// Current byte position (for Range header)
    pub fn current_position(&self) -> u64 {
        self.start + self.downloaded.load(Ordering::Relaxed)
    }

    /// Add downloaded bytes
    pub fn add_progress(&self, bytes: u64) {
        self.downloaded.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Get downloaded bytes
    pub fn downloaded(&self) -> u64 {
        self.downloaded.load(Ordering::Relaxed)
    }

    /// Mark as complete
    pub fn mark_complete(&self) {
        self.complete.store(true, Ordering::Release);
        self.in_progress.store(false, Ordering::Release);
    }

    /// Mark as in progress
    pub fn mark_in_progress(&self) {
        self.in_progress.store(true, Ordering::Release);
    }

    /// Mark as failed (reset in_progress)
    pub fn mark_failed(&self) {
        self.in_progress.store(false, Ordering::Release);
    }

    /// Check if complete
    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    /// Check if in progress
    pub fn is_in_progress(&self) -> bool {
        self.in_progress.load(Ordering::Acquire)
    }

    /// Get current state
    pub fn state(&self) -> SegmentState {
        if self.is_complete() {
            SegmentState::Complete
        } else if self.is_in_progress() {
            SegmentState::InProgress
        } else {
            SegmentState::Pending
        }
    }

    /// Generate HTTP Range header value
    pub fn range_header(&self) -> String {
        format!("bytes={}-{}", self.current_position(), self.end)
    }
}

/// Manager for all segments of a file
#[derive(Debug)]
pub struct SegmentManager {
    segments: Vec<Arc<Segment>>,
    total_size: u64,
}

impl SegmentManager {
    /// Create segments for a file
    pub fn new(total_size: u64, num_segments: usize) -> Self {
        let num_segments = num_segments.max(1);
        let segment_size = (total_size + num_segments as u64 - 1) / num_segments as u64;
        
        let mut segments = Vec::with_capacity(num_segments);
        let mut start = 0u64;
        
        while start < total_size {
            let end = (start + segment_size - 1).min(total_size - 1);
            segments.push(Arc::new(Segment::new(segments.len(), start, end)));
            start = end + 1;
        }
        
        Self {
            segments,
            total_size,
        }
    }

    /// Create single segment (for small files or non-range servers)
    pub fn single(total_size: u64) -> Self {
        Self::new(total_size, 1)
    }

    /// Get next pending segment
    pub fn next_pending(&self) -> Option<Arc<Segment>> {
        self.segments
            .iter()
            .find(|s| s.state() == SegmentState::Pending)
            .cloned()
    }

    /// Get all pending segments
    pub fn pending(&self) -> Vec<Arc<Segment>> {
        self.segments
            .iter()
            .filter(|s| s.state() == SegmentState::Pending)
            .cloned()
            .collect()
    }

    /// Get total downloaded bytes
    pub fn total_downloaded(&self) -> u64 {
        self.segments.iter().map(|s| s.downloaded()).sum()
    }

    /// Get total remaining bytes
    pub fn total_remaining(&self) -> u64 {
        self.total_size.saturating_sub(self.total_downloaded())
    }

    /// Check if all segments are complete
    pub fn is_complete(&self) -> bool {
        self.segments.iter().all(|s| s.is_complete())
    }

    /// Get completion percentage
    pub fn progress_percent(&self) -> f64 {
        if self.total_size == 0 {
            return 100.0;
        }
        (self.total_downloaded() as f64 / self.total_size as f64) * 100.0
    }

    /// Get number of segments
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Get segment by id
    pub fn get(&self, id: usize) -> Option<Arc<Segment>> {
        self.segments.get(id).cloned()
    }

    /// Get all segments
    pub fn all(&self) -> Vec<Arc<Segment>> {
        self.segments.clone()
    }
}
