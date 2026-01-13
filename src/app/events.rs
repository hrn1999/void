#[derive(Debug, Clone)]
pub enum Event {
    Input(InputEvent),
    Player(PlayerEvent),
    Network(NetworkEvent),
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize,
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Started,
    Paused,
    Position { seconds: f64 },
    Duration { seconds: f64 },
    Ended,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    Error(String),
    SearchResults { query: String, tracks: Vec<crate::ytm::models::Track>, continuation: Option<String> },
    SearchMoreResults { tracks: Vec<crate::ytm::models::Track>, continuation: Option<String> },
    HistoryResults { tracks: Vec<crate::ytm::models::Track> },
    HistoryAdded { track: crate::ytm::models::Track },
    LibraryResults { tracks: Vec<crate::ytm::models::Track> },
    ResolvedStream { track: crate::ytm::models::Track, url: String },
    AudioDevices { devices: Vec<crate::app::state::AudioDevice> },
    LyricsLoaded { video_id: String, lyrics: crate::lyrics::ParsedLyrics },
    LyricsNotFound { video_id: String },
}

