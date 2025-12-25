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

//! Custom TUI widgets.

use ratatui::{
    prelude::*,
    widgets::*,
};

/// Sparkline-style mini chart
pub struct SparklineChart<'a> {
    data: &'a [f64],
    max: f64,
    style: Style,
}

impl<'a> SparklineChart<'a> {
    pub fn new(data: &'a [f64]) -> Self {
        let max = data.iter().cloned().fold(0.0f64, f64::max).max(1.0);
        Self {
            data,
            max,
            style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for SparklineChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.data.is_empty() {
            return;
        }

        let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        
        let start = if self.data.len() > area.width as usize {
            self.data.len() - area.width as usize
        } else {
            0
        };

        for (i, &value) in self.data[start..].iter().enumerate() {
            if i >= area.width as usize {
                break;
            }

            let normalized = (value / self.max).min(1.0);
            let bar_index = ((normalized * 7.0) as usize).min(7);
            let char = bar_chars[bar_index];

            buf.get_mut(area.x + i as u16, area.y + area.height - 1)
                .set_char(char)
                .set_style(self.style);
        }
    }
}

/// Circular progress indicator
pub struct CircularProgress {
    percent: u16,
    label: String,
    style: Style,
}

impl CircularProgress {
    pub fn new(percent: u16) -> Self {
        Self {
            percent: percent.min(100),
            label: format!("{}%", percent),
            style: Style::default().fg(Color::Cyan),
        }
    }

    pub fn label(mut self, label: String) -> Self {
        self.label = label;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for CircularProgress {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 5 || area.height < 3 {
            return;
        }

        // Simple circle representation using unicode
        let progress_chars = ['○', '◔', '◑', '◕', '●'];
        let index = ((self.percent as usize * 4) / 100).min(4);
        let char = progress_chars[index];

        let center_x = area.x + area.width / 2;
        let center_y = area.y + area.height / 2;

        buf.get_mut(center_x, center_y)
            .set_char(char)
            .set_style(self.style);

        // Draw label below
        if area.height > 1 && !self.label.is_empty() {
            let label_start = center_x.saturating_sub(self.label.len() as u16 / 2);
            buf.set_string(label_start, center_y + 1, &self.label, self.style);
        }
    }
}

/// Status indicator widget
pub struct StatusIndicator {
    status: Status,
    label: String,
}

#[derive(Clone, Copy)]
pub enum Status {
    Ok,
    Warning,
    Error,
    Unknown,
}

impl StatusIndicator {
    pub fn new(status: Status, label: &str) -> Self {
        Self {
            status,
            label: label.to_string(),
        }
    }
}

impl Widget for StatusIndicator {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 {
            return;
        }

        let (icon, color) = match self.status {
            Status::Ok => ("●", Color::Green),
            Status::Warning => ("●", Color::Yellow),
            Status::Error => ("●", Color::Red),
            Status::Unknown => ("○", Color::DarkGray),
        };

        let style = Style::default().fg(color);
        buf.set_string(area.x, area.y, icon, style);
        
        if area.width > 2 {
            buf.set_string(area.x + 2, area.y, &self.label, Style::default());
        }
    }
}

/// Info card widget
pub struct InfoCard<'a> {
    title: &'a str,
    value: &'a str,
    footer: Option<&'a str>,
    border_style: Style,
}

impl<'a> InfoCard<'a> {
    pub fn new(title: &'a str, value: &'a str) -> Self {
        Self {
            title,
            value,
            footer: None,
            border_style: Style::default().fg(Color::Blue),
        }
    }

    pub fn footer(mut self, footer: &'a str) -> Self {
        self.footer = Some(footer);
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }
}

impl Widget for InfoCard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style)
            .title(Span::styled(
                format!(" {} ", self.title),
                Style::default().fg(Color::Cyan),
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        // Render value centered
        let value_x = inner.x + (inner.width.saturating_sub(self.value.len() as u16)) / 2;
        let value_y = inner.y + inner.height / 2;
        buf.set_string(
            value_x,
            value_y,
            self.value,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        );

        // Render footer if present
        if let Some(footer) = self.footer {
            if inner.height > 2 {
                let footer_x = inner.x + (inner.width.saturating_sub(footer.len() as u16)) / 2;
                buf.set_string(
                    footer_x,
                    inner.y + inner.height - 1,
                    footer,
                    Style::default().fg(Color::DarkGray),
                );
            }
        }
    }
}

/// Package list item widget
pub struct PackageListItem {
    name: String,
    version: String,
    description: String,
    installed: bool,
    selected: bool,
}

impl PackageListItem {
    pub fn new(name: &str, version: &str, description: &str, installed: bool) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            installed,
            selected: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Widget for PackageListItem {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg_color = if self.selected { Color::DarkGray } else { Color::Reset };
        
        // Clear area with background
        for y in area.y..area.y + area.height.min(2) {
            for x in area.x..area.x + area.width {
                buf.get_mut(x, y).set_bg(bg_color);
            }
        }

        // Draw installed indicator
        let icon = if self.installed { "✓" } else { " " };
        let icon_style = Style::default().fg(if self.installed { Color::Green } else { Color::DarkGray });
        buf.set_string(area.x, area.y, icon, icon_style);

        // Draw name
        buf.set_string(
            area.x + 2,
            area.y,
            &self.name,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        );

        // Draw version
        let version_x = area.x + 2 + self.name.len() as u16 + 1;
        buf.set_string(
            version_x,
            area.y,
            &self.version,
            Style::default().fg(Color::Yellow),
        );

        // Draw description on second line if space
        if area.height > 1 && area.width > 4 {
            let desc = if self.description.len() > (area.width - 4) as usize {
                format!("{}...", &self.description[..(area.width - 7) as usize])
            } else {
                self.description.clone()
            };
            buf.set_string(
                area.x + 2,
                area.y + 1,
                &desc,
                Style::default().fg(Color::DarkGray),
            );
        }
    }
}

/// Download progress widget
pub struct DownloadProgress {
    name: String,
    progress: f64,
    speed: String,
    eta: String,
}

impl DownloadProgress {
    pub fn new(name: &str, progress: f64, speed: &str, eta: &str) -> Self {
        Self {
            name: name.to_string(),
            progress: progress.clamp(0.0, 100.0),
            speed: speed.to_string(),
            eta: eta.to_string(),
        }
    }
}

impl Widget for DownloadProgress {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 {
            return;
        }

        // First line: name and percentage
        buf.set_string(
            area.x,
            area.y,
            &self.name,
            Style::default().fg(Color::White),
        );
        
        let percent_str = format!("{:.1}%", self.progress);
        let percent_x = area.x + area.width - percent_str.len() as u16;
        buf.set_string(
            percent_x,
            area.y,
            &percent_str,
            Style::default().fg(Color::Cyan),
        );

        // Second line: progress bar and stats
        let bar_width = (area.width - 15).max(10) as usize;
        let filled = ((self.progress / 100.0) * bar_width as f64) as usize;
        let empty = bar_width - filled;
        
        let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));
        buf.set_string(
            area.x,
            area.y + 1,
            &bar,
            Style::default().fg(Color::Cyan),
        );

        // Speed and ETA
        let stats = format!("{} | {}", self.speed, self.eta);
        let stats_x = area.x + area.width - stats.len() as u16;
        buf.set_string(
            stats_x,
            area.y + 1,
            &stats,
            Style::default().fg(Color::DarkGray),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparkline_chart() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let chart = SparklineChart::new(&data);
        // Just ensure it creates without panic
        drop(chart);
    }

    #[test]
    fn test_circular_progress() {
        let progress = CircularProgress::new(50);
        assert_eq!(progress.percent, 50);
        
        let progress = CircularProgress::new(150);
        assert_eq!(progress.percent, 100);
    }

    #[test]
    fn test_status_indicator() {
        let indicator = StatusIndicator::new(Status::Ok, "Running");
        drop(indicator);
    }
}
