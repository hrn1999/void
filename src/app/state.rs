#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    History,
    Search,
    Library,
    Settings,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFocus {
    Input,
    Results,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsFocus {
    #[default]
    Authentication,
    AudioDevice,
    Cache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RepeatMode::Off => "Repeat: Off",
            RepeatMode::One => "Repeat: One",
            RepeatMode::All => "Repeat: All",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub created_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
}

impl Toast {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: ToastKind::Success,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: ToastKind::Error,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > std::time::Duration::from_secs(3)
    }
}

impl Screen {
    pub fn next(self) -> Self {
        match self {
            Screen::History => Screen::Search,
            Screen::Search => Screen::Library,
            Screen::Library => Screen::Settings,
            Screen::Settings => Screen::Help,
            Screen::Help => Screen::History,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Screen::History => Screen::Help,
            Screen::Search => Screen::History,
            Screen::Library => Screen::Search,
            Screen::Settings => Screen::Library,
            Screen::Help => Screen::Settings,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,
}

/// Per-screen list state to keep each screen's selection independent
#[derive(Debug, Clone, Default)]
pub struct ScreenListState {
    pub items: Vec<String>,
    pub tracks: Vec<crate::ytm::models::Track>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub loading: bool,
    pub loaded: bool,
    pub continuation: Option<String>,
    pub has_more: bool,
    pub loading_more: bool,
}

impl ScreenListState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1).min(self.items.len().saturating_sub(1));
        }
    }

    pub fn selected_track(&self) -> Option<&crate::ytm::models::Track> {
        self.tracks.get(self.selected)
    }

    pub fn set_tracks(&mut self, tracks: Vec<crate::ytm::models::Track>) {
        self.items = tracks
            .iter()
            .map(|t| {
                if t.artists.is_empty() {
                    t.title.clone()
                } else {
                    format!("{} - {}", t.title, t.artists.join(", "))
                }
            })
            .collect();
        self.tracks = tracks;
        self.selected = 0;
        self.loaded = true;
        self.loading = false;
    }

    pub fn append_tracks(&mut self, tracks: Vec<crate::ytm::models::Track>) {
        for t in tracks {
            let display = if t.artists.is_empty() {
                t.title.clone()
            } else {
                format!("{} - {}", t.title, t.artists.join(", "))
            };
            self.items.push(display);
            self.tracks.push(t);
        }
        self.loading_more = false;
    }

    pub fn should_load_more(&self, _visible_height: usize) -> bool {
        if self.loading_more || !self.has_more {
            return false;
        }
        let threshold = 5;
        self.selected + threshold >= self.items.len()
    }

    pub fn update_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected - visible_height + 1;
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.tracks.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.continuation = None;
        self.has_more = false;
        self.loading_more = false;
        self.loaded = false;
    }
}

pub struct AppState {
    pub should_quit: bool,
    pub tick: u64,

    pub screen: Screen,
    pub sidebar_selected: usize,

    // Independent screen lists
    pub history_list: ScreenListState,
    pub search_list: ScreenListState,
    pub library_list: ScreenListState,

    // Search
    pub search_query: String,
    pub last_search: Option<String>,
    pub search_focus: SearchFocus,

    // Playback
    pub now_playing: Option<String>,
    pub current_track: Option<crate::ytm::models::Track>,
    pub current_url: Option<String>,
    pub paused: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,

    // Lyrics
    pub lyrics: Option<crate::lyrics::ParsedLyrics>,
    pub lyrics_video_id: Option<String>,
    pub lyrics_loading: bool,

    // Settings: authentication
    pub auth_browsers: Vec<&'static str>,
    pub auth_selected: usize,

    // Settings: audio device selection
    pub audio_devices: Vec<AudioDevice>,
    pub audio_selected: usize,
    pub audio_loaded: bool,
    pub settings_focus: SettingsFocus,

    // Cache info
    pub cache_size_bytes: u64,

    // Repeat mode
    pub repeat_mode: RepeatMode,

    // Toast notification
    pub toast: Option<Toast>,

    // Status message (for debugging/info)
    pub status: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            tick: 0,
            screen: Screen::History,
            sidebar_selected: 0,
            history_list: ScreenListState::new(),
            search_list: ScreenListState::new(),
            library_list: ScreenListState::new(),
            search_query: String::new(),
            last_search: None,
            search_focus: SearchFocus::Input,
            now_playing: None,
            current_track: None,
            current_url: None,
            paused: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume: 80,
            lyrics: None,
            lyrics_video_id: None,
            lyrics_loading: false,
            auth_browsers: vec!["none", "chrome", "firefox", "brave", "edge", "safari", "chromium", "opera", "zen"],
            auth_selected: 0,
            audio_devices: Vec::new(),
            audio_selected: 0,
            audio_loaded: false,
            settings_focus: SettingsFocus::default(),
            cache_size_bytes: 0,
            repeat_mode: RepeatMode::default(),
            toast: None,
            status: String::new(),
        }
    }

    pub fn active_list(&self) -> &ScreenListState {
        match self.screen {
            Screen::History => &self.history_list,
            Screen::Search => &self.search_list,
            Screen::Library => &self.library_list,
            Screen::Settings | Screen::Help => &self.history_list,
        }
    }

    pub fn active_list_mut(&mut self) -> &mut ScreenListState {
        match self.screen {
            Screen::History => &mut self.history_list,
            Screen::Search => &mut self.search_list,
            Screen::Library => &mut self.library_list,
            Screen::Settings | Screen::Help => &mut self.history_list,
        }
    }
}
