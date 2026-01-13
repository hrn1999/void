//! Lyrics module for fetching and displaying synchronized lyrics
//!
//! This module provides:
//! - LRCLIB API client for fetching lyrics
//! - LRC format parser for synchronized lyrics
//! - Data structures for lyrics display

pub mod lrclib;
pub mod parser;

pub use lrclib::LrclibClient;
pub use parser::ParsedLyrics;

/// Get lyrics for a track
pub async fn fetch_lyrics(
    client: &LrclibClient,
    title: &str,
    artist: &str,
    album: Option<&str>,
    duration_secs: Option<u32>,
) -> anyhow::Result<Option<ParsedLyrics>> {
    let result = client.get_lyrics(title, artist, album, duration_secs).await?;

    if let Some(lyrics) = result {
        // Try synced lyrics first, fall back to plain
        if let Some(synced) = &lyrics.synced_lyrics
            && !synced.is_empty() {
                return Ok(Some(ParsedLyrics::parse(synced, true)));
            }
        if let Some(plain) = &lyrics.plain_lyrics
            && !plain.is_empty() {
                return Ok(Some(ParsedLyrics::parse(plain, false)));
            }
    }

    Ok(None)
}
