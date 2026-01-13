//! Theme configuration - Monochrome grayscale

pub mod borders;
pub mod icons;
pub mod palette;

pub use borders::BorderStyle;
pub use icons::{Icons, LoadingSpinner};
pub use palette::Palette;

/// Active theme configuration
#[derive(Debug, Clone)]
pub struct Theme {
    pub palette: Palette,
    pub icons: Icons,
}

impl Theme {
    pub fn new() -> Self {
        Self {
            palette: Palette::MONO,
            icons: Icons::nerd(),
        }
    }

    pub fn border_set(&self) -> ratatui::symbols::border::Set<'static> {
        BorderStyle::to_border_set()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the theme (always Mono)
pub fn get_theme() -> Theme {
    Theme::new()
}
