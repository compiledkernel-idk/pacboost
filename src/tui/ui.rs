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

//! TUI rendering.

use ratatui::{
    prelude::*,
    widgets::*,
};
use super::app::{App, Tab, LogLevel, DownloadStatus};

/// Main draw function
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Main content
            Constraint::Length(3),  // Footer/status bar
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_main(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    // Draw help overlay if active
    if app.show_help {
        draw_help_overlay(f);
    }
}

/// Draw header with tabs
fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let style = if i == app.tab_index {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(format!(" {} ", tab.title())).style(style)
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                " 🚀 PACBOOST ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )))
        .highlight_style(Style::default().fg(Color::Cyan))
        .divider(Span::raw("|"));

    f.render_widget(tabs, area);
}

/// Draw main content based on active tab
fn draw_main(f: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        Tab::Dashboard => draw_dashboard(f, app, area),
        Tab::Packages => draw_packages(f, app, area),
        Tab::Search => draw_search(f, app, area),
        Tab::Downloads => draw_downloads(f, app, area),
        Tab::Settings => draw_settings(f, app, area),
    }
}

/// Draw dashboard with system info
fn draw_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[1]);

    // System info
    draw_system_info(f, app, left_chunks[0]);
    
    // CPU chart
    draw_cpu_chart(f, app, left_chunks[1]);
    
    // Memory chart
    draw_memory_chart(f, app, right_chunks[0]);
    
    // Logs
    draw_logs(f, app, right_chunks[1]);
}

/// Draw system information block
fn draw_system_info(f: &mut Frame, app: &App, area: Rect) {
    let info_items = vec![
        Line::from(vec![
            Span::styled("CPU: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{:.1}%", app.metrics.cpu_usage),
                Style::default().fg(if app.metrics.cpu_usage > 80.0 { Color::Red } else { Color::Green }),
            ),
        ]),
        Line::from(vec![
            Span::styled("Memory: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{} / {}", 
                    App::format_bytes(app.metrics.memory_used),
                    App::format_bytes(app.metrics.memory_total)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Disk: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{} / {}", 
                    App::format_bytes(app.metrics.disk_used),
                    App::format_bytes(app.metrics.disk_total)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Uptime: ", Style::default().fg(Color::Cyan)),
            Span::styled(app.format_uptime(), Style::default().fg(Color::Yellow)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title(Span::styled(" System Info ", Style::default().fg(Color::Cyan)));

    let paragraph = Paragraph::new(info_items).block(block);
    f.render_widget(paragraph, area);
}

/// Draw CPU usage chart
fn draw_cpu_chart(f: &mut Frame, app: &App, area: Rect) {
    let data: Vec<(f64, f64)> = app.cpu_history
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v as f64))
        .collect();

    let datasets = vec![
        Dataset::default()
            .name("CPU")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&data),
    ];

    let chart = Chart::new(datasets)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(Span::styled(" CPU Usage ", Style::default().fg(Color::Cyan))))
        .x_axis(Axis::default()
            .style(Style::default().fg(Color::DarkGray))
            .bounds([0.0, 60.0]))
        .y_axis(Axis::default()
            .style(Style::default().fg(Color::DarkGray))
            .labels(vec![
                Span::raw("0%"),
                Span::raw("50%"),
                Span::raw("100%"),
            ])
            .bounds([0.0, 100.0]));

    f.render_widget(chart, area);
}

/// Draw memory usage chart
fn draw_memory_chart(f: &mut Frame, app: &App, area: Rect) {
    let mem_percent = app.memory_percent();
    
    let gauge = Gauge::default()
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(Span::styled(" Memory Usage ", Style::default().fg(Color::Cyan))))
        .gauge_style(Style::default()
            .fg(if mem_percent > 80.0 { Color::Red } else if mem_percent > 60.0 { Color::Yellow } else { Color::Green }))
        .percent(mem_percent as u16)
        .label(format!("{:.1}%", mem_percent));

    f.render_widget(gauge, area);
}

/// Draw logs
fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let log_lines: Vec<Line> = app.logs
        .iter()
        .rev()
        .take(10)
        .map(|entry| {
            let (icon, color) = match entry.level {
                LogLevel::Info => ("ℹ", Color::Blue),
                LogLevel::Warning => ("⚠", Color::Yellow),
                LogLevel::Error => ("✗", Color::Red),
                LogLevel::Success => ("✓", Color::Green),
            };
            Line::from(vec![
                Span::styled(format!("{} ", entry.timestamp), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(&entry.message, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(log_lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(Span::styled(" Logs ", Style::default().fg(Color::Cyan))))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Draw packages list
fn draw_packages(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app.packages
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let installed_icon = if pkg.installed { "✓" } else { " " };
            let style = if i == app.selected_package {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            
            ListItem::new(Line::from(vec![
                Span::styled(format!("[{}] ", installed_icon), 
                    Style::default().fg(if pkg.installed { Color::Green } else { Color::DarkGray })),
                Span::styled(&pkg.name, Style::default().fg(Color::Cyan)),
                Span::styled(format!(" {} ", pkg.version), Style::default().fg(Color::Yellow)),
                Span::styled(format!("({})", pkg.repo), Style::default().fg(Color::DarkGray)),
            ])).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(Span::styled(
                format!(" Packages ({}) ", app.packages.len()),
                Style::default().fg(Color::Cyan),
            )))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, area);
}

/// Draw search interface
fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    // Search input
    let search_style = if app.search_mode {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let search_input = Paragraph::new(Line::from(vec![
        Span::styled("🔍 ", Style::default().fg(Color::Cyan)),
        Span::styled(&app.search_query, Style::default().fg(Color::White)),
        if app.search_mode {
            Span::styled("█", Style::default().fg(Color::Yellow))
        } else {
            Span::raw("")
        },
    ]))
    .block(Block::default()
        .borders(Borders::ALL)
        .border_style(search_style)
        .title(Span::styled(" Search (Press / to type) ", Style::default().fg(Color::Cyan))));

    f.render_widget(search_input, chunks[0]);

    // Results placeholder
    let help_text = if app.search_query.is_empty() {
        "Type to search packages across repos and AUR..."
    } else {
        "Press Enter to search, Esc to cancel"
    };

    let results = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(Span::styled(" Results ", Style::default().fg(Color::Cyan))))
        .alignment(Alignment::Center);

    f.render_widget(results, chunks[1]);
}

/// Draw downloads
fn draw_downloads(f: &mut Frame, app: &App, area: Rect) {
    if app.downloads.is_empty() {
        let msg = Paragraph::new("No active downloads")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(Span::styled(" Downloads ", Style::default().fg(Color::Cyan))))
            .alignment(Alignment::Center);
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app.downloads
        .iter()
        .map(|dl| {
            let status_style = match dl.status {
                DownloadStatus::Completed => Style::default().fg(Color::Green),
                DownloadStatus::Downloading => Style::default().fg(Color::Cyan),
                DownloadStatus::Failed => Style::default().fg(Color::Red),
                DownloadStatus::Paused => Style::default().fg(Color::Yellow),
                DownloadStatus::Pending => Style::default().fg(Color::DarkGray),
            };

            let progress_bar = create_progress_bar(dl.progress as u16, 20);

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(&dl.name, Style::default().fg(Color::White)),
                    Span::raw(" "),
                    Span::styled(format!("{:.1}%", dl.progress), status_style),
                ]),
                Line::from(vec![
                    Span::styled(progress_bar, Style::default().fg(Color::Cyan)),
                    Span::raw(" "),
                    Span::styled(
                        format!("{}/s", App::format_bytes(dl.speed_bps)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(Span::styled(
                format!(" Downloads ({}) ", app.downloads.len()),
                Style::default().fg(Color::Cyan),
            )));

    f.render_widget(list, area);
}

/// Draw settings
fn draw_settings(f: &mut Frame, _app: &App, area: Rect) {
    let build_jobs = format!("{}", num_cpus());
    let settings = vec![
        ("Parallel downloads", "8"),
        ("Mirror count", "5"),
        ("AUR support", "enabled"),
        ("Security scan", "enabled"),
        ("Compression", "disabled (fast)"),
        ("Build jobs", build_jobs.as_str()),
    ];

    let items: Vec<ListItem> = settings
        .iter()
        .map(|(key, value)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}: ", key), Style::default().fg(Color::Cyan)),
                Span::styled(*value, Style::default().fg(Color::Yellow)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(Span::styled(" Settings ", Style::default().fg(Color::Cyan))));

    f.render_widget(list, area);
}

/// Draw footer/status bar
fn draw_footer(f: &mut Frame, _app: &App, area: Rect) {
    let hints = vec![
        ("q", "Quit"),
        ("Tab", "Switch tab"),
        ("/", "Search"),
        ("?", "Help"),
        ("r", "Refresh"),
    ];

    let spans: Vec<Span> = hints
        .iter()
        .flat_map(|(key, action)| {
            vec![
                Span::styled(format!(" {} ", key), Style::default().fg(Color::Black).bg(Color::Cyan)),
                Span::styled(format!(" {} ", action), Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
            ]
        })
        .collect();

    let paragraph = Paragraph::new(Line::from(spans))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(paragraph, area);
}

/// Draw help overlay
fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(60, 60, f.area());

    let help_text = vec![
        Line::from(Span::styled("Keyboard Shortcuts", Style::default().add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("q/Esc", Style::default().fg(Color::Cyan)),
            Span::raw(" - Quit"),
        ]),
        Line::from(vec![
            Span::styled("Tab/→/l", Style::default().fg(Color::Cyan)),
            Span::raw(" - Next tab"),
        ]),
        Line::from(vec![
            Span::styled("Shift+Tab/←/h", Style::default().fg(Color::Cyan)),
            Span::raw(" - Previous tab"),
        ]),
        Line::from(vec![
            Span::styled("↑/k", Style::default().fg(Color::Cyan)),
            Span::raw(" - Move up"),
        ]),
        Line::from(vec![
            Span::styled("↓/j", Style::default().fg(Color::Cyan)),
            Span::raw(" - Move down"),
        ]),
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(" - Search mode"),
        ]),
        Line::from(vec![
            Span::styled("1-5", Style::default().fg(Color::Cyan)),
            Span::raw(" - Jump to tab"),
        ]),
        Line::from(vec![
            Span::styled("r", Style::default().fg(Color::Cyan)),
            Span::raw(" - Refresh data"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press any key to close", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(" Help ", Style::default().fg(Color::Yellow)))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(help_text).block(block);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

/// Create a simple text progress bar
fn create_progress_bar(percent: u16, width: usize) -> String {
    let filled = (percent as usize * width / 100).min(width);
    let empty = width - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// Helper to create centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Get number of CPUs
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
