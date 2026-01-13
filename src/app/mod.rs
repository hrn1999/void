pub mod actions;
pub mod events;
pub mod state;

use crate::config::Config;
use crate::input;
use crate::storage::Storage;
use crate::tui::{self, TuiTerminal};
use crate::player::mpv::MpvHandle;
use crate::ytm::{self, api::YtmClient};
use actions::Action;
use events::Event;
use state::{AppState, RepeatMode, Screen, SearchFocus, SettingsFocus, Toast};
use tokio::sync::mpsc;

pub struct App {
    cfg: Config,
    config_path: std::path::PathBuf,
    state: AppState,
    ytm: YtmClient,
    lrclib: crate::lyrics::LrclibClient,
    mpv: Option<MpvHandle>,
}

impl App {
    pub fn new(cfg: Config, config_path: std::path::PathBuf) -> anyhow::Result<Self> {
        let auth = match cfg.ytm.cookies.as_deref() {
            Some(p) if p.exists() => Some(ytm::auth::load_netscape_cookies(p)?),
            _ => None,
        };
        let ytm = YtmClient::new(auth)?;
        let lrclib = crate::lyrics::LrclibClient::new();
        let _ = Storage::open(&cfg.paths.data_dir.join("cache.sqlite3"))?;

        // Create state with config values
        let mut state = AppState::new();
        state.volume = cfg.player.volume;

        // Restore last screen if available
        if let Some(screen_name) = &cfg.ui.last_screen {
            state.screen = match screen_name.as_str() {
                "history" => Screen::History,
                "search" => Screen::Search,
                "library" => Screen::Library,
                "settings" => Screen::Settings,
                "help" => Screen::Help,
                _ => Screen::History,
            };
            state.sidebar_selected = screen_to_sidebar(state.screen);
        }

        Ok(Self {
            cfg,
            config_path,
            state,
            ytm,
            lrclib,
            mpv: None,
        })
    }

    pub async fn run(&mut self, terminal: &mut TuiTerminal) -> anyhow::Result<()> {
        let (tx, mut rx) = mpsc::channel::<Event>(256);

        input::spawn_input_task(tx.clone(), self.cfg.input.mouse);
        // Performance: don't drive the UI with a constant ticker.
        // We re-render on input, network, and player events.

        // Phase 2: start mpv backend (best-effort).
        let mpv_log = self.cfg.paths.data_dir.join("mpv.log");
            match MpvHandle::spawn(
            tx.clone(),
            self.cfg.player.audio_device.as_deref(),
            Some(&mpv_log),
        )
        .await
        {
            Ok(h) => {
                self.mpv = Some(h);
            }
            Err(e) => {
                self.state.toast = Some(Toast::error(format!("mpv disabled: {e:#}")));
                self.mpv = None;
            }
        }

        // First draw
        tui::draw(terminal, &self.cfg, &mut self.state)?;

        // Auto-load History on startup
        self.handle_action(Action::LoadHistory, &tx).await;

        while let Some(ev) = rx.recv().await {
            match ev {
                Event::Input(input_ev) => {
                    if let Some(action) = input::map_input_to_action(&self.state, input_ev) {
                        self.handle_action(action, &tx).await;
                    }
                }
                Event::Player(_pe) => {
                    self.handle_player(_pe, &tx).await;
                }
                Event::Network(ne) => {
                    self.handle_network(ne, &tx).await;
                }
            }

            if self.state.should_quit {
                break;
            }

            tui::draw(terminal, &self.cfg, &mut self.state)?;
        }

        // Save state before quitting
        self.save_state_on_quit();

        Ok(())
    }

    fn on_screen_enter(&mut self, tx: &mpsc::Sender<Event>) {
        match self.state.screen {
            Screen::Settings => self.spawn_load_audio_devices(tx),
            Screen::Library if !self.state.library_list.loaded => self.spawn_load_library(tx),
            _ => {}
        }
    }

    fn save_state_on_quit(&mut self) {
        // Save volume
        self.cfg.player.volume = self.state.volume;

        // Save last screen
        let screen_name = match self.state.screen {
            Screen::History => "history",
            Screen::Search => "search",
            Screen::Library => "library",
            Screen::Settings => "settings",
            Screen::Help => "help",
        };
        self.cfg.ui.last_screen = Some(screen_name.to_string());

        // Persist to disk
        let _ = crate::config::save(&self.cfg, Some(&self.config_path));
    }

    async fn handle_action(&mut self, action: Action, tx: &mpsc::Sender<Event>) {
        match action {
            Action::SetScreen(screen) => {
                // Side effects on enter.
                if screen == Screen::Settings {
                    self.spawn_load_audio_devices(tx);
                    self.update_cache_sizes();
                }
                if screen == Screen::Library && !self.state.library_list.loaded {
                    self.spawn_load_library(tx);
                }
                self.reduce(Action::SetScreen(screen));
            }
            Action::NextScreen => {
                self.reduce(Action::NextScreen);
                self.on_screen_enter(tx);
            }
            Action::PrevScreen => {
                self.reduce(Action::PrevScreen);
                self.on_screen_enter(tx);
            }
            Action::SidebarUp => {
                self.reduce(Action::SidebarUp);
                self.on_screen_enter(tx);
            }
            Action::SidebarDown => {
                self.reduce(Action::SidebarDown);
                self.on_screen_enter(tx);
            }
            Action::StartSearch => {
                self.spawn_search(tx);
            }
            Action::ListDown => {
                self.reduce(Action::ListDown);
                // Check if we should load more search results
                if self.state.screen == Screen::Search
                    && self.state.search_list.should_load_more(20) {
                        self.spawn_search_more(tx);
                    }
            }
            Action::ListUp => {
                self.reduce(Action::ListUp);
            }
            Action::PageDown => {
                self.reduce(Action::PageDown);
                // Check if we should load more search results
                if self.state.screen == Screen::Search
                    && self.state.search_list.should_load_more(20) {
                        self.spawn_search_more(tx);
                    }
            }
            Action::GoBottom => {
                self.reduce(Action::GoBottom);
                // Check if we should load more search results when going to bottom
                if self.state.screen == Screen::Search
                    && self.state.search_list.should_load_more(20) {
                        self.spawn_search_more(tx);
                    }
            }
            Action::LoadHistory => {
                self.spawn_load_history(tx);
            }
            Action::Refresh => {
                match self.state.screen {
                    Screen::History => self.spawn_load_history(tx),
                    Screen::Search => {
                        self.spawn_search(tx);
                    }
                    Screen::Library => {
                        self.state.library_list.loaded = false;
                        self.spawn_load_library(tx);
                    }
                    Screen::Settings => self.spawn_load_audio_devices(tx),
                    _ => {}
                }
            }
            Action::ApplySelectedAudioDevice => {
                self.apply_selected_audio_device(tx).await;
            }
            Action::SettingsFocusNext => {
                self.state.settings_focus = match self.state.settings_focus {
                    SettingsFocus::Authentication => SettingsFocus::AudioDevice,
                    SettingsFocus::AudioDevice => SettingsFocus::Cache,
                    SettingsFocus::Cache => SettingsFocus::Authentication,
                };
            }
            Action::SettingsFocusPrev => {
                self.state.settings_focus = match self.state.settings_focus {
                    SettingsFocus::Authentication => SettingsFocus::Cache,
                    SettingsFocus::AudioDevice => SettingsFocus::Authentication,
                    SettingsFocus::Cache => SettingsFocus::AudioDevice,
                };
            }
            Action::ApplySelectedBrowser => {
                self.apply_selected_browser();
            }
            Action::ClearCache => {
                self.clear_cache();
            }
            Action::Activate => {
                // "Activate" on a Track plays it
                let track = self.state.active_list().selected_track().cloned();
                if let Some(track) = track {
                    self.state.now_playing = Some(track.title.clone());
                    self.state.current_track = Some(track.clone());
                    self.state.status = "Resolving stream...".into();

                    // Add to history and notify UI
                    let storage = self.storage_cache_handle();
                    let track_for_history = track.clone();
                    let tx_history = tx.clone();
                    tokio::spawn(async move {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;
                        if let Ok(Ok(())) = tokio::task::spawn_blocking({
                            let storage = storage.clone();
                            let t = track_for_history.clone();
                            move || storage.add_to_history(&t, now)
                        })
                        .await
                        {
                            let _ = tx_history
                                .send(Event::Network(crate::app::events::NetworkEvent::HistoryAdded {
                                    track: track_for_history,
                                }))
                                .await;
                        }
                    });

                    // Start lyrics fetch immediately
                    self.spawn_lyrics_fetch(track.clone(), tx.clone());

                    let storage = self.storage_cache_handle();
                    let cookies = self.cfg.ytm.cookies.clone();
                    let cookies_from_browser = self.cfg.ytm.cookies_from_browser.clone();
                    let tx = tx.clone();

                    tokio::spawn(async move {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;

                        if let Ok(Ok(Some(url))) = tokio::task::spawn_blocking({
                            let storage = storage.clone();
                            let vid = track.video_id.clone();
                            move || storage.get_stream_url(&vid, now)
                        })
                        .await
                        {
                            let _ = tx
                                .send(Event::Network(crate::app::events::NetworkEvent::ResolvedStream {
                                    track,
                                    url,
                                }))
                                .await;
                            return;
                        }

                        match crate::ytm::resolve::resolve_audio_url(
                            &track.video_id,
                            cookies.as_deref(),
                            cookies_from_browser.as_deref(),
                        )
                        .await
                        {
                            Ok(url) => {
                                // Cache for 1 hour.
                                let expires_at = now + 3600;
                                let _ = tokio::task::spawn_blocking({
                                    let storage = storage.clone();
                                    let vid = track.video_id.clone();
                                    let url2 = url.clone();
                                    move || storage.cache_stream_url(&vid, &url2, expires_at, now)
                                })
                                .await;

                                let _ = tx
                                    .send(Event::Network(
                                        crate::app::events::NetworkEvent::ResolvedStream { track, url },
                                    ))
                                    .await;
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(Event::Network(crate::app::events::NetworkEvent::Error(
                                        format!("{e:#}"),
                                    )))
                                    .await;
                            }
                        }
                    });
                } else {
                    self.reduce(Action::Activate);
                }
            }
            Action::ToggleRepeatMode => {
                self.state.repeat_mode = self.state.repeat_mode.next();
                self.state.status = self.state.repeat_mode.label().into();
            }
            Action::TogglePause => {
                if let Some(mpv) = &self.mpv
                    && let Err(e) = mpv.toggle_pause().await {
                        self.state.status = format!("mpv error: {e:#}");
                    }
            }
            Action::VolumeUp => {
                let v = self.state.volume.saturating_add(5).min(100);
                self.state.volume = v;
                if let Some(mpv) = &self.mpv {
                    let _ = mpv.set_volume(v).await;
                }
            }
            Action::VolumeDown => {
                let v = self.state.volume.saturating_sub(5);
                self.state.volume = v;
                if let Some(mpv) = &self.mpv {
                    let _ = mpv.set_volume(v).await;
                }
            }
            Action::SeekForward => {
                if let Some(mpv) = &self.mpv {
                    let _ = mpv.seek_relative(10.0).await;
                }
            }
            Action::SeekBack => {
                if let Some(mpv) = &self.mpv {
                    let _ = mpv.seek_relative(-10.0).await;
                }
            }
            _ => self.reduce(action),
        }
    }

    fn spawn_search(&mut self, tx: &mpsc::Sender<Event>) {
        if self.state.search_list.loading {
            return;
        }
        if self.state.search_query.trim().is_empty() {
            self.state.status = "Type a query first".into();
            return;
        }
        let query = self.state.search_query.trim().to_string();
        self.state.search_list.loading = true;
        self.state.search_list.continuation = None;
        self.state.search_list.has_more = false;
        self.state.status = format!("Searching: {query}");

        let ytm = self.ytm.clone();
        let storage = self.storage_cache_handle();
        let tx = tx.clone();

        tokio::spawn(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            // Cache hit (freshness policy can be refined later)
            if let Ok(Ok(Some((json, _ts)))) = tokio::task::spawn_blocking({
                let storage = storage.clone();
                let query = query.clone();
                move || storage.get_cached_search(&query)
            })
            .await
                && let Ok(tracks) = serde_json::from_str::<Vec<crate::ytm::models::Track>>(&json) {
                    let _ = tx
                        .send(Event::Network(
                            crate::app::events::NetworkEvent::SearchResults { query, tracks, continuation: None },
                        ))
                        .await;
                    return;
                }

            match ytm.search_with_continuation(&query).await {
                Ok(result) => {
                    if let Ok(raw) = serde_json::to_string(&result.tracks) {
                        let _ = tokio::task::spawn_blocking({
                            let storage = storage.clone();
                            let query = query.clone();
                            move || storage.cache_search(&query, &raw, now)
                        })
                        .await;
                    }
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::SearchResults {
                            query,
                            tracks: result.tracks,
                            continuation: result.continuation,
                        }))
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::Error(
                            format!("{e:#}"),
                        )))
                        .await;
                }
            }
        });
    }

    fn spawn_search_more(&mut self, tx: &mpsc::Sender<Event>) {
        if self.state.search_list.loading_more {
            return;
        }
        let continuation = match &self.state.search_list.continuation {
            Some(c) => c.clone(),
            None => return,
        };

        self.state.search_list.loading_more = true;
        self.state.status = "Loading more results...".into();

        let ytm = self.ytm.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            match ytm.search_continue(&continuation).await {
                Ok(result) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::SearchMoreResults {
                            tracks: result.tracks,
                            continuation: result.continuation,
                        }))
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::Error(
                            format!("Load more failed: {e:#}"),
                        )))
                        .await;
                }
            }
        });
    }

    fn spawn_load_history(&mut self, tx: &mpsc::Sender<Event>) {
        if self.state.history_list.loading {
            return;
        }
        self.state.history_list.loading = true;
        self.state.status = "Loading history...".into();

        let storage = self.storage_cache_handle();
        let tx = tx.clone();
        tokio::spawn(async move {
            match tokio::task::spawn_blocking(move || storage.get_history(100)).await {
                Ok(Ok(tracks)) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::HistoryResults {
                            tracks,
                        }))
                        .await;
                }
                Ok(Err(e)) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::Error(
                            format!("{e:#}"),
                        )))
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::Error(
                            format!("spawn error: {e:#}"),
                        )))
                        .await;
                }
            }
        });
    }

    fn spawn_load_library(&mut self, tx: &mpsc::Sender<Event>) {
        if self.state.library_list.loading {
            return;
        }
        self.state.library_list.loading = true;
        self.state.status = "Loading library...".into();

        let ytm = self.ytm.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match ytm.get_liked_music().await {
                Ok(tracks) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::LibraryResults {
                            tracks,
                        }))
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::Error(
                            format!("Library: {e:#}"),
                        )))
                        .await;
                }
            }
        });
    }

    fn spawn_load_audio_devices(&mut self, tx: &mpsc::Sender<Event>) {
        self.state.audio_loaded = false;
        self.state.status = "Loading audio devices...".into();

        let tx = tx.clone();
        tokio::spawn(async move {
            let out = tokio::process::Command::new("mpv")
                .args(["--audio-device=help", "--no-video", "--idle=no"])
                .output()
                .await;

            let out = match out {
                Ok(o) => o,
                Err(e) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::Error(format!(
                            "mpv audio devices failed: {e}"
                        ))))
                        .await;
                    return;
                }
            };

            let text = String::from_utf8_lossy(&out.stdout);
            let mut devices = Vec::new();
            for line in text.lines() {
                let line = line.trim();
                if !line.starts_with('\'') {
                    continue;
                }
                // "'name' (desc)"
                if let Some(end) = line[1..].find('\'') {
                    let name = line[1..1 + end].to_string();
                    let rest = line[1 + end + 1..].trim();
                    let desc = rest
                        .trim_start_matches('(')
                        .trim_end_matches(')')
                        .to_string();
                    let _ = desc; // unused but parsed
                    devices.push(crate::app::state::AudioDevice { name });
                }
            }

            if devices.is_empty() {
                devices.push(crate::app::state::AudioDevice { name: "auto".into() });
            }

            let _ = tx
                .send(Event::Network(crate::app::events::NetworkEvent::AudioDevices { devices }))
                .await;
        });
    }

    fn reduce(&mut self, action: Action) {
        match action {
            Action::Quit => self.state.should_quit = true,
            Action::NextScreen => {
                self.state.screen = self.state.screen.next();
                self.state.sidebar_selected = screen_to_sidebar(self.state.screen);
                if self.state.screen == Screen::Search {
                    self.state.search_focus = SearchFocus::Input;
                }
            }
            Action::PrevScreen => {
                self.state.screen = self.state.screen.prev();
                self.state.sidebar_selected = screen_to_sidebar(self.state.screen);
                if self.state.screen == Screen::Search {
                    self.state.search_focus = SearchFocus::Input;
                }
            }
            Action::SidebarUp => {
                self.state.sidebar_selected = self.state.sidebar_selected.saturating_sub(1);
                self.state.screen = sidebar_to_screen(self.state.sidebar_selected);
                if self.state.screen == Screen::Search {
                    self.state.search_focus = SearchFocus::Input;
                }
            }
            Action::SidebarDown => {
                self.state.sidebar_selected = (self.state.sidebar_selected + 1).min(5);
                self.state.screen = sidebar_to_screen(self.state.sidebar_selected);
                if self.state.screen == Screen::Search {
                    self.state.search_focus = SearchFocus::Input;
                }
            }
            Action::ListUp => {
                if self.state.screen == Screen::Settings {
                    match self.state.settings_focus {
                        SettingsFocus::Authentication => {
                            self.state.auth_selected = self.state.auth_selected.saturating_sub(1);
                        }
                        SettingsFocus::AudioDevice => {
                            self.state.audio_selected = self.state.audio_selected.saturating_sub(1);
                        }
                        SettingsFocus::Cache => {}
                    }
                } else {
                    let list = self.state.active_list_mut();
                    list.select_prev();
                    list.update_scroll(20);
                }
            }
            Action::ListDown => {
                if self.state.screen == Screen::Settings {
                    match self.state.settings_focus {
                        SettingsFocus::Authentication => {
                            self.state.auth_selected =
                                (self.state.auth_selected + 1).min(self.state.auth_browsers.len().saturating_sub(1));
                        }
                        SettingsFocus::AudioDevice => {
                            self.state.audio_selected =
                                (self.state.audio_selected + 1).min(self.state.audio_devices.len().saturating_sub(1));
                        }
                        SettingsFocus::Cache => {}
                    }
                } else {
                    let list = self.state.active_list_mut();
                    list.select_next();
                    list.update_scroll(20);
                }
            }
            Action::GoTop => {
                if self.state.screen == Screen::Settings {
                    match self.state.settings_focus {
                        SettingsFocus::Authentication => self.state.auth_selected = 0,
                        SettingsFocus::AudioDevice => self.state.audio_selected = 0,
                        SettingsFocus::Cache => {}
                    }
                } else {
                    let list = self.state.active_list_mut();
                    list.selected = 0;
                    list.scroll_offset = 0;
                }
            }
            Action::GoBottom => {
                if self.state.screen == Screen::Settings {
                    match self.state.settings_focus {
                        SettingsFocus::Authentication => {
                            self.state.auth_selected = self.state.auth_browsers.len().saturating_sub(1);
                        }
                        SettingsFocus::AudioDevice => {
                            self.state.audio_selected = self.state.audio_devices.len().saturating_sub(1);
                        }
                        SettingsFocus::Cache => {}
                    }
                } else {
                    let list = self.state.active_list_mut();
                    list.selected = list.items.len().saturating_sub(1);
                    list.update_scroll(20);
                }
            }
            Action::PageUp => {
                if self.state.screen == Screen::Settings {
                    match self.state.settings_focus {
                        SettingsFocus::Authentication => {
                            self.state.auth_selected = self.state.auth_selected.saturating_sub(10);
                        }
                        SettingsFocus::AudioDevice => {
                            self.state.audio_selected = self.state.audio_selected.saturating_sub(10);
                        }
                        SettingsFocus::Cache => {}
                    }
                } else {
                    let list = self.state.active_list_mut();
                    list.selected = list.selected.saturating_sub(10);
                    list.update_scroll(20);
                }
            }
            Action::PageDown => {
                if self.state.screen == Screen::Settings {
                    match self.state.settings_focus {
                        SettingsFocus::Authentication => {
                            self.state.auth_selected =
                                (self.state.auth_selected + 10).min(self.state.auth_browsers.len().saturating_sub(1));
                        }
                        SettingsFocus::AudioDevice => {
                            self.state.audio_selected =
                                (self.state.audio_selected + 10).min(self.state.audio_devices.len().saturating_sub(1));
                        }
                        SettingsFocus::Cache => {}
                    }
                } else {
                    let list = self.state.active_list_mut();
                    list.selected = (list.selected + 10).min(list.items.len().saturating_sub(1));
                    list.update_scroll(20);
                }
            }
            Action::Activate => {
                let active = self.state.active_list();
                self.state.status = format!(
                    "Activated: {}",
                    active
                        .items
                        .get(active.selected)
                        .map(|s| s.as_str())
                        .unwrap_or("<none>")
                );
            }
            Action::ToggleRepeatMode => {} // handled in handle_action
            Action::Resize => {
                // Resize is handled by terminal
            }
            Action::SetScreen(screen) => {
                self.state.screen = screen;
                self.state.sidebar_selected = screen_to_sidebar(screen);
                if screen == Screen::Search {
                    self.state.search_focus = SearchFocus::Input;
                }
            }
            Action::SetSearchFocus(f) => self.state.search_focus = f,
            Action::InputChar(c) => self.state.search_query.push(c),
            Action::Backspace => {
                self.state.search_query.pop();
            }
            Action::ClearInput => self.state.search_query.clear(),
            Action::StartSearch => {} // handled in handle_action
            Action::LoadHistory => {} // handled in handle_action
            Action::Refresh => {}
            Action::ApplySelectedAudioDevice => {}
            Action::ApplySelectedBrowser => {}
            Action::TogglePause => {}
            Action::VolumeUp => {}
            Action::VolumeDown => {}
            Action::SeekForward => {}
            Action::SeekBack => {}
            Action::SettingsFocusNext => {} // Handled in handle_action
            Action::SettingsFocusPrev => {} // Handled in handle_action
            Action::ClearCache => {} // Handled in handle_action
        }
    }

    async fn handle_player(&mut self, pe: crate::app::events::PlayerEvent, tx: &mpsc::Sender<Event>) {
        match pe {
            crate::app::events::PlayerEvent::Started => self.state.paused = false,
            crate::app::events::PlayerEvent::Paused => self.state.paused = true,
            crate::app::events::PlayerEvent::Position { seconds } => {
                self.state.position_secs = seconds;
            }
            crate::app::events::PlayerEvent::Duration { seconds } => self.state.duration_secs = seconds,
            crate::app::events::PlayerEvent::Ended => {
                self.state.position_secs = 0.0;
                self.state.duration_secs = 0.0;

                // Handle repeat mode
                if self.state.repeat_mode == RepeatMode::One {
                    // Repeat current track
                    if let Some(track) = self.state.current_track.clone() {
                        self.state.status = format!("Repeating: {}", track.title);
                        self.play_track(track, tx).await;
                        return;
                    }
                }

                self.state.status = "Playback ended".into();
            }
            crate::app::events::PlayerEvent::Error(e) => self.state.status = format!("Player error: {e}"),
        }
    }

    async fn handle_network(&mut self, ne: crate::app::events::NetworkEvent, _tx: &mpsc::Sender<Event>) {
        match ne {
            crate::app::events::NetworkEvent::Error(e) => {
                // Reset loading state on all lists
                self.state.history_list.loading = false;
                self.state.search_list.loading = false;
                self.state.search_list.loading_more = false;
                self.state.library_list.loading = false;
                self.state.toast = Some(Toast::error(e.clone()));
                self.state.status = format!("Error: {e} (press r to retry)");
            }
            crate::app::events::NetworkEvent::SearchResults { query, tracks, continuation } => {
                self.state.last_search = Some(query);
                self.state.search_list.set_tracks(tracks);
                self.state.search_list.continuation = continuation.clone();
                self.state.search_list.has_more = continuation.is_some();
                self.state.status = format!("Results: {}", self.state.search_list.items.len());
                if !self.state.search_list.items.is_empty() {
                    self.state.search_focus = SearchFocus::Results;
                }
            }
            crate::app::events::NetworkEvent::SearchMoreResults { tracks, continuation } => {
                let count_before = self.state.search_list.items.len();
                self.state.search_list.append_tracks(tracks);
                self.state.search_list.continuation = continuation.clone();
                self.state.search_list.has_more = continuation.is_some();
                let count_after = self.state.search_list.items.len();
                self.state.status = format!("Results: {} (+{})", count_after, count_after - count_before);
            }
            crate::app::events::NetworkEvent::HistoryResults { tracks } => {
                self.state.history_list.set_tracks(tracks);
                if self.state.history_list.items.is_empty() {
                    self.state.status = "No history yet. Play some music!".into();
                } else {
                    self.state.status = format!("History: {} tracks", self.state.history_list.items.len());
                }
            }
            crate::app::events::NetworkEvent::HistoryAdded { track } => {
                // Remove existing entry if present (move to top, don't duplicate)
                if let Some(idx) = self
                    .state
                    .history_list
                    .tracks
                    .iter()
                    .position(|t| t.video_id == track.video_id)
                {
                    self.state.history_list.tracks.remove(idx);
                    self.state.history_list.items.remove(idx);
                }

                // Prepend new track to history list for immediate UI update
                let display = if track.artists.is_empty() {
                    track.title.clone()
                } else {
                    format!("{} - {}", track.title, track.artists.join(", "))
                };
                self.state.history_list.items.insert(0, display);
                self.state.history_list.tracks.insert(0, track);
                self.state.history_list.loaded = true;
                // Update cache sizes to reflect new data
                self.update_cache_sizes();
            }
            crate::app::events::NetworkEvent::LibraryResults { tracks } => {
                self.state.library_list.set_tracks(tracks);
                if self.state.library_list.items.is_empty() {
                    self.state.status = "No liked music found. Try liking songs on YouTube Music!".into();
                } else {
                    self.state.status = format!("Library: {} tracks", self.state.library_list.items.len());
                }
            }
            crate::app::events::NetworkEvent::ResolvedStream { track, url } => {
                self.state.now_playing = Some(track.title.clone());
                self.state.current_track = Some(track.clone());
                if let Some(mpv) = &self.mpv {
                    let _ = mpv.set_volume(self.state.volume).await;
                    match mpv.load_url(&url).await {
                        Ok(()) => {
                            self.state.current_url = Some(url);
                            self.state.status = "Playing".into();
                        }
                        Err(e) => {
                            self.state.status = format!("mpv load failed: {e:#}");
                        }
                    }
                } else {
                    self.state.status = "mpv not available".into();
                }
            }
            crate::app::events::NetworkEvent::AudioDevices { devices } => {
                self.state.audio_loaded = true;
                self.state.audio_devices = devices;
                self.state.status = format!("Audio devices: {}", self.state.audio_devices.len());
                // keep selection in bounds
                if self.state.audio_devices.is_empty() {
                    self.state.audio_selected = 0;
                } else {
                    self.state.audio_selected = self
                        .state
                        .audio_selected
                        .min(self.state.audio_devices.len().saturating_sub(1));
                }
            }
            crate::app::events::NetworkEvent::LyricsLoaded { video_id, lyrics } => {
                if self.state.lyrics_video_id.as_deref() == Some(video_id.as_str()) {
                    self.state.lyrics = Some(lyrics);
                    self.state.lyrics_loading = false;
                }
            }
            crate::app::events::NetworkEvent::LyricsNotFound { video_id } => {
                if self.state.lyrics_video_id.as_deref() == Some(video_id.as_str()) {
                    self.state.lyrics = None;
                    self.state.lyrics_loading = false;
                }
            }
        }
    }

    async fn play_track(&mut self, track: crate::ytm::models::Track, tx: &mpsc::Sender<Event>) {
        self.state.now_playing = Some(track.title.clone());
        self.state.current_track = Some(track.clone());

        // Add to history
        let storage = self.storage_cache_handle();
        let track_for_history = track.clone();
        tokio::spawn(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let _ = tokio::task::spawn_blocking(move || {
                storage.add_to_history(&track_for_history, now)
            })
            .await;
        });

        // Start lyrics fetch
        self.spawn_lyrics_fetch(track.clone(), tx.clone());

        // Resolve and play stream
        let storage = self.storage_cache_handle();
        let cookies = self.cfg.ytm.cookies.clone();
        let cookies_from_browser = self.cfg.ytm.cookies_from_browser.clone();
        let tx2 = tx.clone();

        tokio::spawn(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            // Check cache first
            if let Ok(Ok(Some(url))) = tokio::task::spawn_blocking({
                let storage = storage.clone();
                let vid = track.video_id.clone();
                move || storage.get_stream_url(&vid, now)
            })
            .await
            {
                let _ = tx2
                    .send(Event::Network(crate::app::events::NetworkEvent::ResolvedStream {
                        track,
                        url,
                    }))
                    .await;
                return;
            }

            match crate::ytm::resolve::resolve_audio_url(
                &track.video_id,
                cookies.as_deref(),
                cookies_from_browser.as_deref(),
            )
            .await
            {
                Ok(url) => {
                    let expires_at = now + 3600;
                    let _ = tokio::task::spawn_blocking({
                        let storage = storage.clone();
                        let vid = track.video_id.clone();
                        let url2 = url.clone();
                        move || storage.cache_stream_url(&vid, &url2, expires_at, now)
                    })
                    .await;

                    let _ = tx2
                        .send(Event::Network(crate::app::events::NetworkEvent::ResolvedStream {
                            track,
                            url,
                        }))
                        .await;
                }
                Err(e) => {
                    let _ = tx2
                        .send(Event::Network(crate::app::events::NetworkEvent::Error(
                            format!("{e:#}"),
                        )))
                        .await;
                }
            }
        });
    }

    fn spawn_lyrics_fetch(&mut self, track: crate::ytm::models::Track, tx: mpsc::Sender<Event>) {
        // Skip if we already have lyrics for this track
        if self.state.lyrics_video_id.as_deref() == Some(&track.video_id) {
            return;
        }

        self.state.lyrics = None;
        self.state.lyrics_loading = true;
        self.state.lyrics_video_id = Some(track.video_id.clone());

        let storage = self.storage_cache_handle();
        let lrclib = self.lrclib.clone();
        let title = track.title.clone();
        let artist = track.artists.first().cloned().unwrap_or_default();
        let album = track.album.clone();
        let duration = track.duration_seconds;
        let video_id = track.video_id.clone();

        tokio::spawn(async move {
            // Check cache first
            if let Ok(Ok(Some((lrc_content, synced)))) = tokio::task::spawn_blocking({
                let storage = storage.clone();
                let vid = video_id.clone();
                move || storage.get_lyrics(&vid)
            })
            .await
            {
                let lyrics = crate::lyrics::ParsedLyrics::parse(&lrc_content, synced);
                let _ = tx
                    .send(Event::Network(crate::app::events::NetworkEvent::LyricsLoaded {
                        video_id,
                        lyrics,
                    }))
                    .await;
                return;
            }

            // Fetch from LRCLIB
            match crate::lyrics::fetch_lyrics(
                &lrclib,
                &title,
                &artist,
                album.as_deref(),
                duration,
            )
            .await
            {
                Ok(Some(lyrics)) => {
                    // Cache the lyrics
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;

                    // Reconstruct LRC content for caching
                    let lrc_content: String = lyrics
                        .lines
                        .iter()
                        .map(|l| {
                            if lyrics.synced {
                                let min = l.time_ms / 60000;
                                let sec = (l.time_ms % 60000) / 1000;
                                let ms = (l.time_ms % 1000) / 10;
                                format!("[{:02}:{:02}.{:02}]{}", min, sec, ms, l.text)
                            } else {
                                l.text.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    let _ = tokio::task::spawn_blocking({
                        let storage = storage.clone();
                        let vid = video_id.clone();
                        let synced = lyrics.synced;
                        move || storage.cache_lyrics(&vid, &lrc_content, synced, now)
                    })
                    .await;

                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::LyricsLoaded {
                            video_id,
                            lyrics,
                        }))
                        .await;
                }
                Ok(None) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::LyricsNotFound {
                            video_id,
                        }))
                        .await;
                }
                Err(_) => {
                    let _ = tx
                        .send(Event::Network(crate::app::events::NetworkEvent::LyricsNotFound {
                            video_id,
                        }))
                        .await;
                }
            }
        });
    }

    fn clear_cache(&mut self) {
        let data_dir = &self.cfg.paths.data_dir;
        let cache_db = data_dir.join("cache.sqlite3");

        // Clear database file
        if cache_db.exists() {
            let _ = std::fs::remove_file(&cache_db);
        }

        // Recreate database with schema
        let _ = Storage::open(&cache_db);

        // Clear all in-memory cached state
        self.state.history_list.clear();
        self.state.search_list.clear();
        self.state.library_list.clear();
        self.state.last_search = None;

        // Clear lyrics cache
        self.state.lyrics = None;
        self.state.lyrics_video_id = None;

        // Update cache sizes to reflect actual disk usage
        self.update_cache_sizes();

        self.state.toast = Some(Toast::success("Cache cleared"));
    }

    fn update_cache_sizes(&mut self) {
        let data_dir = &self.cfg.paths.data_dir;
        let cache_db = data_dir.join("cache.sqlite3");

        // Get database size
        self.state.cache_size_bytes = std::fs::metadata(&cache_db)
            .map(|m| m.len())
            .unwrap_or(0);
    }

    fn apply_selected_browser(&mut self) {
        let browser = self.state.auth_browsers[self.state.auth_selected];

        // Update config
        if browser == "none" {
            self.cfg.ytm.cookies_from_browser = None;
        } else if browser == "zen" {
            // Zen browser needs special handling - detect profile path
            match Self::detect_zen_profile() {
                Ok(profile_str) => {
                    self.cfg.ytm.cookies_from_browser = Some(profile_str);
                }
                Err(e) => {
                    self.state.toast = Some(Toast::error(format!("Zen detection failed: {e}")));
                    return;
                }
            }
        } else {
            self.cfg.ytm.cookies_from_browser = Some(browser.to_string());
        }

        // Save config
        if let Err(e) = crate::config::save(&self.cfg, Some(&self.config_path)) {
            self.state.toast = Some(Toast::error(format!("Failed to save config: {e}")));
            return;
        }

        // Recreate YTM client with new auth settings
        let auth = match self.cfg.ytm.cookies.as_deref() {
            Some(p) if p.exists() => ytm::auth::load_netscape_cookies(p).ok(),
            _ => None,
        };

        match YtmClient::new(auth) {
            Ok(client) => {
                self.ytm = client;
                if browser == "none" {
                    self.state.toast = Some(Toast::success("Authentication disabled"));
                } else {
                    self.state.toast = Some(Toast::success(format!("Browser set to: {}", browser)));
                }
            }
            Err(e) => {
                self.state.toast = Some(Toast::error(format!("Failed to reinitialize: {e}")));
            }
        }
    }

    /// Detect Zen browser profile path and return yt-dlp compatible string.
    /// Zen is Firefox-based, so we use "firefox:{profile_path}" format.
    fn detect_zen_profile() -> anyhow::Result<String> {
        use anyhow::Context;

        let base_dirs = directories::BaseDirs::new()
            .ok_or_else(|| anyhow::anyhow!("BaseDirs unavailable"))?;

        // Zen stores profiles in platform-specific locations
        #[cfg(target_os = "macos")]
        let base = base_dirs
            .home_dir()
            .join("Library")
            .join("Application Support")
            .join("zen");

        #[cfg(target_os = "linux")]
        let base = base_dirs.home_dir().join(".zen");

        #[cfg(target_os = "windows")]
        let base = base_dirs.data_dir().join("zen");

        // Read profiles.ini to find default profile
        let ini_path = base.join("profiles.ini");
        let raw = std::fs::read_to_string(&ini_path)
            .with_context(|| format!("read {}", ini_path.display()))?;

        // Find the default profile path
        let default_rel: String = raw
            .lines()
            .find_map(|l: &str| l.strip_prefix("Default="))
            .map(|s: &str| s.trim().to_string())
            .ok_or_else(|| anyhow::anyhow!("no Default=... in zen profiles.ini"))?;

        let profile_dir = base.join(&default_rel);

        if !profile_dir.exists() {
            anyhow::bail!("Zen profile not found: {}", profile_dir.display());
        }

        Ok(format!("firefox:{}", profile_dir.display()))
    }

    async fn apply_selected_audio_device(&mut self, tx: &mpsc::Sender<Event>) {
        if self.state.audio_devices.is_empty() {
            self.state.toast = Some(Toast::error("No audio devices loaded"));
            return;
        }
        let dev = self
            .state
            .audio_devices
            .get(self.state.audio_selected)
            .cloned()
            .unwrap();

        // Persist
        if dev.name == "auto" {
            self.cfg.player.audio_device = None;
        } else {
            self.cfg.player.audio_device = Some(dev.name.clone());
        }
        let _ = crate::config::save(&self.cfg, Some(&self.config_path));

        // Restart mpv to apply device, and reload current stream if any.
        self.state.status = format!("Applying audio device: {}", dev.name);
        self.mpv = None;
        let mpv_log = self.cfg.paths.data_dir.join("mpv.log");
        match MpvHandle::spawn(
            tx.clone(),
            self.cfg.player.audio_device.as_deref(),
            Some(&mpv_log),
        )
        .await
        {
            Ok(h) => {
                self.mpv = Some(h);
                if let Some(mpv) = &self.mpv {
                    let _ = mpv.set_volume(self.state.volume).await;
                    if let Some(url) = self.state.current_url.clone() {
                        let _ = mpv.load_url(&url).await;
                    }
                }
                self.state.status = "Audio device applied".into();
            }
            Err(e) => {
                self.state.status = format!("mpv restart failed: {e:#}");
            }
        }
    }

    fn storage_cache_handle(&self) -> StorageHandle {
        StorageHandle {
            path: self.cfg.paths.data_dir.join("cache.sqlite3"),
        }
    }
}

fn sidebar_to_screen(idx: usize) -> Screen {
    match idx {
        0 => Screen::History,
        1 => Screen::Search,
        2 => Screen::Library,
        3 => Screen::Settings,
        _ => Screen::Help,
    }
}

fn screen_to_sidebar(screen: Screen) -> usize {
    match screen {
        Screen::History => 0,
        Screen::Search => 1,
        Screen::Library => 2,
        Screen::Settings => 3,
        Screen::Help => 4,
    }
}

// Simple way to use rusqlite from async tasks: open per-operation.
// (Phase 5 can pool this; Phase 1 prefers simplicity + correctness.)
#[derive(Clone)]
struct StorageHandle {
    path: std::path::PathBuf,
}

impl StorageHandle {
    fn open(&self) -> anyhow::Result<Storage> {
        Storage::open(&self.path)
    }

    fn get_cached_search(&self, query: &str) -> anyhow::Result<Option<(String, i64)>> {
        self.open()?.get_cached_search(query)
    }

    fn cache_search(&self, query: &str, results_json: &str, now_unix: i64) -> anyhow::Result<()> {
        self.open()?.cache_search(query, results_json, now_unix)
    }

    fn get_stream_url(&self, video_id: &str, now_unix: i64) -> anyhow::Result<Option<String>> {
        self.open()?.get_stream_url(video_id, now_unix)
    }

    fn cache_stream_url(
        &self,
        video_id: &str,
        url: &str,
        expires_at: i64,
        now_unix: i64,
    ) -> anyhow::Result<()> {
        self.open()?
            .cache_stream_url(video_id, url, expires_at, now_unix)
    }

    fn add_to_history(&self, track: &crate::ytm::models::Track, played_at: i64) -> anyhow::Result<()> {
        self.open()?.add_to_history(track, played_at)
    }

    fn get_history(&self, limit: usize) -> anyhow::Result<Vec<crate::ytm::models::Track>> {
        self.open()?.get_history(limit)
    }

    fn get_lyrics(&self, video_id: &str) -> anyhow::Result<Option<(String, bool)>> {
        self.open()?.get_lyrics(video_id)
    }

    fn cache_lyrics(&self, video_id: &str, lrc_content: &str, synced: bool, now_unix: i64) -> anyhow::Result<()> {
        self.open()?.cache_lyrics(video_id, lrc_content, synced, now_unix)
    }
}

