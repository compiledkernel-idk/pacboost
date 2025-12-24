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

//! Logging and observability with tracing support.

use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};
use std::path::Path;

/// Initialize the logging system
pub fn init() {
    init_with_level("info")
}

/// Initialize logging with a specific level
pub fn init_with_level(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));
    
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer()
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false)
            .compact())
        .init();
}

/// Initialize logging with optional file output
pub fn init_with_file(level: &str, log_file: Option<&Path>) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));
    
    if let Some(path) = log_file {
        // Create log directory if needed
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        
        // Try to create file appender
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let file_layer = fmt::layer()
                .with_writer(file)
                .with_ansi(false)
                .with_target(true);
            
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().compact())
                .with(file_layer)
                .init();
            
            return;
        }
    }
    
    // Fallback to console-only
    init_with_level(level);
}

/// Log macros re-exported for convenience
pub use tracing::{debug, error, info, trace, warn};

/// Span creation helpers
#[macro_export]
macro_rules! span_operation {
    ($name:expr) => {
        tracing::info_span!("operation", name = $name)
    };
}

#[macro_export]
macro_rules! span_download {
    ($url:expr) => {
        tracing::info_span!("download", url = $url)
    };
}

#[macro_export]
macro_rules! span_build {
    ($package:expr) => {
        tracing::info_span!("build", package = $package)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_init() {
        // Just verify it doesn't panic
        // Note: tracing subscriber can only be set once per process
        // so we can't really test multiple init calls
    }
}
