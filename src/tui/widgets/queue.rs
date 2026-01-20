//! Queue screen widget - displays the playback queue

use crate::app::state::AppState;
use crate::tui::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = get_theme();
    let icons = &theme.icons;

    // Add padding
    let padded = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area)[1];

    let queue = &state.queue;

    if queue.is_empty() {
        let empty_msg = Line::from(vec![
            Span::styled(
                "Queue is empty. ",
                Style::default().fg(theme.palette.fg_secondary),
            ),
            Span::styled(
                "Select tracks to add them here.",
                Style::default().fg(theme.palette.fg_secondary),
            ),
        ]);
        let paragraph = Paragraph::new(empty_msg);
        frame.render_widget(paragraph, padded);
        return;
    }

    // Header line with shuffle status
    let header = Line::from(vec![
        Span::styled(
            format!("{} tracks", queue.len()),
            Style::default().fg(theme.palette.fg_secondary),
        ),
        Span::raw("  "),
        if queue.is_shuffle_enabled() {
            Span::styled(
                format!("{} Shuffle ON", icons.shuffle),
                Style::default().fg(theme.palette.accent),
            )
        } else {
            Span::styled(
                format!("{} Shuffle OFF", icons.shuffle),
                Style::default().fg(theme.palette.fg_secondary),
            )
        },
    ]);

    // Track list
    let tracks = queue.tracks();
    let current_idx = queue.current_index();
    let selected_idx = state.queue_list.selected;
    let scroll_offset = state.queue_list.scroll_offset;

    let visible_height = padded.height.saturating_sub(2) as usize; // -2 for header and hints
    let max_width = padded.width.saturating_sub(6) as usize; // -6 for index and icons

    let mut lines: Vec<Line> = vec![header, Line::default()];

    for (i, track) in tracks.iter().enumerate().skip(scroll_offset).take(visible_height) {
        let is_current = current_idx == Some(i);
        let is_selected = i == selected_idx;

        let prefix = if is_current {
            format!("{} ", icons.play)
        } else {
            "  ".to_string()
        };

        let index_str = format!("{:>3}. ", i + 1);

        let display = if track.artists.is_empty() {
            track.title.clone()
        } else {
            format!("{} - {}", track.title, track.artists.join(", "))
        };
        let display = truncate_str(&display, max_width);

        let style = if is_selected {
            Style::default()
                .fg(theme.palette.fg_primary)
                .bg(theme.palette.bg_highlight)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .fg(theme.palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.palette.fg_primary)
        };

        let prefix_style = if is_current {
            Style::default().fg(theme.palette.accent)
        } else {
            Style::default().fg(theme.palette.fg_secondary)
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, prefix_style),
            Span::styled(index_str, Style::default().fg(theme.palette.fg_secondary)),
            Span::styled(display, style),
        ]));
    }

    // Hints at the bottom
    if lines.len() < (padded.height as usize) {
        let remaining = (padded.height as usize) - lines.len();
        for _ in 0..remaining.saturating_sub(1) {
            lines.push(Line::default());
        }
        lines.push(Line::from(vec![
            Span::styled(
                "Enter: Play  d: Remove  c: Clear  s: Shuffle  K/J: Move",
                Style::default().fg(theme.palette.fg_secondary),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines);
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
