use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub video_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub track_count: Option<u32>,
    pub thumbnail_url: Option<String>,
}

/// Unified search result item that can be either a track or a playlist
#[derive(Debug, Clone)]
pub enum SearchItem {
    Track(Track),
    Playlist(Playlist),
}

#[allow(dead_code)]
impl SearchItem {
    pub fn display_title(&self) -> &str {
        match self {
            SearchItem::Track(t) => &t.title,
            SearchItem::Playlist(p) => &p.title,
        }
    }

    pub fn display_subtitle(&self) -> String {
        match self {
            SearchItem::Track(t) => {
                if t.artists.is_empty() {
                    String::new()
                } else {
                    t.artists.join(", ")
                }
            }
            SearchItem::Playlist(p) => {
                let mut parts = Vec::new();
                if let Some(author) = &p.author {
                    parts.push(author.clone());
                }
                if let Some(count) = p.track_count {
                    parts.push(format!("{} tracks", count));
                }
                parts.join(" - ")
            }
        }
    }

    pub fn is_track(&self) -> bool {
        matches!(self, SearchItem::Track(_))
    }

    pub fn is_playlist(&self) -> bool {
        matches!(self, SearchItem::Playlist(_))
    }

    pub fn as_track(&self) -> Option<&Track> {
        match self {
            SearchItem::Track(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_playlist(&self) -> Option<&Playlist> {
        match self {
            SearchItem::Playlist(p) => Some(p),
            _ => None,
        }
    }
}

