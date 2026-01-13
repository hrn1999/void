//! Track list widget - renders lists of tracks with virtual scrolling

use crate::app::state::{AppState, Screen, SearchFocus};
use crate::config::Config;
use crate::tui::theme::{get_theme, LoadingSpinner};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Render the search input box
pub fn render_search_box(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = get_theme();

    let is_focused = state.search_focus == SearchFocus::Input;
    let border_color = if is_focused {
        theme.palette.accent
    } else {
        theme.palette.border
    };

    let title = " Query ";

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(border_color))
        .title(title)
        .title_style(Style::default().fg(theme.palette.accent));

    let prompt = if state.search_list.loading {
        let spinner = LoadingSpinner::frame(state.tick);
        format!("{} {}", state.search_query, spinner)
    } else {
        let cursor = if is_focused { "▏" } else { "" };
        format!("{}{}", state.search_query, cursor)
    };

    let p = Paragraph::new(Line::from(prompt))
        .style(Style::default().fg(theme.palette.fg_primary))
        .block(block);
    frame.render_widget(p, area);
}

/// Render the track list (called within an existing block area)
pub fn render(frame: &mut Frame, _cfg: &Config, state: &AppState, area: Rect) {
    let theme = get_theme();
    let list_state = state.active_list();

    // Show loading state
    if list_state.loading {
        let spinner = LoadingSpinner::frame(state.tick);
        let loading = Paragraph::new(Line::from(format!("{} Loading...", spinner)))
            .style(Style::default().fg(theme.palette.fg_secondary));
        frame.render_widget(loading, area);
        return;
    }

    // Show empty state
    if list_state.items.is_empty() {
        let empty_msg = match state.screen {
            Screen::History => "No history yet. Play some music!",
            Screen::Search => "Search for music above",
            _ => "No items",
        };
        let empty = Paragraph::new(Line::from(empty_msg))
            .style(Style::default().fg(theme.palette.fg_secondary));
        frame.render_widget(empty, area);
        return;
    }

    // Calculate visible height for virtual scroll
    let visible_height = area.height as usize;

    // Highlight search query in results
    let search_query = if state.screen == Screen::Search && !state.search_query.is_empty() {
        Some(state.search_query.to_lowercase())
    } else {
        None
    };

    // Virtual scroll: only render visible items
    let scroll_offset = list_state.scroll_offset;
    let end_idx = (scroll_offset + visible_height).min(list_state.items.len());

    let mut items: Vec<ListItem> = list_state
        .items
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, s)| {
            let is_selected = i == list_state.selected;
            let base_style = if is_selected {
                Style::default()
                    .fg(theme.palette.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.palette.fg_primary)
            };

            // Apply search highlighting if we have a query
            if let Some(ref query) = search_query {
                let spans = highlight_text(s, query, base_style, &theme);
                ListItem::new(Line::from(spans))
            } else {
                ListItem::new(Line::from(Span::styled(s.as_str(), base_style)))
            }
        })
        .collect();

    // Add "loading more" indicator if paginating
    if list_state.loading_more && end_idx >= list_state.items.len() {
        let spinner = LoadingSpinner::frame(state.tick);
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!("  {} Loading more...", spinner),
            Style::default().fg(theme.palette.fg_secondary),
        )])));
    }

    // Add "more available" hint if has_more
    if list_state.has_more && !list_state.loading_more && end_idx >= list_state.items.len() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "  ↓ Scroll for more",
            Style::default().fg(theme.palette.fg_secondary),
        )])));
    }

    // Adjust selection index for virtual scroll offset
    let adjusted_selected = list_state.selected.saturating_sub(scroll_offset);

    let mut ratatui_list_state = ListState::default();
    ratatui_list_state.select(Some(adjusted_selected));

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(theme.palette.bg_primary)
                .bg(theme.palette.accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{f054} "); // nf-fa-chevron_right

    frame.render_stateful_widget(list, area, &mut ratatui_list_state);

    // Show scroll position indicator in top-right corner
    if list_state.items.len() > visible_height {
        let total = list_state.items.len();
        let pos_text = format!("{}/{}", list_state.selected + 1, total);
        let pos_len = pos_text.len() as u16;
        let pos_x = area.x + area.width.saturating_sub(pos_len);
        if pos_x > area.x {
            frame.render_widget(
                Paragraph::new(pos_text).style(Style::default().fg(theme.palette.fg_secondary)),
                Rect::new(pos_x, area.y, pos_len, 1),
            );
        }
    }
}

/// Highlight search query matches in text
fn highlight_text<'a>(
    text: &'a str,
    query: &str,
    base_style: Style,
    theme: &crate::tui::theme::Theme,
) -> Vec<Span<'a>> {
    let highlight_style = base_style.bg(theme.palette.accent_alt);
    let lower_text = text.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    // Split query into words for multi-word highlighting
    for word in query.split_whitespace() {
        if word.is_empty() {
            continue;
        }
        let mut search_start = 0;
        while let Some(start) = lower_text[search_start..].find(word) {
            let abs_start = search_start + start;
            let abs_end = abs_start + word.len();

            // Only add match if it's after our last position
            if abs_start >= last_end {
                // Add non-matching portion before this match
                if abs_start > last_end {
                    spans.push(Span::styled(&text[last_end..abs_start], base_style));
                }
                // Add highlighted match
                spans.push(Span::styled(&text[abs_start..abs_end], highlight_style));
                last_end = abs_end;
            }
            search_start = abs_end;
            if search_start >= lower_text.len() {
                break;
            }
        }
    }

    // Add remaining text after last match
    if last_end < text.len() {
        spans.push(Span::styled(&text[last_end..], base_style));
    }

    if spans.is_empty() {
        vec![Span::styled(text, base_style)]
    } else {
        spans
    }
}
