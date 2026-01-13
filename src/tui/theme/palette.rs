//! Color palette - Monochrome grayscale theme

use ratatui::style::Color;

/// Monochrome grayscale palette
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Palette {
    pub bg_primary: Color,
    pub bg_secondary: Color,
    pub bg_highlight: Color,
    pub fg_primary: Color,
    pub fg_secondary: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub border: Color,
    pub playing: Color,
    pub error: Color,
}

impl Palette {
    /// Monochrome palette - pure black, white, and grays
    pub const MONO: Self = Self {
        bg_primary: Color::Rgb(0, 0, 0),          // #000000 pure black
        bg_secondary: Color::Rgb(18, 18, 18),    // #121212 near black
        bg_highlight: Color::Rgb(48, 48, 48),    // #303030 dark gray
        fg_primary: Color::Rgb(255, 255, 255),   // #ffffff white
        fg_secondary: Color::Rgb(136, 136, 136), // #888888 medium gray
        accent: Color::Rgb(255, 255, 255),       // #ffffff white (accent = white)
        accent_alt: Color::Rgb(200, 200, 200),   // #c8c8c8 light gray
        border: Color::Rgb(64, 64, 64),          // #404040 dark gray
        playing: Color::Rgb(255, 255, 255),      // #ffffff white
        error: Color::Rgb(255, 255, 255),        // #ffffff white (errors still visible via icon)
    };
}

impl Default for Palette {
    fn default() -> Self {
        Self::MONO
    }
}
