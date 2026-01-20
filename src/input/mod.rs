use crate::app::actions::Action;
use crate::app::events::{Event, InputEvent};
use crate::app::state::{AppState, Screen, SearchFocus, SettingsFocus};
use crossterm::event::{
    self, Event as CtEvent, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
};
use tokio::sync::mpsc;

pub fn spawn_input_task(tx: mpsc::Sender<Event>, mouse_enabled: bool) {
    tokio::task::spawn_blocking(move || {
        let _ = mouse_enabled;
        loop {
            if event::poll(std::time::Duration::from_millis(250)).unwrap_or(false) {
                match event::read() {
                    Ok(CtEvent::Key(k)) => {
                        if k.kind == KeyEventKind::Press
                            && tx.blocking_send(Event::Input(InputEvent::Key(k))).is_err() {
                                break;
                            }
                    }
                    Ok(CtEvent::Mouse(m)) => {
                        if tx.blocking_send(Event::Input(InputEvent::Mouse(m))).is_err() {
                            break;
                        }
                    }
                    Ok(CtEvent::Resize(_, _)) => {
                        if tx
                            .blocking_send(Event::Input(InputEvent::Resize))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        }
    });
}

pub fn map_input_to_action(state: &AppState, ev: InputEvent) -> Option<Action> {
    match ev {
        InputEvent::Resize => Some(Action::Resize),
        InputEvent::Mouse(m) => match m.kind {
            MouseEventKind::ScrollUp => Some(Action::ListUp),
            MouseEventKind::ScrollDown => Some(Action::ListDown),
            _ => None,
        },
        InputEvent::Key(k) => handle_normal_mode(state, k),
    }
}

fn handle_search_results(k: crossterm::event::KeyEvent) -> Option<Action> {
    match k.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc | KeyCode::Char('/') => Some(Action::SetSearchFocus(SearchFocus::Input)),
        KeyCode::Tab => Some(Action::NextScreen),
        KeyCode::BackTab => Some(Action::PrevScreen),
        KeyCode::Char('i') => Some(Action::SetSearchFocus(SearchFocus::Input)),
        KeyCode::Enter => Some(Action::Activate),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::ListUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::ListDown),
        KeyCode::Char('g') => Some(Action::GoTop),
        KeyCode::Char('G') => Some(Action::GoBottom),
        KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageDown),
        KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageUp),
        KeyCode::Left | KeyCode::Char('h') => Some(Action::SidebarUp),
        KeyCode::Right | KeyCode::Char('l') => Some(Action::SidebarDown),
        KeyCode::Char('r') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Refresh),
        KeyCode::Char(' ') => Some(Action::TogglePause),
        KeyCode::Char('=') | KeyCode::Char('+') => Some(Action::VolumeUp),
        KeyCode::Char('-') | KeyCode::Char('_') => Some(Action::VolumeDown),
        KeyCode::Char(']') => Some(Action::SeekForward),
        KeyCode::Char('[') => Some(Action::SeekBack),
        _ => None,
    }
}

fn handle_normal_mode(state: &AppState, k: crossterm::event::KeyEvent) -> Option<Action> {
    if state.screen == Screen::Search {
        return handle_search_screen_normal(state, k);
    }

    if state.screen == Screen::Settings {
        return handle_settings_screen(state, k);
    }

    if state.screen == Screen::Queue {
        return handle_queue_screen(k);
    }

    if state.screen == Screen::Library {
        return handle_library_screen(state, k);
    }

    match k.code {
        // Quit
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc => Some(Action::Quit),

        // Navigation - vim style
        KeyCode::Up | KeyCode::Char('k') => Some(Action::ListUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::ListDown),
        KeyCode::Char('g') => Some(Action::GoTop),
        KeyCode::Char('G') => Some(Action::GoBottom),
        KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageDown),
        KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageUp),

        // Sidebar navigation
        KeyCode::Left | KeyCode::Char('h') => Some(Action::SidebarUp),
        KeyCode::Right | KeyCode::Char('l') => Some(Action::SidebarDown),

        // Screen switching - Tab cycles through screens
        KeyCode::Tab => Some(Action::NextScreen),
        KeyCode::BackTab => Some(Action::PrevScreen),
        KeyCode::Char('1') => Some(Action::SetScreen(Screen::History)),
        KeyCode::Char('2') => Some(Action::SetScreen(Screen::Search)),
        KeyCode::Char('3') => Some(Action::SetScreen(Screen::Queue)),
        KeyCode::Char('4') => Some(Action::SetScreen(Screen::Library)),
        KeyCode::Char('5') => Some(Action::SetScreen(Screen::Settings)),
        KeyCode::Char('6') => Some(Action::SetScreen(Screen::Help)),

        // Quick queue access
        KeyCode::Char('Q') => Some(Action::SetScreen(Screen::Queue)),

        // Playback navigation
        KeyCode::Char('n') => Some(Action::PlayNext),
        KeyCode::Char('p') => Some(Action::PlayPrev),

        // Playback
        KeyCode::Char(' ') => Some(Action::TogglePause),
        KeyCode::Char('=') | KeyCode::Char('+') => Some(Action::VolumeUp),
        KeyCode::Char('-') | KeyCode::Char('_') => Some(Action::VolumeDown),
        KeyCode::Char(']') => Some(Action::SeekForward),
        KeyCode::Char('[') => Some(Action::SeekBack),

        // Actions
        KeyCode::Enter => Some(Action::Activate),
        KeyCode::Char('r') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Refresh),
        KeyCode::Char('R') => Some(Action::ToggleRepeatMode),
        KeyCode::F(5) => Some(Action::Refresh),
        KeyCode::Char('?') | KeyCode::F(1) => Some(Action::SetScreen(Screen::Help)),

        _ => None,
    }
}

fn handle_settings_screen(state: &AppState, k: crossterm::event::KeyEvent) -> Option<Action> {
    match k.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc => Some(Action::Quit),

        // Tab switches focus within settings
        KeyCode::Tab => Some(Action::SettingsFocusNext),
        KeyCode::BackTab => Some(Action::SettingsFocusPrev),

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Some(Action::ListUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::ListDown),

        // Sidebar navigation (to change screens)
        KeyCode::Left | KeyCode::Char('h') => Some(Action::SidebarUp),
        KeyCode::Right | KeyCode::Char('l') => Some(Action::SidebarDown),

        // Direct screen switching
        KeyCode::Char('1') => Some(Action::SetScreen(Screen::History)),
        KeyCode::Char('2') => Some(Action::SetScreen(Screen::Search)),
        KeyCode::Char('3') => Some(Action::SetScreen(Screen::Queue)),
        KeyCode::Char('4') => Some(Action::SetScreen(Screen::Library)),
        KeyCode::Char('6') => Some(Action::SetScreen(Screen::Help)),

        // Playback
        KeyCode::Char(' ') => Some(Action::TogglePause),
        KeyCode::Char('=') | KeyCode::Char('+') => Some(Action::VolumeUp),
        KeyCode::Char('-') | KeyCode::Char('_') => Some(Action::VolumeDown),

        // Clear cache when on cache section
        KeyCode::Char('c') if state.settings_focus == SettingsFocus::Cache => {
            Some(Action::ClearCache)
        }

        // Apply selection
        KeyCode::Enter => {
            match state.settings_focus {
                SettingsFocus::Authentication => Some(Action::ApplySelectedBrowser),
                SettingsFocus::AudioDevice => Some(Action::ApplySelectedAudioDevice),
                SettingsFocus::Cache => Some(Action::ClearCache),
            }
        }

        KeyCode::Char('r') => Some(Action::Refresh),
        KeyCode::F(5) => Some(Action::Refresh),
        KeyCode::Char('?') | KeyCode::F(1) => Some(Action::SetScreen(Screen::Help)),

        _ => None,
    }
}

fn handle_search_screen_normal(state: &AppState, k: crossterm::event::KeyEvent) -> Option<Action> {
    match state.search_focus {
        SearchFocus::Input => {
            match k.code {
                KeyCode::Esc => Some(Action::Quit),
                KeyCode::Tab => Some(Action::NextScreen),
                KeyCode::BackTab => Some(Action::PrevScreen),
                KeyCode::Enter => Some(Action::StartSearch),
                KeyCode::Backspace => Some(Action::Backspace),
                KeyCode::Down if !state.search_list.items.is_empty() => {
                    Some(Action::SetSearchFocus(SearchFocus::Results))
                }
                KeyCode::Left => Some(Action::SidebarUp),
                KeyCode::Right => Some(Action::SidebarDown),
                KeyCode::F(5) => Some(Action::Refresh),
                KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::ClearInput),
                KeyCode::Char(c) => Some(Action::InputChar(c)),
                _ => None,
            }
        }
        SearchFocus::Results => handle_search_results(k),
    }
}

fn handle_library_screen(state: &AppState, k: crossterm::event::KeyEvent) -> Option<Action> {
    // If playlist view is open, handle navigation within it
    if state.playlist_view.is_open() {
        return handle_playlist_view(k);
    }

    match k.code {
        // Quit
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc => Some(Action::Quit),

        // Tab switching within Library
        KeyCode::Tab => Some(Action::LibraryTabNext),
        KeyCode::BackTab => Some(Action::LibraryTabPrev),

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Some(Action::ListUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::ListDown),
        KeyCode::Char('g') => Some(Action::GoTop),
        KeyCode::Char('G') => Some(Action::GoBottom),
        KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageDown),
        KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageUp),

        // Sidebar navigation
        KeyCode::Left | KeyCode::Char('h') => Some(Action::SidebarUp),
        KeyCode::Right | KeyCode::Char('l') => Some(Action::SidebarDown),

        // Screen switching
        KeyCode::Char('1') => Some(Action::SetScreen(Screen::History)),
        KeyCode::Char('2') => Some(Action::SetScreen(Screen::Search)),
        KeyCode::Char('3') => Some(Action::SetScreen(Screen::Queue)),
        KeyCode::Char('5') => Some(Action::SetScreen(Screen::Settings)),
        KeyCode::Char('6') => Some(Action::SetScreen(Screen::Help)),

        // Playback
        KeyCode::Char(' ') => Some(Action::TogglePause),
        KeyCode::Char('=') | KeyCode::Char('+') => Some(Action::VolumeUp),
        KeyCode::Char('-') | KeyCode::Char('_') => Some(Action::VolumeDown),
        KeyCode::Char(']') => Some(Action::SeekForward),
        KeyCode::Char('[') => Some(Action::SeekBack),
        KeyCode::Char('R') => Some(Action::ToggleRepeatMode),
        KeyCode::Char('n') => Some(Action::PlayNext),
        KeyCode::Char('p') => Some(Action::PlayPrev),
        KeyCode::Char('Q') => Some(Action::SetScreen(Screen::Queue)),

        // Actions
        KeyCode::Enter => Some(Action::Activate),
        KeyCode::Char('r') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Refresh),
        KeyCode::F(5) => Some(Action::Refresh),
        KeyCode::Char('?') | KeyCode::F(1) => Some(Action::SetScreen(Screen::Help)),

        _ => None,
    }
}

fn handle_playlist_view(k: crossterm::event::KeyEvent) -> Option<Action> {
    match k.code {
        // Close playlist view
        KeyCode::Esc | KeyCode::Backspace => Some(Action::ClosePlaylist),

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Some(Action::ListUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::ListDown),
        KeyCode::Char('g') => Some(Action::GoTop),
        KeyCode::Char('G') => Some(Action::GoBottom),
        KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageDown),
        KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageUp),

        // Playback
        KeyCode::Char(' ') => Some(Action::TogglePause),
        KeyCode::Char('=') | KeyCode::Char('+') => Some(Action::VolumeUp),
        KeyCode::Char('-') | KeyCode::Char('_') => Some(Action::VolumeDown),
        KeyCode::Char('n') => Some(Action::PlayNext),
        KeyCode::Char('p') => Some(Action::PlayPrev),

        // Play selected track
        KeyCode::Enter => Some(Action::Activate),

        // Add to queue
        KeyCode::Char('a') => Some(Action::AddSelectedToQueue),
        KeyCode::Char('A') => Some(Action::AddAllToQueue),

        // Quick quit
        KeyCode::Char('q') => Some(Action::Quit),

        _ => None,
    }
}

fn handle_queue_screen(k: crossterm::event::KeyEvent) -> Option<Action> {
    match k.code {
        // Quit
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc => Some(Action::Quit),

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Some(Action::ListUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::ListDown),
        KeyCode::Char('g') => Some(Action::GoTop),
        KeyCode::Char('G') => Some(Action::GoBottom),
        KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageDown),
        KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::PageUp),

        // Sidebar navigation
        KeyCode::Left | KeyCode::Char('h') => Some(Action::SidebarUp),
        KeyCode::Right | KeyCode::Char('l') => Some(Action::SidebarDown),

        // Screen switching
        KeyCode::Tab => Some(Action::NextScreen),
        KeyCode::BackTab => Some(Action::PrevScreen),
        KeyCode::Char('1') => Some(Action::SetScreen(Screen::History)),
        KeyCode::Char('2') => Some(Action::SetScreen(Screen::Search)),
        KeyCode::Char('4') => Some(Action::SetScreen(Screen::Library)),
        KeyCode::Char('5') => Some(Action::SetScreen(Screen::Settings)),
        KeyCode::Char('6') => Some(Action::SetScreen(Screen::Help)),

        // Playback
        KeyCode::Char(' ') => Some(Action::TogglePause),
        KeyCode::Char('=') | KeyCode::Char('+') => Some(Action::VolumeUp),
        KeyCode::Char('-') | KeyCode::Char('_') => Some(Action::VolumeDown),
        KeyCode::Char(']') => Some(Action::SeekForward),
        KeyCode::Char('[') => Some(Action::SeekBack),
        KeyCode::Char('R') => Some(Action::ToggleRepeatMode),

        // Queue-specific actions
        KeyCode::Enter => Some(Action::Activate), // Play selected track
        KeyCode::Char('d') | KeyCode::Delete => Some(Action::QueueRemove(0)), // Will use selected index
        KeyCode::Char('c') => Some(Action::QueueClear),
        KeyCode::Char('s') => Some(Action::QueueShuffle),
        KeyCode::Char('K') => Some(Action::QueueMoveUp),   // Shift+K to move up
        KeyCode::Char('J') => Some(Action::QueueMoveDown), // Shift+J to move down
        KeyCode::Char('n') => Some(Action::PlayNext),
        KeyCode::Char('p') => Some(Action::PlayPrev),

        KeyCode::Char('?') | KeyCode::F(1) => Some(Action::SetScreen(Screen::Help)),

        _ => None,
    }
}
