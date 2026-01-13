//! Border styles

use ratatui::symbols::border;

/// Border style - using rounded by default
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BorderStyle;

impl BorderStyle {
    pub fn to_border_set() -> border::Set<'static> {
        border::ROUNDED
    }
}
