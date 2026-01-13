use crate::app::state::{AppState, AudioDevice, SettingsFocus};
use crate::config::Config;
use crate::tui::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, cfg: &Config, state: &AppState, area: Rect) {
    let theme = get_theme();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),  // Auth section (with browser list)
            Constraint::Min(5),      // Audio section
            Constraint::Length(4),   // Lyrics section
            Constraint::Length(6),   // Cache section
            Constraint::Length(3),   // Help section
        ])
        .split(area);

    render_auth_section(frame, cfg, state, &theme, rows[0]);
    render_audio_devices(frame, cfg, state, &theme, rows[1]);
    render_lyrics_section(frame, state, &theme, rows[2]);
    render_cache_section(frame, state, &theme, rows[3]);
    render_help(frame, state, &theme, rows[4]);
}

fn render_auth_section(frame: &mut Frame, cfg: &Config, state: &AppState, theme: &crate::tui::theme::Theme, area: Rect) {
    let icons = &theme.icons;
    let is_focused = state.settings_focus == SettingsFocus::Authentication;
    let border_color = if is_focused { theme.palette.accent } else { theme.palette.border };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(border_color))
        .title(format!(" {} Authentication ", icons.artist))
        .title_style(Style::default().fg(theme.palette.accent));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into status and browser list
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // Status
            Constraint::Min(1),      // Browser list
        ])
        .split(inner);

    // Status line
    let (status_icon, status_text, status_color) = if cfg.ytm.cookies.is_some() {
        (icons.success, "Authenticated (cookie file)".to_string(), theme.palette.playing)
    } else if let Some(browser) = cfg.ytm.cookies_from_browser.as_deref() {
        (icons.success, format!("Browser: {}", browser), theme.palette.playing)
    } else {
        (icons.error, "Not authenticated".to_string(), theme.palette.error)
    };

    let status_line = Line::from(vec![
        Span::styled("Status: ", Style::default().fg(theme.palette.fg_secondary)),
        Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
        Span::styled(status_text, Style::default().fg(status_color)),
    ]);
    frame.render_widget(Paragraph::new(status_line), rows[0]);

    // Browser list
    let current_browser = cfg.ytm.cookies_from_browser.as_deref().unwrap_or("none");
    let items: Vec<ListItem> = state
        .auth_browsers
        .iter()
        .enumerate()
        .map(|(i, browser)| {
            let is_current = *browser == current_browser;
            let is_selected = i == state.auth_selected;

            let style = if is_current {
                Style::default().fg(theme.palette.playing).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.palette.fg_primary)
            };

            let suffix = if is_current { " (current)" } else { "" };
            let prefix = if is_selected && is_focused { "▸ " } else { "  " };

            ListItem::new(Line::from(Span::styled(
                format!("{}{}{}", prefix, browser, suffix),
                style,
            )))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.auth_selected.min(items.len().saturating_sub(1))));

    let highlight_style = if is_focused {
        Style::default()
            .fg(theme.palette.bg_primary)
            .bg(theme.palette.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.palette.fg_secondary)
    };

    let list = List::new(items).highlight_style(highlight_style);

    frame.render_stateful_widget(list, rows[1], &mut list_state);
}

fn render_audio_devices(frame: &mut Frame, cfg: &Config, state: &AppState, theme: &crate::tui::theme::Theme, area: Rect) {
    let icons = &theme.icons;
    let is_focused = state.settings_focus == SettingsFocus::AudioDevice;
    let border_color = if is_focused { theme.palette.accent } else { theme.palette.border };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(border_color))
        .title(format!(" {} Audio Output ", icons.volume))
        .title_style(Style::default().fg(theme.palette.accent));

    if !state.audio_loaded {
        let loading = Paragraph::new(format!("{} Loading audio devices... (press r to retry)", icons.loading))
            .style(Style::default().fg(theme.palette.fg_secondary))
            .block(block);
        frame.render_widget(loading, area);
        return;
    }

    let current = cfg.player.audio_device.as_deref().unwrap_or("auto");

    let items: Vec<ListItem> = state
        .audio_devices
        .iter()
        .map(|d| device_item(d, current, theme))
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.audio_selected.min(items.len().saturating_sub(1))));

    let highlight_style = if is_focused {
        Style::default()
            .fg(theme.palette.bg_primary)
            .bg(theme.palette.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.palette.fg_secondary)
    };

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight_style)
        .highlight_symbol(if is_focused { "▸ " } else { "  " });

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn device_item(d: &AudioDevice, current: &str, theme: &crate::tui::theme::Theme) -> ListItem<'static> {
    let is_current = d.name == current;
    let style = if is_current {
        Style::default().fg(theme.palette.playing).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.palette.fg_primary)
    };

    let suffix = if is_current { " (current)" } else { "" };
    ListItem::new(Line::from(Span::styled(
        format!("{}{}", d.name, suffix),
        style,
    )))
}

fn render_lyrics_section(frame: &mut Frame, state: &AppState, theme: &crate::tui::theme::Theme, area: Rect) {
    let icons = &theme.icons;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.palette.border))
        .title(format!(" {} Lyrics ", icons.lyrics))
        .title_style(Style::default().fg(theme.palette.accent));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (status_icon, status_text, status_color) = if state.lyrics.is_some() {
        (icons.success, "Loaded", theme.palette.playing)
    } else if state.lyrics_loading {
        (icons.loading, "Loading...", theme.palette.fg_secondary)
    } else {
        (icons.bullet, "Not loaded", theme.palette.fg_secondary)
    };

    let synced_info = state
        .lyrics
        .as_ref()
        .map(|l| if l.synced { "Synced" } else { "Unsynced" })
        .unwrap_or("-");

    let content = vec![
        Line::from(vec![
            Span::styled(format!("{} Status: ", icons.bullet), Style::default().fg(theme.palette.fg_secondary)),
            Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
            Span::styled(status_text, Style::default().fg(status_color)),
            Span::styled(format!("  ({})", synced_info), Style::default().fg(theme.palette.fg_secondary)),
        ]),
    ];

    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}

fn render_cache_section(frame: &mut Frame, state: &AppState, theme: &crate::tui::theme::Theme, area: Rect) {
    let icons = &theme.icons;
    let is_focused = state.settings_focus == SettingsFocus::Cache;
    let border_color = if is_focused { theme.palette.accent } else { theme.palette.border };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(border_color))
        .title(format!(" {} Cache & Data ", icons.library))
        .title_style(Style::default().fg(theme.palette.accent));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let cache_size = format_size(state.cache_size_bytes);

    let content = vec![
        Line::from(vec![
            Span::styled(format!("{} Cache size: ", icons.bullet), Style::default().fg(theme.palette.fg_secondary)),
            Span::styled(cache_size, Style::default().fg(theme.palette.fg_primary)),
        ]),
        Line::from(vec![
            Span::styled(format!("{} Press 'c' to clear cache", icons.info), Style::default().fg(theme.palette.fg_secondary)),
        ]),
    ];

    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}

fn render_help(frame: &mut Frame, state: &AppState, theme: &crate::tui::theme::Theme, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.palette.border));

    let focus_hint = match state.settings_focus {
        SettingsFocus::Authentication => "Auth",
        SettingsFocus::AudioDevice => "Audio",
        SettingsFocus::Cache => "Cache",
    };

    let msg = Line::from(vec![
        Span::styled("Tab", Style::default().fg(theme.palette.accent_alt)),
        Span::styled(" switch  ", Style::default().fg(theme.palette.fg_secondary)),
        Span::styled("j/k", Style::default().fg(theme.palette.accent_alt)),
        Span::styled(" navigate  ", Style::default().fg(theme.palette.fg_secondary)),
        Span::styled("Enter", Style::default().fg(theme.palette.accent_alt)),
        Span::styled(" apply  ", Style::default().fg(theme.palette.fg_secondary)),
        Span::styled(format!("[{}]", focus_hint), Style::default().fg(theme.palette.playing)),
    ]);

    frame.render_widget(Paragraph::new(msg).block(block), area);
}

fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
