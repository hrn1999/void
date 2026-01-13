//! Now Playing widget - compact text-only player for bottom bar

use crate::app::state::{AppState, RepeatMode, ToastKind};
use crate::tui::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = get_theme();
    let icons = &theme.icons;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.palette.border))
        .title(format!(" {} Player ", icons.music))
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

    // Simple vertical layout for text-only player
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Track title
            Constraint::Length(1), // Artist
            Constraint::Length(1), // Spacing
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Time + controls + volume
            Constraint::Min(0),    // Toast (if any)
        ])
        .split(padded);

    let content_width = padded.width.saturating_sub(1) as usize;

    // Track title
    let np = state.now_playing.as_deref().unwrap_or("Not playing");
    let title_line = Line::from(Span::styled(
        truncate_str(np, content_width),
        Style::default()
            .fg(theme.palette.fg_primary)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(Paragraph::new(title_line), rows[0]);

    // Artist
    let artist = state
        .current_track
        .as_ref()
        .map(|t| t.artists.join(", "))
        .unwrap_or_default();
    let artist_line = Line::from(Span::styled(
        truncate_str(&artist, content_width),
        Style::default().fg(theme.palette.fg_secondary),
    ));
    frame.render_widget(Paragraph::new(artist_line), rows[1]);

    // Progress bar (row 3, after spacing)
    let has_playback = state.current_url.is_some() || state.now_playing.is_some();
    let ratio = if has_playback && state.duration_secs > 0.0 {
        (state.position_secs / state.duration_secs).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let bar_width = rows[3].width as usize;
    let progress_bar = render_progress_bar(bar_width, ratio, icons);
    let progress_line = Line::from(Span::styled(
        progress_bar,
        Style::default().fg(theme.palette.accent),
    ));
    frame.render_widget(Paragraph::new(progress_line), rows[3]);

    // Time display + controls + volume (all on one line)
    let pos_min = (state.position_secs / 60.0).floor() as u32;
    let pos_sec = (state.position_secs % 60.0).floor() as u32;
    let dur_min = (state.duration_secs / 60.0).floor() as u32;
    let dur_sec = (state.duration_secs % 60.0).floor() as u32;

    let play_icon = if state.paused { icons.play } else { icons.pause };

    let vol_icon = if state.volume == 0 {
        icons.volume_mute
    } else if state.volume < 50 {
        icons.volume_low
    } else {
        icons.volume_high
    };

    let mut controls_spans = vec![
        Span::styled(
            format!("{:02}:{:02}/{:02}:{:02}", pos_min, pos_sec, dur_min, dur_sec),
            Style::default().fg(theme.palette.fg_secondary),
        ),
        Span::raw(" "),
        Span::styled(icons.prev, Style::default().fg(theme.palette.fg_secondary)),
        Span::raw(" "),
        Span::styled(play_icon, Style::default().fg(theme.palette.playing)),
        Span::raw(" "),
        Span::styled(icons.next, Style::default().fg(theme.palette.fg_secondary)),
        Span::raw("  "),
        Span::styled(vol_icon, Style::default().fg(theme.palette.fg_secondary)),
        Span::raw(" "),
        Span::styled(
            format!("{}%", state.volume),
            Style::default().fg(theme.palette.fg_secondary),
        ),
    ];

    // Add repeat indicator if active
    match state.repeat_mode {
        RepeatMode::Off => {}
        RepeatMode::One => {
            controls_spans.push(Span::raw(" "));
            controls_spans.push(Span::styled(
                icons.repeat_one,
                Style::default().fg(theme.palette.accent_alt),
            ));
        }
        RepeatMode::All => {
            controls_spans.push(Span::raw(" "));
            controls_spans.push(Span::styled(
                icons.repeat,
                Style::default().fg(theme.palette.accent_alt),
            ));
        }
    }

    frame.render_widget(Paragraph::new(Line::from(controls_spans)), rows[4]);

    // Toast messages if any (row 5)
    if let Some(toast) = &state.toast
        && !toast.is_expired()
    {
        let (prefix, color) = match toast.kind {
            ToastKind::Success => (icons.success, theme.palette.playing),
            ToastKind::Error => (icons.error, theme.palette.error),
        };
        let toast_line = Line::from(vec![
            Span::styled(format!("{} ", prefix), Style::default().fg(color)),
            Span::styled(
                truncate_str(&toast.message, content_width.saturating_sub(3)),
                Style::default().fg(color),
            ),
        ]);
        frame.render_widget(Paragraph::new(toast_line), rows[5]);
    }
}

/// Renders a modern progress bar
fn render_progress_bar(width: usize, ratio: f64, icons: &crate::tui::theme::Icons) -> String {
    if width < 3 {
        return String::new();
    }

    let filled = ((width - 1) as f64 * ratio).round() as usize;
    let empty = width.saturating_sub(filled + 1);

    let mut bar = String::with_capacity(width * 3);

    for _ in 0..filled {
        bar.push_str(icons.progress_full);
    }

    bar.push_str(icons.progress_head);

    for _ in 0..empty {
        bar.push_str(icons.progress_empty);
    }

    bar
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
