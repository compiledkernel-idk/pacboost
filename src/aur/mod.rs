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

//! AUR (Arch User Repository) support module.
//!
//! This module provides comprehensive AUR functionality including:
//! - RPC client with caching
//! - Dependency resolution with topological sorting
//! - PKGBUILD parsing and security validation
//! - Sandboxed package building

pub mod client;
pub mod resolver;
pub mod pkgbuild;
pub mod builder;

pub use client::{AurClient, AurPackageInfo};
pub use resolver::DependencyGraph;
pub use pkgbuild::{Pkgbuild, SecurityValidator, SecurityReport};
pub use builder::AurBuilder;

use serde::Deserialize;

/// AUR RPC API response wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct AurRpcResponse {
    pub version: u32,
    #[serde(rename = "type")]
    pub response_type: String,
    pub resultcount: usize,
    pub results: Vec<AurPackageInfo>,
    pub error: Option<String>,
}
