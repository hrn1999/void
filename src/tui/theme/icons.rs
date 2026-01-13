//! Nerd Font icons for TUI display
//! Requires a Nerd Font to be installed (https://www.nerdfonts.com)

/// Icon set using Nerd Font glyphs
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Icons {
    // Playback controls
    pub play: &'static str,
    pub pause: &'static str,
    pub stop: &'static str,
    pub next: &'static str,
    pub prev: &'static str,

    // Volume
    pub volume: &'static str,
    pub volume_mute: &'static str,
    pub volume_low: &'static str,
    pub volume_high: &'static str,

    // Repeat/Shuffle
    pub repeat: &'static str,
    pub repeat_one: &'static str,
    pub shuffle: &'static str,

    // Navigation
    pub home: &'static str,
    pub search: &'static str,
    pub library: &'static str,
    pub queue: &'static str,
    pub history: &'static str,
    pub settings: &'static str,
    pub help: &'static str,

    // Status
    pub success: &'static str,
    pub error: &'static str,
    pub loading: &'static str,
    pub info: &'static str,

    // Music
    pub music: &'static str,
    pub artist: &'static str,
    pub album: &'static str,
    pub playlist: &'static str,
    pub lyrics: &'static str,
    pub radio: &'static str,
    pub favorite: &'static str,
    pub star: &'static str,

    // Selection
    pub selected: &'static str,
    pub unselected: &'static str,

    // Progress bar
    pub progress_full: &'static str,
    pub progress_empty: &'static str,
    pub progress_head: &'static str,

    // Separators
    pub separator: &'static str,
    pub bullet: &'static str,

    // Misc
    pub folder: &'static str,
    pub file: &'static str,
    pub download: &'static str,
    pub cache: &'static str,
    pub command: &'static str,
}

impl Icons {
    /// Nerd Font icon set
    pub const fn nerd() -> Self {
        Self {
            // Playback - nf-fa-* and nf-md-*
            play: "\u{f04b}",           // nf-fa-play
            pause: "\u{f04c}",          // nf-fa-pause
            stop: "\u{f04d}",           // nf-fa-stop
            next: "\u{f051}",           // nf-fa-step_forward
            prev: "\u{f048}",           // nf-fa-step_backward

            // Volume - nf-fa-volume_*
            volume: "\u{f028}",         // nf-fa-volume_up
            volume_mute: "\u{f026}",    // nf-fa-volume_off
            volume_low: "\u{f027}",     // nf-fa-volume_down
            volume_high: "\u{f028}",    // nf-fa-volume_up

            // Repeat/Shuffle - nf-md-*
            repeat: "\u{f456}",         // nf-md-repeat
            repeat_one: "\u{f458}",     // nf-md-repeat_once
            shuffle: "\u{f49d}",        // nf-md-shuffle

            // Navigation - mixed nf-*
            home: "\u{f015}",           // nf-fa-home
            search: "\u{f002}",         // nf-fa-search
            library: "\u{f02d}",        // nf-fa-book
            queue: "\u{f03a}",          // nf-fa-list
            history: "\u{f1da}",        // nf-fa-history
            settings: "\u{f013}",       // nf-fa-cog
            help: "\u{f059}",           // nf-fa-question_circle

            // Status
            success: "\u{f00c}",        // nf-fa-check
            error: "\u{f00d}",          // nf-fa-times
            loading: "\u{f110}",        // nf-fa-spinner
            info: "\u{f05a}",           // nf-fa-info_circle

            // Music - nf-md-* and nf-fa-*
            music: "\u{f001}",          // nf-fa-music
            artist: "\u{f007}",         // nf-fa-user
            album: "\u{f51f}",          // nf-md-album
            playlist: "\u{f0cb}",       // nf-fa-list_ol
            lyrics: "\u{f15c}",         // nf-fa-file_text_o
            radio: "\u{f519}",          // nf-md-radio
            favorite: "\u{f004}",       // nf-fa-heart
            star: "\u{f005}",           // nf-fa-star

            // Selection
            selected: "\u{f054}",       // nf-fa-chevron_right
            unselected: " ",

            // Progress bar
            progress_full: "━",
            progress_empty: "─",
            progress_head: "●",

            // Separators
            separator: "─",
            bullet: "•",

            // Misc
            folder: "\u{f07b}",         // nf-fa-folder
            file: "\u{f15b}",           // nf-fa-file
            download: "\u{f019}",       // nf-fa-download
            cache: "\u{f1c0}",          // nf-fa-database
            command: "\u{f120}",        // nf-fa-terminal
        }
    }
}

impl Default for Icons {
    fn default() -> Self {
        Self::nerd()
    }
}

/// Loading spinner frames
pub struct LoadingSpinner;

impl LoadingSpinner {
    /// Braille-based smooth spinner
    pub const BRAILLE: [&'static str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

    pub fn frame(tick: u64) -> &'static str {
        let idx = (tick / 4) as usize % Self::BRAILLE.len();
        Self::BRAILLE[idx]
    }
}
