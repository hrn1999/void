//! Root layout widget - orchestrates main layout structure

use crate::app::state::{AppState, Screen};
use crate::config::Config;
use crate::tui::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::{help, now_playing, settings, sidebar, track_list};

/// Main layout structure:
/// ┌──────────┬─────────────────────────────────────────┐
/// │  Menu    │           Main Content                  │
/// │          │         (History/Search/                │
/// │  History │          Library/etc)                   │
/// │  Search  │                                         │
/// │  Library │                                         │
/// │  Settings│                                         │
/// │  Help    │                                         │
/// ├──────────┼────────────────────┬────────────────────┤
/// │ :q :radio│      Player        │       Lyrics       │
/// └──────────┴────────────────────┴────────────────────┘
pub fn render(frame: &mut Frame, cfg: &Config, state: &mut AppState) {
    let root = frame.area();

    // Main vertical layout: top area | bottom bar
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),       // Top area (sidebar + content)
            Constraint::Length(7),    // Bottom bar (player + lyrics) - compact
        ])
        .split(root);

    // Top area: sidebar | main content
    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),   // Sidebar menu
            Constraint::Min(40),      // Main content area
        ])
        .split(rows[0]);

    // Bottom bar: player | lyrics
    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45), // Player
            Constraint::Percentage(55), // Lyrics
        ])
        .split(rows[1]);

    sidebar::render(frame, state, top_cols[0]);
    render_main_content(frame, cfg, state, top_cols[1]);
    now_playing::render(frame, state, bottom_cols[0]);
    render_lyrics_section(frame, state, bottom_cols[1]);
}

/// Render the lyrics section in the bottom bar (multiple lines)
fn render_lyrics_section(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = get_theme();
    let icons = &theme.icons;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.palette.border))
        .title(format!(" {} Lyrics ", icons.lyrics))
        .title_style(Style::default().fg(theme.palette.accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Add horizontal padding
    let padded = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(1),  // Left padding
            Constraint::Min(1),     // Content
            Constraint::Length(1),  // Right padding
        ])
        .split(inner)[1];

    use ratatui::layout::Alignment;

    let Some(lyrics) = &state.lyrics else {
        let content = Line::from(Span::styled(
            if state.lyrics_loading { "Loading..." } else { "No lyrics available" },
            Style::default().fg(theme.palette.fg_secondary),
        ));
        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, padded);
        return;
    };

    if lyrics.lines.is_empty() {
        let content = Line::from(Span::styled(
            "No lyrics available",
            Style::default().fg(theme.palette.fg_secondary),
        ));
        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, padded);
        return;
    }

    // Find current line based on position
    let position_ms = (state.position_secs * 1000.0) as u64;
    let current_idx = if lyrics.synced {
        lyrics
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.time_ms <= position_ms)
            .map(|(i, _)| i)
            .next_back()
            .unwrap_or(0)
    } else {
        0
    };

    let max_width = padded.width.saturating_sub(4) as usize;

    // Show 3 lines: 1 before, current, 1 after (compact)
    let lines_before = 1;
    let lines_after = 1;

    let start_idx = current_idx.saturating_sub(lines_before);
    let end_idx = (current_idx + lines_after + 1).min(lyrics.lines.len());

    let mut display_lines: Vec<Line> = Vec::new();

    for i in start_idx..end_idx {
        let line_text = lyrics.lines.get(i).map(|l| l.text.as_str()).unwrap_or("");
        let is_current = i == current_idx;

        let style = if is_current {
            Style::default()
                .fg(theme.palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.palette.fg_secondary)
        };

        let prefix = if is_current { "♪ " } else { "  " };

        display_lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(truncate_str(line_text, max_width), style),
        ]));
    }

    // Center vertically
    let available_height = padded.height as usize;
    let content_height = display_lines.len();
    let top_padding = available_height.saturating_sub(content_height) / 2;

    // Add empty lines for vertical centering
    let mut centered_lines: Vec<Line> = vec![Line::default(); top_padding];
    centered_lines.extend(display_lines);

    let paragraph = Paragraph::new(centered_lines);
    frame.render_widget(paragraph, padded);
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    let char_count: usize = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max_len).collect()
    }
}

/// Render the main content area based on current screen
fn render_main_content(frame: &mut Frame, cfg: &Config, state: &mut AppState, area: Rect) {
    let theme = get_theme();
    let icons = &theme.icons;

    // Get title with icon for current screen
    let title = match state.screen {
        Screen::History => format!(" {} History ", icons.history),
        Screen::Search => format!(" {} Search ", icons.search),
        Screen::Library => format!(" {} Library ", icons.library),
        Screen::Settings => format!(" {} Settings ", icons.settings),
        Screen::Help => format!(" {} Keybinds ", icons.help),
    };

    let main = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.palette.border))
        .title(title)
        .title_style(Style::default().fg(theme.palette.accent));
    let inner = main.inner(area);
    frame.render_widget(main, area);

    match state.screen {
        Screen::Search => {
            let sub = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(3)])
                .split(inner);
            track_list::render_search_box(frame, state, sub[0]);
            track_list::render(frame, cfg, state, sub[1]);
        }
        Screen::Settings => {
            settings::render(frame, cfg, state, inner);
        }
        Screen::History | Screen::Library => {
            track_list::render(frame, cfg, state, inner);
        }
        Screen::Help => {
            help::render(frame, state, inner);
        }
    }
}
