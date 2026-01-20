use crate::app::state::AppState;
use crate::tui::theme::get_theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

/// Menu item definition with icon and label
struct MenuItem {
    icon: &'static str,
    label: &'static str,
    is_separator: bool,
}

impl MenuItem {
    const fn item(icon: &'static str, label: &'static str) -> Self {
        Self { icon, label, is_separator: false }
    }

    const fn separator() -> Self {
        Self { icon: "", label: "", is_separator: true }
    }
}

pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = get_theme();
    let icons = &theme.icons;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.palette.border))
        .title(" Menu ")
        .title_style(Style::default().fg(theme.palette.accent));

    // Menu items with icons
    let menu_items = [
        MenuItem::item(icons.history, "History"),
        MenuItem::item(icons.search, "Search"),
        MenuItem::item(icons.queue, "Queue"),
        MenuItem::item(icons.library, "Library"),
        MenuItem::separator(),
        MenuItem::item(icons.settings, "Settings"),
        MenuItem::item(icons.help, "Help"),
    ];

    // Map menu index to actual selection index (skipping separator)
    // Menu indices: 0=History, 1=Search, 2=Queue, 3=Library, 4=separator, 5=Settings, 6=Help
    // Selection indices: 0=History, 1=Search, 2=Queue, 3=Library, 4=Settings, 5=Help
    let selection_to_menu: [usize; 6] = [0, 1, 2, 3, 5, 6];
    let menu_to_selection: [Option<usize>; 7] = [
        Some(0), Some(1), Some(2), Some(3), None, Some(4), Some(5)
    ];

    let items: Vec<ListItem> = menu_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if item.is_separator {
                // Visual separator line
                return ListItem::new(Line::from(""));
            }

            let is_selected = menu_to_selection[i] == Some(state.sidebar_selected);

            let style = if is_selected {
                Style::default()
                    .fg(theme.palette.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.palette.fg_primary)
            };

            let icon_style = if is_selected {
                Style::default().fg(theme.palette.accent)
            } else {
                Style::default().fg(theme.palette.fg_secondary)
            };

            let prefix = if is_selected { icons.selected } else { icons.unselected };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, icon_style),
                Span::raw(" "),
                Span::styled(item.icon, icon_style),
                Span::raw(" "),
                Span::styled(item.label, style),
            ]))
        })
        .collect();

    // Map selection to list position (account for separator)
    let list_idx = selection_to_menu[state.sidebar_selected.min(5)];

    let mut list_state = ListState::default();
    list_state.select(Some(list_idx));

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .fg(theme.palette.bg_primary)
                .bg(theme.palette.accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("");

    frame.render_stateful_widget(list, area, &mut list_state);
}
