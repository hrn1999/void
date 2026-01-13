//! Help screen showing keybindings and commands

use crate::app::state::AppState;
use crate::tui::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// Render the help screen
pub fn render(frame: &mut Frame, _state: &AppState, area: Rect) {
    let theme = get_theme();
    let icons = &theme.icons;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.palette.border))
        .title(format!(" {} Keybinds ", icons.help))
        .title_style(Style::default().fg(theme.palette.accent));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into columns
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    // Left column - Navigation & Playback
    let left_content = vec![
        section_header("Navigation", &theme),
        keybind("j / Down", "Move down", &theme),
        keybind("k / Up", "Move up", &theme),
        keybind("g", "Go to top", &theme),
        keybind("G", "Go to bottom", &theme),
        keybind("Ctrl+d", "Page down", &theme),
        keybind("Ctrl+u", "Page up", &theme),
        keybind("h / Left", "Previous screen", &theme),
        keybind("l / Right", "Next screen", &theme),
        keybind("Tab", "Next screen / Focus search", &theme),
        keybind("1-5", "Go to screen", &theme),
        Line::default(),
        section_header("Playback", &theme),
        keybind("Space", "Toggle pause", &theme),
        keybind("Enter", "Play selected track", &theme),
        keybind("+ / =", "Volume up", &theme),
        keybind("- / _", "Volume down", &theme),
        keybind("]", "Seek forward 10s", &theme),
        keybind("[", "Seek back 10s", &theme),
        keybind("R", "Toggle repeat mode", &theme),
    ];

    let left_para = Paragraph::new(left_content).wrap(Wrap { trim: false });
    frame.render_widget(left_para, cols[0]);

    // Right column - Search & General
    let right_content = vec![
        section_header("Search", &theme),
        keybind("/", "Return to search bar", &theme),
        keybind("i", "Return to search bar", &theme),
        keybind("Tab", "Go to search", &theme),
        keybind("Enter", "Execute search", &theme),
        keybind("Ctrl+u", "Clear input", &theme),
        keybind("Down", "Focus results", &theme),
        Line::default(),
        section_header("General", &theme),
        keybind("q", "Quit application", &theme),
        keybind("Ctrl+r", "Refresh current screen", &theme),
        keybind("Esc", "Quit", &theme),
    ];

    let right_para = Paragraph::new(right_content).wrap(Wrap { trim: false });
    frame.render_widget(right_para, cols[1]);
}

fn section_header(title: &str, theme: &crate::tui::theme::Theme) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("━━ {} ━━", title),
        Style::default()
            .fg(theme.palette.accent)
            .add_modifier(Modifier::BOLD),
    )])
}

fn keybind(key: &str, desc: &str, theme: &crate::tui::theme::Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{:12}", key),
            Style::default()
                .fg(theme.palette.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc.to_string(), Style::default().fg(theme.palette.fg_primary)),
    ])
}
