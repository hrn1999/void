use super::state::{Screen, SearchFocus};
use crate::ytm::models::{Playlist, Track};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Action {
    Quit,
    NextScreen,
    PrevScreen,
    SetScreen(Screen),
    SetSearchFocus(SearchFocus),

    SidebarUp,
    SidebarDown,
    ListUp,
    ListDown,
    GoTop,
    GoBottom,
    PageUp,
    PageDown,
    Activate,
    ToggleRepeatMode,

    InputChar(char),
    Backspace,
    ClearInput,
    StartSearch,
    LoadHistory,
    Refresh,
    ApplySelectedAudioDevice,
    ApplySelectedBrowser,
    SettingsFocusNext,
    SettingsFocusPrev,
    ClearCache,
    TogglePause,
    VolumeUp,
    VolumeDown,
    SeekForward,
    SeekBack,

    Resize,

    // Queue actions
    QueueAdd(Track),
    QueueAddMany(Vec<Track>),
    QueueReplace(Vec<Track>),
    QueueRemove(usize),
    QueueClear,
    QueueShuffle,
    QueueMoveUp,
    QueueMoveDown,
    QueuePlayIndex(usize),
    PlayNext,
    PlayPrev,
    AddSelectedToQueue,    // Add currently selected track to queue
    AddAllToQueue,         // Add all tracks (from playlist view) to queue

    // Library tab actions
    LibraryTabNext,
    LibraryTabPrev,
    LoadPlaylists,
    OpenPlaylist(Playlist),
    ClosePlaylist,

    // Track ended - for auto-advance
    TrackEnded,
}
