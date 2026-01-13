use super::state::{Screen, SearchFocus};

#[derive(Debug, Clone)]
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
}
