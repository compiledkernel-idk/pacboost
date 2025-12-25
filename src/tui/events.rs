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

//! TUI event handling.

use crossterm::event::{self, Event, KeyEvent, KeyCode, KeyModifiers, MouseEvent, MouseEventKind};
use std::time::Duration;
use tokio::sync::mpsc;

/// Application event types
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Keyboard event
    Key(KeyEvent),
    /// Mouse event
    Mouse(MouseEvent),
    /// Terminal resize
    Resize(u16, u16),
    /// Timer tick
    Tick,
    /// Download progress update
    DownloadProgress {
        name: String,
        progress: f64,
        speed: u64,
    },
    /// Operation completed
    OperationComplete {
        operation: String,
        success: bool,
        message: String,
    },
    /// Log message
    Log {
        level: LogLevel,
        message: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// Event handler for async event processing
pub struct EventHandler {
    tx: mpsc::Sender<AppEvent>,
    rx: mpsc::Receiver<AppEvent>,
    tick_rate: Duration,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self { tx, rx, tick_rate }
    }

    /// Get the sender for external events
    pub fn sender(&self) -> mpsc::Sender<AppEvent> {
        self.tx.clone()
    }

    /// Start the event loop (call this in a separate task)
    pub async fn run(&self) {
        let tx = self.tx.clone();
        let tick_rate = self.tick_rate;

        loop {
            // Poll for crossterm events
            if event::poll(tick_rate).unwrap_or(false) {
                match event::read() {
                    Ok(Event::Key(key)) => {
                        let _ = tx.send(AppEvent::Key(key)).await;
                    }
                    Ok(Event::Mouse(mouse)) => {
                        let _ = tx.send(AppEvent::Mouse(mouse)).await;
                    }
                    Ok(Event::Resize(w, h)) => {
                        let _ = tx.send(AppEvent::Resize(w, h)).await;
                    }
                    _ => {}
                }
            }

            // Send tick event
            let _ = tx.send(AppEvent::Tick).await;
        }
    }

    /// Receive the next event
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}

/// Key binding configuration
pub struct KeyBindings {
    pub quit: Vec<KeyCode>,
    pub next_tab: Vec<KeyCode>,
    pub prev_tab: Vec<KeyCode>,
    pub up: Vec<KeyCode>,
    pub down: Vec<KeyCode>,
    pub select: Vec<KeyCode>,
    pub search: Vec<KeyCode>,
    pub help: Vec<KeyCode>,
    pub refresh: Vec<KeyCode>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            quit: vec![KeyCode::Char('q'), KeyCode::Esc],
            next_tab: vec![KeyCode::Tab, KeyCode::Right, KeyCode::Char('l')],
            prev_tab: vec![KeyCode::BackTab, KeyCode::Left, KeyCode::Char('h')],
            up: vec![KeyCode::Up, KeyCode::Char('k')],
            down: vec![KeyCode::Down, KeyCode::Char('j')],
            select: vec![KeyCode::Enter, KeyCode::Char(' ')],
            search: vec![KeyCode::Char('/')],
            help: vec![KeyCode::Char('?'), KeyCode::F(1)],
            refresh: vec![KeyCode::Char('r'), KeyCode::F(5)],
        }
    }
}

impl KeyBindings {
    /// Check if a key matches a binding
    pub fn matches(&self, key: KeyCode, binding: &[KeyCode]) -> bool {
        binding.contains(&key)
    }

    /// Get action for a key
    pub fn get_action(&self, key: KeyCode) -> Option<Action> {
        if self.matches(key, &self.quit) {
            Some(Action::Quit)
        } else if self.matches(key, &self.next_tab) {
            Some(Action::NextTab)
        } else if self.matches(key, &self.prev_tab) {
            Some(Action::PrevTab)
        } else if self.matches(key, &self.up) {
            Some(Action::Up)
        } else if self.matches(key, &self.down) {
            Some(Action::Down)
        } else if self.matches(key, &self.select) {
            Some(Action::Select)
        } else if self.matches(key, &self.search) {
            Some(Action::Search)
        } else if self.matches(key, &self.help) {
            Some(Action::Help)
        } else if self.matches(key, &self.refresh) {
            Some(Action::Refresh)
        } else {
            None
        }
    }
}

/// High-level action enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    NextTab,
    PrevTab,
    Up,
    Down,
    Select,
    Search,
    Help,
    Refresh,
}

/// Mouse action handler
pub struct MouseHandler;

impl MouseHandler {
    /// Handle mouse event and return action
    pub fn handle(event: MouseEvent) -> Option<MouseAction> {
        match event.kind {
            MouseEventKind::Down(button) => {
                Some(MouseAction::Click {
                    x: event.column,
                    y: event.row,
                    button: match button {
                        event::MouseButton::Left => MouseButton::Left,
                        event::MouseButton::Right => MouseButton::Right,
                        event::MouseButton::Middle => MouseButton::Middle,
                    },
                })
            }
            MouseEventKind::ScrollUp => Some(MouseAction::ScrollUp),
            MouseEventKind::ScrollDown => Some(MouseAction::ScrollDown),
            MouseEventKind::Moved => {
                Some(MouseAction::Move {
                    x: event.column,
                    y: event.row,
                })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MouseAction {
    Click { x: u16, y: u16, button: MouseButton },
    Move { x: u16, y: u16 },
    ScrollUp,
    ScrollDown,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_bindings_default() {
        let bindings = KeyBindings::default();
        assert!(bindings.quit.contains(&KeyCode::Char('q')));
        assert!(bindings.up.contains(&KeyCode::Char('k')));
    }

    #[test]
    fn test_get_action() {
        let bindings = KeyBindings::default();
        assert_eq!(bindings.get_action(KeyCode::Char('q')), Some(Action::Quit));
        assert_eq!(bindings.get_action(KeyCode::Tab), Some(Action::NextTab));
        assert_eq!(bindings.get_action(KeyCode::Char('x')), None);
    }
}
