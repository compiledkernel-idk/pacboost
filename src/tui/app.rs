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

//! TUI application state management.

use crossterm::event::KeyCode;
use sysinfo::{System, Disks};
use std::collections::VecDeque;

/// Active tab in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Packages,
    Search,
    Downloads,
    Settings,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Packages => "Packages",
            Tab::Search => "Search",
            Tab::Downloads => "Downloads",
            Tab::Settings => "Settings",
        }
    }

    pub fn all() -> &'static [Tab] {
        &[Tab::Dashboard, Tab::Packages, Tab::Search, Tab::Downloads, Tab::Settings]
    }
}

/// Download item in the queue
#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub name: String,
    pub url: String,
    pub progress: f64,
    pub size_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub status: DownloadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
    Paused,
}

/// Package info for display
#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub installed: bool,
    pub size: String,
    pub repo: String,
}

/// Log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// System metrics
#[derive(Debug, Clone, Default)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    pub uptime_secs: u64,
}

/// TUI application state
pub struct App {
    /// Current active tab
    pub active_tab: Tab,
    /// Tab index for navigation
    pub tab_index: usize,
    /// System information
    pub system: System,
    /// System metrics history
    pub cpu_history: VecDeque<f32>,
    pub memory_history: VecDeque<f32>,
    /// Current system metrics
    pub metrics: SystemMetrics,
    /// Download queue
    pub downloads: Vec<DownloadItem>,
    /// Package list (cached)
    pub packages: Vec<PackageInfo>,
    /// Selected package index
    pub selected_package: usize,
    /// Search query
    pub search_query: String,
    /// Search mode active
    pub search_mode: bool,
    /// Log entries
    pub logs: VecDeque<LogEntry>,
    /// Tick counter
    pub tick: u64,
    /// Show help overlay
    pub show_help: bool,
}

impl App {
    /// Create a new app instance
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let mut app = Self {
            active_tab: Tab::Dashboard,
            tab_index: 0,
            system,
            cpu_history: VecDeque::with_capacity(60),
            memory_history: VecDeque::with_capacity(60),
            metrics: SystemMetrics::default(),
            downloads: Vec::new(),
            packages: Vec::new(),
            selected_package: 0,
            search_query: String::new(),
            search_mode: false,
            logs: VecDeque::with_capacity(100),
            tick: 0,
            show_help: false,
        };

        // Initialize with some sample data
        app.add_log(LogLevel::Info, "Pacboost TUI started");
        app.add_log(LogLevel::Success, "System information loaded");
        app.update_metrics();
        app.load_sample_packages();

        app
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: KeyCode) {
        if self.show_help {
            self.show_help = false;
            return;
        }

        if self.search_mode {
            match key {
                KeyCode::Enter => {
                    self.search_mode = false;
                    self.add_log(LogLevel::Info, &format!("Searching for: {}", self.search_query));
                }
                KeyCode::Esc => {
                    self.search_mode = false;
                    self.search_query.clear();
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                }
                _ => {}
            }
            return;
        }

        match key {
            // Tab navigation
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                self.tab_index = (self.tab_index + 1) % Tab::all().len();
                self.active_tab = Tab::all()[self.tab_index];
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                if self.tab_index == 0 {
                    self.tab_index = Tab::all().len() - 1;
                } else {
                    self.tab_index -= 1;
                }
                self.active_tab = Tab::all()[self.tab_index];
            }

            // List navigation
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_package > 0 {
                    self.selected_package -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_package < self.packages.len().saturating_sub(1) {
                    self.selected_package += 1;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected_package = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.selected_package = self.packages.len().saturating_sub(1);
            }

            // Actions
            KeyCode::Char('/') => {
                self.search_mode = true;
                self.active_tab = Tab::Search;
                self.tab_index = 2;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            KeyCode::Char('r') => {
                self.refresh();
            }
            KeyCode::Char('1') => {
                self.tab_index = 0;
                self.active_tab = Tab::Dashboard;
            }
            KeyCode::Char('2') => {
                self.tab_index = 1;
                self.active_tab = Tab::Packages;
            }
            KeyCode::Char('3') => {
                self.tab_index = 2;
                self.active_tab = Tab::Search;
            }
            KeyCode::Char('4') => {
                self.tab_index = 3;
                self.active_tab = Tab::Downloads;
            }
            KeyCode::Char('5') => {
                self.tab_index = 4;
                self.active_tab = Tab::Settings;
            }

            _ => {}
        }
    }

    /// Tick - called every frame
    pub fn tick(&mut self) {
        self.tick += 1;

        // Update metrics every 10 ticks (1 second)
        if self.tick % 10 == 0 {
            self.update_metrics();
        }

        // Simulate download progress
        for download in &mut self.downloads {
            if download.status == DownloadStatus::Downloading {
                download.downloaded_bytes += download.speed_bps / 10;
                if download.downloaded_bytes >= download.size_bytes {
                    download.downloaded_bytes = download.size_bytes;
                    download.progress = 100.0;
                    download.status = DownloadStatus::Completed;
                } else {
                    download.progress = (download.downloaded_bytes as f64 / download.size_bytes as f64) * 100.0;
                }
            }
        }
    }

    /// Update system metrics
    fn update_metrics(&mut self) {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();

        let cpu_usage = self.system.global_cpu_usage();
        let mem_used = self.system.used_memory();
        let mem_total = self.system.total_memory();

        // Update history
        if self.cpu_history.len() >= 60 {
            self.cpu_history.pop_front();
        }
        self.cpu_history.push_back(cpu_usage);

        let mem_percent = (mem_used as f32 / mem_total as f32) * 100.0;
        if self.memory_history.len() >= 60 {
            self.memory_history.pop_front();
        }
        self.memory_history.push_back(mem_percent);

        // Get disk info  
        let disks = Disks::new_with_refreshed_list();
        let (disk_used, disk_total) = disks
            .iter()
            .filter(|d| d.mount_point() == std::path::Path::new("/"))
            .map(|d| (d.total_space() - d.available_space(), d.total_space()))
            .next()
            .unwrap_or((0, 0));

        self.metrics = SystemMetrics {
            cpu_usage,
            memory_used: mem_used,
            memory_total: mem_total,
            disk_used,
            disk_total,
            uptime_secs: System::uptime(),
        };
    }

    /// Refresh all data
    fn refresh(&mut self) {
        self.system.refresh_all();
        self.update_metrics();
        self.add_log(LogLevel::Info, "Data refreshed");
    }

    /// Add a log entry
    pub fn add_log(&mut self, level: LogLevel, message: &str) {
        use chrono::Local;
        
        if self.logs.len() >= 100 {
            self.logs.pop_front();
        }
        
        self.logs.push_back(LogEntry {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            level,
            message: message.to_string(),
        });
    }

    /// Load sample packages for demo
    fn load_sample_packages(&mut self) {
        self.packages = vec![
            PackageInfo {
                name: "firefox".to_string(),
                version: "120.0.1".to_string(),
                description: "Standalone web browser from mozilla.org".to_string(),
                installed: true,
                size: "234 MB".to_string(),
                repo: "extra".to_string(),
            },
            PackageInfo {
                name: "linux".to_string(),
                version: "6.6.7-arch1-1".to_string(),
                description: "The Linux kernel and modules".to_string(),
                installed: true,
                size: "143 MB".to_string(),
                repo: "core".to_string(),
            },
            PackageInfo {
                name: "rust".to_string(),
                version: "1.74.1".to_string(),
                description: "Systems programming language".to_string(),
                installed: true,
                size: "678 MB".to_string(),
                repo: "extra".to_string(),
            },
            PackageInfo {
                name: "neovim".to_string(),
                version: "0.9.4".to_string(),
                description: "Fork of Vim aiming to improve the codebase".to_string(),
                installed: true,
                size: "12 MB".to_string(),
                repo: "extra".to_string(),
            },
            PackageInfo {
                name: "git".to_string(),
                version: "2.43.0".to_string(),
                description: "The fast distributed version control system".to_string(),
                installed: true,
                size: "35 MB".to_string(),
                repo: "extra".to_string(),
            },
            PackageInfo {
                name: "yay".to_string(),
                version: "12.3.0".to_string(),
                description: "Yet another yogurt - An AUR Helper".to_string(),
                installed: false,
                size: "8 MB".to_string(),
                repo: "aur".to_string(),
            },
        ];
    }

    /// Add a download to the queue
    pub fn add_download(&mut self, name: &str, url: &str, size: u64) {
        self.downloads.push(DownloadItem {
            name: name.to_string(),
            url: url.to_string(),
            progress: 0.0,
            size_bytes: size,
            downloaded_bytes: 0,
            speed_bps: 10_000_000, // 10 MB/s demo speed
            status: DownloadStatus::Downloading,
        });
        self.add_log(LogLevel::Info, &format!("Started download: {}", name));
    }

    /// Get memory usage percentage
    pub fn memory_percent(&self) -> f32 {
        if self.metrics.memory_total == 0 {
            return 0.0;
        }
        (self.metrics.memory_used as f32 / self.metrics.memory_total as f32) * 100.0
    }

    /// Get disk usage percentage
    pub fn disk_percent(&self) -> f32 {
        if self.metrics.disk_total == 0 {
            return 0.0;
        }
        (self.metrics.disk_used as f32 / self.metrics.disk_total as f32) * 100.0
    }

    /// Format bytes to human readable
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Format uptime
    pub fn format_uptime(&self) -> String {
        let secs = self.metrics.uptime_secs;
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;

        if days > 0 {
            format!("{}d {}h {}m", days, hours, mins)
        } else if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
