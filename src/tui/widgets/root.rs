//! Root layout widget - orchestrates main layout structure

use crate::app::state::{AppState, LibraryTab, Screen};
use crate::config::Config;
use crate::tui::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::{help, now_playing, queue, settings, sidebar, track_list};

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
        Screen::Queue => format!(" {} Queue ", icons.queue),
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
        Screen::Queue => {
            queue::render(frame, state, inner);
        }
        Screen::Settings => {
            settings::render(frame, cfg, state, inner);
        }
        Screen::History => {
            track_list::render(frame, cfg, state, inner);
        }
        Screen::Library => {
            render_library_with_tabs(frame, cfg, state, inner);
        }
        Screen::Help => {
            help::render(frame, state, inner);
        }
    }
}

/// Render the library screen with tabs for Liked Songs, Playlists, Albums
fn render_library_with_tabs(frame: &mut Frame, cfg: &Config, state: &mut AppState, area: Rect) {
    let theme = get_theme();

    // Split into tabs bar and content
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(3)])
        .split(area);

    // Render tabs
    let tabs = [
        ("Liked Songs", LibraryTab::LikedSongs),
        ("Playlists", LibraryTab::Playlists),
        ("Albums", LibraryTab::Albums),
    ];

    let tab_spans: Vec<Span> = tabs
        .iter()
        .enumerate()
        .flat_map(|(i, (label, tab))| {
            let is_selected = state.library_tab == *tab;
            let style = if is_selected {
                Style::default()
                    .fg(theme.palette.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.palette.fg_secondary)
            };

            let bracket_style = if is_selected {
                Style::default().fg(theme.palette.accent)
            } else {
                Style::default().fg(theme.palette.fg_secondary)
            };

            let mut spans = vec![
                Span::styled("[", bracket_style),
                Span::styled(*label, style),
                Span::styled("]", bracket_style),
            ];

            if i < tabs.len() - 1 {
                spans.push(Span::raw("  "));
            }
            spans
        })
        .collect();

    let tabs_line = Line::from(tab_spans);
    let tabs_paragraph = Paragraph::new(tabs_line);
    frame.render_widget(tabs_paragraph, layout[0]);

    // Render content based on selected tab
    match state.library_tab {
        LibraryTab::LikedSongs => {
            track_list::render(frame, cfg, state, layout[1]);
        }
        LibraryTab::Playlists => {
            render_playlists_list(frame, state, layout[1]);
        }
        LibraryTab::Albums => {
            render_albums_placeholder(frame, layout[1]);
        }
    }
}

/// Render the playlists list in the Library
fn render_playlists_list(frame: &mut Frame, state: &AppState, area: Rect) {
    // If playlist view is open, render that instead
    if state.playlist_view.is_open() {
        render_playlist_tracks_view(frame, state, area);
        return;
    }

    let theme = get_theme();
    let icons = &theme.icons;

    let playlist_state = &state.playlist_list;

    if playlist_state.loading {
        let spinner = crate::tui::theme::LoadingSpinner::frame(state.tick);
        let loading = Paragraph::new(Line::from(format!("{} Loading playlists...", spinner)))
            .style(Style::default().fg(theme.palette.fg_secondary));
        frame.render_widget(loading, area);
        return;
    }

    if playlist_state.playlists.is_empty() {
        let msg = if playlist_state.loaded {
            "No playlists found. Create some on YouTube Music!"
        } else {
            "Press Tab to load playlists (requires authentication)"
        };
        let empty = Paragraph::new(Line::from(msg))
            .style(Style::default().fg(theme.palette.fg_secondary));
        frame.render_widget(empty, area);
        return;
    }

    let visible_height = area.height as usize;
    let scroll_offset = playlist_state.scroll_offset;

    let items: Vec<ListItem> = playlist_state
        .playlists
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, playlist)| {
            let is_selected = i == playlist_state.selected;

            let style = if is_selected {
                Style::default()
                    .fg(theme.palette.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.palette.fg_primary)
            };

            let track_count = playlist
                .track_count
                .map(|c| format!(" ({} tracks)", c))
                .unwrap_or_default();

            let display = format!("{} {}{}", icons.playlist, playlist.title, track_count);

            ListItem::new(Line::from(Span::styled(display, style)))
        })
        .collect();

    let adjusted_selected = playlist_state.selected.saturating_sub(scroll_offset);
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(adjusted_selected));

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(theme.palette.bg_primary)
                .bg(theme.palette.accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{f054} ");

    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Placeholder for albums list
fn render_albums_placeholder(frame: &mut Frame, area: Rect) {
    let theme = get_theme();
    let msg = "Albums tab coming soon...";
    let placeholder = Paragraph::new(Line::from(msg))
        .style(Style::default().fg(theme.palette.fg_secondary));
    frame.render_widget(placeholder, area);
}

/// Render the tracks within an opened playlist
fn render_playlist_tracks_view(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = get_theme();
    let icons = &theme.icons;
    let view = &state.playlist_view;

    // Header with back hint and playlist name
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(3)])
        .split(area);

    let playlist_name = view
        .playlist
        .as_ref()
        .map(|p| p.title.as_str())
        .unwrap_or("Unknown Playlist");

    let track_count = view.tracks.len();

    let header = Line::from(vec![
        Span::styled("← ", Style::default().fg(theme.palette.fg_secondary)),
        Span::styled("Esc/Backspace", Style::default().fg(theme.palette.accent)),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("\"{}\" ({} tracks)", playlist_name, track_count),
            Style::default()
                .fg(theme.palette.fg_primary)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), layout[0]);

    // Loading state
    if view.loading {
        let spinner = crate::tui::theme::LoadingSpinner::frame(state.tick);
        let loading = Paragraph::new(Line::from(format!("{} Loading tracks...", spinner)))
            .style(Style::default().fg(theme.palette.fg_secondary));
        frame.render_widget(loading, layout[1]);
        return;
    }

    // Empty state
    if view.tracks.is_empty() {
        let empty = Paragraph::new(Line::from("This playlist is empty"))
            .style(Style::default().fg(theme.palette.fg_secondary));
        frame.render_widget(empty, layout[1]);
        return;
    }

    // Track list
    let visible_height = layout[1].height as usize;
    let scroll_offset = view.scroll_offset;

    let items: Vec<ListItem> = view
        .tracks
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, track)| {
            let is_selected = i == view.selected;

            let style = if is_selected {
                Style::default()
                    .fg(theme.palette.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.palette.fg_primary)
            };

            let artists = if track.artists.is_empty() {
                String::new()
            } else {
                format!(" - {}", track.artists.join(", "))
            };

            let display = format!("{} {}{}", icons.music, track.title, artists);

            ListItem::new(Line::from(Span::styled(display, style)))
        })
        .collect();

    let adjusted_selected = view.selected.saturating_sub(scroll_offset);
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(adjusted_selected));

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(theme.palette.bg_primary)
                .bg(theme.palette.accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{f054} ");

    frame.render_stateful_widget(list, layout[1], &mut list_state);

    // Scroll position indicator
    if view.tracks.len() > visible_height {
        let pos_text = format!("{}/{}", view.selected + 1, view.tracks.len());
        let pos_len = pos_text.len() as u16;
        let pos_x = layout[1].x + layout[1].width.saturating_sub(pos_len);
        if pos_x > layout[1].x {
            frame.render_widget(
                Paragraph::new(pos_text).style(Style::default().fg(theme.palette.fg_secondary)),
                Rect::new(pos_x, layout[1].y, pos_len, 1),
            );
        }
    }
}
