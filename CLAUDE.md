# Void - YouTube Music TUI Player

## Overview

Void is a terminal-based (TUI) music player for YouTube Music, written in Rust. It provides a vim-style interface for browsing, searching, and playing music from YouTube Music directly in the terminal.

## Tech Stack

- **Language**: Rust (Edition 2024)
- **TUI Framework**: ratatui + crossterm
- **Async Runtime**: tokio
- **Audio Backend**: mpv (via IPC/JSON socket)
- **Database**: SQLite (rusqlite) for caching
- **HTTP Client**: reqwest

## Architecture

```
src/
├── main.rs              # Entry point, CLI parsing
├── lib.rs               # Library exports
├── app/                 # Application core
│   ├── mod.rs           # Main App struct, event loop, action handling
│   ├── actions.rs       # Action enum (all user/system actions)
│   ├── events.rs        # Event types (Input, Player, Network)
│   └── state.rs         # AppState, Screen, UI state structs
├── config/              # Configuration management
│   ├── mod.rs           # Config loading/saving (TOML)
│   └── defaults.rs      # Default configuration values
├── input/               # Input handling
│   └── mod.rs           # Keyboard/mouse mapping to Actions
├── player/              # Audio playback
│   ├── mod.rs           # Module exports
│   └── mpv.rs           # MpvHandle - IPC communication with mpv
├── queue/               # Playback queue
│   └── mod.rs           # Queue struct with shuffle support
├── tui/                 # Terminal UI
│   ├── mod.rs           # Drawing logic, terminal setup
│   ├── theme/           # Colors, icons, borders
│   └── widgets/         # UI components (sidebar, now_playing, etc.)
├── ytm/                 # YouTube Music integration
│   ├── mod.rs           # Module exports
│   ├── api.rs           # YtmClient - API requests
│   ├── auth.rs          # Cookie loading (Netscape format)
│   ├── models.rs        # Track, Playlist, SearchItem structs
│   └── resolve.rs       # Audio URL resolution (yt-dlp)
├── lyrics/              # Lyrics fetching
│   ├── mod.rs           # LrclibClient, fetch logic
│   ├── lrclib.rs        # LRCLIB API integration
│   └── parser.rs        # LRC format parsing
└── storage/             # Data persistence
    └── mod.rs           # SQLite cache (history, stream URLs, lyrics)
```

## Key Concepts

### Screens
The app has 6 screens accessible via sidebar or number keys (1-6):
1. **History** - Recently played tracks (stored locally)
2. **Search** - Search YouTube Music for tracks/playlists
3. **Queue** - Current playback queue with reordering
4. **Library** - User's liked songs and playlists (requires auth)
5. **Settings** - Audio device, browser auth, cache management
6. **Help** - Keybindings reference

### Event-Driven Architecture
- **InputEvent**: Keyboard/mouse input
- **PlayerEvent**: mpv playback events (position, pause, ended)
- **NetworkEvent**: API responses (search results, resolved streams)

All events flow through a tokio mpsc channel and are processed in the main loop.

### Playback Flow
1. User selects a track → `Action::Activate`
2. App resolves audio URL via yt-dlp (or cache)
3. `NetworkEvent::ResolvedStream` received
4. URL loaded into mpv via IPC
5. `PlayerEvent::Position/Duration` updates UI

### Queue System
- `Queue` struct manages playlist with shuffle support
- `playing_from_queue` flag tracks if current track is from queue
- Auto-advances only when playing from queue (not from search/history)
- Supports repeat modes: Off, One, All

### Authentication
YouTube Music auth is handled via browser cookies:
- `cookies_from_browser`: yt-dlp extracts from browser (chrome, firefox, brave, zen, etc.)
- `cookies`: Path to Netscape cookie file

## Keybindings (Vim-style)

| Key | Action |
|-----|--------|
| `j/k` or `↓/↑` | Navigate list |
| `h/l` or `←/→` | Switch screens |
| `Enter` | Play/Select |
| `Space` | Toggle pause |
| `n/p` | Next/Previous in queue |
| `+/-` | Volume up/down |
| `[/]` | Seek back/forward |
| `R` | Cycle repeat mode |
| `q` | Quit |
| `1-6` | Jump to screen |
| `Tab` | Next screen |

Queue-specific:
- `d` - Remove track
- `c` - Clear queue
- `s` - Toggle shuffle
- `K/J` - Move track up/down

## Configuration

Config file: `~/.config/void/config.toml`

```toml
[theme]
name = "nostalgic"

[input]
mouse = true

[player]
volume = 80
# audio_device = "pulse"  # optional

[ytm]
# cookies_from_browser = "firefox"  # or "chrome", "brave", "zen"
# cookies = "/path/to/cookies.txt"  # alternative: Netscape format
```

## Data Storage

SQLite database at `~/.local/share/void/cache.sqlite3`:
- `history` - Played tracks with timestamps
- `stream_cache` - Cached audio URLs (expire after 1h)
- `lyrics_cache` - Cached lyrics from LRCLIB

## Dependencies

External:
- **mpv** - Audio playback (required)
- **yt-dlp** - Audio URL resolution (required)

## Development Notes

### Running
```bash
cargo run
```

### Common Tasks

**Adding a new action:**
1. Add variant to `Action` enum in `src/app/actions.rs`
2. Handle in `handle_action()` or `reduce()` in `src/app/mod.rs`
3. Map key in `src/input/mod.rs`

**Adding a new screen:**
1. Add variant to `Screen` enum in `src/app/state.rs`
2. Update `screen_to_sidebar()` and `sidebar_to_screen()`
3. Add widget in `src/tui/widgets/`
4. Add rendering in `src/tui/mod.rs`

**Modifying mpv communication:**
- Edit `src/player/mpv.rs`
- mpv IPC uses JSON over Unix socket

### Important Patterns

- **State is centralized** in `AppState` struct
- **Actions are synchronous** updates; async work spawns tasks
- **Network results** come back via `NetworkEvent`
- **No direct UI mutations** - all changes go through state
