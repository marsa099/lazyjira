//! Mention picker component for @-mentioning users in the comment composer.
//!
//! Opened by typing `@` while composing a comment. Lists assignable users for the
//! issue's project and lets the user filter by typing and select one to insert a
//! mention. Unlike the assignee picker it has no "Unassigned" option and a single
//! search-as-you-type mode tuned for autocomplete.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::api::types::User;

/// Action resulting from mention picker input.
#[derive(Debug, Clone, PartialEq)]
pub enum MentionAction {
    /// Insert a mention for the user (account_id, display_name).
    Select(String, String),
    /// Close the picker without selecting.
    Cancel,
}

/// Mention picker component.
///
/// Search-as-you-type list of users. Navigation: Up/Down (or Ctrl+P/Ctrl+N),
/// Enter/Tab selects, Esc cancels, other characters filter.
#[derive(Debug)]
pub struct MentionPicker {
    /// Available users to mention.
    users: Vec<User>,
    /// Currently selected index into `filtered_indices`.
    selected: usize,
    /// Whether the picker is visible.
    visible: bool,
    /// Whether users are still loading.
    loading: bool,
    /// Search/filter query.
    search_query: String,
    /// Filtered user indices (into `users`).
    filtered_indices: Vec<usize>,
}

impl MentionPicker {
    /// Create a new mention picker.
    pub fn new() -> Self {
        Self {
            users: Vec::new(),
            selected: 0,
            visible: false,
            loading: false,
            search_query: String::new(),
            filtered_indices: Vec::new(),
        }
    }

    /// Check if the picker is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Check if users are loading.
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Whether users have already been loaded (so we can skip re-fetching).
    pub fn has_users(&self) -> bool {
        !self.users.is_empty()
    }

    /// Show the picker. Pass `loading = true` when a fetch is still in flight.
    pub fn show(&mut self, loading: bool) {
        self.selected = 0;
        self.search_query.clear();
        self.loading = loading;
        self.visible = true;
        self.update_filtered_indices();
    }

    /// Populate the user list (e.g. when an async fetch completes).
    pub fn set_users(&mut self, users: Vec<User>) {
        self.users = users;
        self.loading = false;
        self.update_filtered_indices();
    }

    /// Hide the picker.
    pub fn hide(&mut self) {
        self.visible = false;
        self.loading = false;
        self.search_query.clear();
    }

    /// Number of loaded users.
    pub fn user_count(&self) -> usize {
        self.users.len()
    }

    /// Update filtered indices based on the search query.
    fn update_filtered_indices(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices = (0..self.users.len()).collect();
        } else {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_indices = self
                .users
                .iter()
                .enumerate()
                .filter(|(_, u)| u.display_name.to_lowercase().contains(&query_lower))
                .map(|(i, _)| i)
                .collect();
        }
        if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len().saturating_sub(1);
        }
    }

    /// Handle keyboard input. Returns an action for the parent to handle.
    pub fn handle_input(&mut self, key: KeyEvent) -> Option<MentionAction> {
        if !self.visible {
            return None;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.hide();
                Some(MentionAction::Cancel)
            }
            (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                if !self.filtered_indices.is_empty()
                    && self.selected < self.filtered_indices.len() - 1
                {
                    self.selected += 1;
                }
                None
            }
            (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                None
            }
            (KeyCode::Enter, _) | (KeyCode::Tab, _) => self.select_current(),
            (KeyCode::Backspace, _) => {
                if self.search_query.pop().is_some() {
                    self.update_filtered_indices();
                }
                None
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.search_query.push(c);
                self.update_filtered_indices();
                None
            }
            _ => None,
        }
    }

    /// Select the currently highlighted user, if any.
    fn select_current(&mut self) -> Option<MentionAction> {
        let user_idx = *self.filtered_indices.get(self.selected)?;
        let user = self.users.get(user_idx)?;
        let action = MentionAction::Select(user.account_id.clone(), user.display_name.clone());
        self.hide();
        Some(action)
    }

    /// Render the mention picker.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let dialog_width = 50.min(area.width.saturating_sub(4));
        let dialog_height = 16.min(area.height.saturating_sub(4));
        let dialog_area = centered_rect(dialog_width, dialog_height, area);

        frame.render_widget(Clear, dialog_area);

        let block = Block::default()
            .title(" Mention User ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Search bar
                Constraint::Min(3),    // Users list
                Constraint::Length(2), // Help text
            ])
            .split(inner);

        // Search bar
        let search_line = Line::from(vec![
            Span::styled(
                "@",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&self.search_query, Style::default().fg(Color::White)),
            Span::styled("▏", Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(search_line), chunks[0]);

        // List / loading / empty states
        if self.loading {
            let loading_text = Paragraph::new("Loading users...")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            frame.render_widget(loading_text, chunks[1]);
        } else if self.filtered_indices.is_empty() {
            let empty = if self.users.is_empty() {
                "No users found"
            } else {
                "No matches"
            };
            let empty_text = Paragraph::new(empty)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);
            frame.render_widget(empty_text, chunks[1]);
        } else {
            let items: Vec<ListItem> = self
                .filtered_indices
                .iter()
                .filter_map(|&idx| self.users.get(idx))
                .map(|user| ListItem::new(format!("  {}", user.display_name)))
                .collect();

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            let mut state = ListState::default();
            state.select(Some(self.selected));
            frame.render_stateful_widget(list, chunks[1], &mut state);
        }

        // Help text
        let help_text = Line::from(vec![
            Span::styled("type", Style::default().fg(Color::Cyan)),
            Span::raw(": filter  "),
            Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
            Span::raw(": navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(": insert  "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(": cancel"),
        ]);
        frame.render_widget(
            Paragraph::new(help_text).alignment(Alignment::Center),
            chunks[2],
        );
    }
}

impl Default for MentionPicker {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a centered rectangle.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(account_id: &str, display_name: &str) -> User {
        User {
            account_id: account_id.to_string(),
            display_name: display_name.to_string(),
            email_address: None,
            active: true,
            avatar_urls: None,
        }
    }

    #[test]
    fn test_new_hidden() {
        let picker = MentionPicker::new();
        assert!(!picker.is_visible());
        assert_eq!(picker.user_count(), 0);
    }

    #[test]
    fn test_show_then_set_users() {
        let mut picker = MentionPicker::new();
        picker.show(true);
        assert!(picker.is_visible());
        assert!(picker.is_loading());

        picker.set_users(vec![user("a", "Alice"), user("b", "Bob")]);
        assert!(!picker.is_loading());
        assert_eq!(picker.filtered_indices.len(), 2);
    }

    #[test]
    fn test_filter_by_typing() {
        let mut picker = MentionPicker::new();
        picker.show(false);
        picker.set_users(vec![
            user("a", "Alice Smith"),
            user("b", "Bob Jones"),
            user("c", "Alice Jones"),
        ]);

        for c in ['a', 'l', 'i'] {
            picker.handle_input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(picker.filtered_indices.len(), 2);

        picker.handle_input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        // "al" still matches both Alices
        assert_eq!(picker.filtered_indices.len(), 2);
    }

    #[test]
    fn test_select_with_enter() {
        let mut picker = MentionPicker::new();
        picker.show(false);
        picker.set_users(vec![user("a", "Alice"), user("b", "Bob")]);

        picker.handle_input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let action = picker.handle_input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            action,
            Some(MentionAction::Select("b".to_string(), "Bob".to_string()))
        );
        assert!(!picker.is_visible());
    }

    #[test]
    fn test_enter_with_no_matches_stays_open() {
        let mut picker = MentionPicker::new();
        picker.show(false);
        picker.set_users(vec![user("a", "Alice")]);

        for c in ['z', 'z', 'z'] {
            picker.handle_input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        let action = picker.handle_input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(action.is_none());
        assert!(picker.is_visible());
    }

    #[test]
    fn test_esc_cancels() {
        let mut picker = MentionPicker::new();
        picker.show(false);
        picker.set_users(vec![user("a", "Alice")]);

        let action = picker.handle_input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(action, Some(MentionAction::Cancel));
        assert!(!picker.is_visible());
    }

    #[test]
    fn test_query_survives_loading_until_users_arrive() {
        let mut picker = MentionPicker::new();
        picker.show(true);
        // Type while still loading.
        for c in ['b', 'o'] {
            picker.handle_input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        picker.set_users(vec![user("a", "Alice"), user("b", "Bob")]);
        // Filter applies once users load.
        assert_eq!(picker.filtered_indices.len(), 1);
    }
}
