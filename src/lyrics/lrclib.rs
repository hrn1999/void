//! LRCLIB API client
//!
//! LRCLIB is a free lyrics API that provides synchronized (LRC format) lyrics.
//! API Documentation: https://lrclib.net/docs

use serde::Deserialize;

/// LRCLIB API response
#[derive(Debug, Deserialize, Clone)]
pub struct LrclibResponse {
    #[allow(dead_code)]
    id: i64,
    #[allow(dead_code)]
    #[serde(rename = "trackName")]
    track_name: String,
    #[allow(dead_code)]
    #[serde(rename = "artistName")]
    artist_name: String,
    #[allow(dead_code)]
    #[serde(rename = "albumName")]
    album_name: Option<String>,
    #[allow(dead_code)]
    duration: Option<f64>,
    #[serde(rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    pub synced_lyrics: Option<String>,
}

/// LRCLIB API client
#[derive(Debug, Clone)]
pub struct LrclibClient {
    client: reqwest::Client,
    base_url: String,
}

impl LrclibClient {
    const DEFAULT_BASE_URL: &'static str = "https://lrclib.net/api";
    const USER_AGENT: &'static str = "Kakariko/0.1.0 (https://github.com/kakariko)";

    /// Create a new LRCLIB client
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent(Self::USER_AGENT)
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to create reqwest client"),
            base_url: Self::DEFAULT_BASE_URL.to_string(),
        }
    }

    /// Get lyrics by track info
    pub async fn get_lyrics(
        &self,
        track_name: &str,
        artist_name: &str,
        album_name: Option<&str>,
        duration_secs: Option<u32>,
    ) -> anyhow::Result<Option<LrclibResponse>> {
        // First try the "get" endpoint with exact match
        if let Some(lyrics) = self.get_exact(track_name, artist_name, album_name, duration_secs).await? {
            return Ok(Some(lyrics));
        }

        // Fall back to search
        self.search(track_name, artist_name).await
    }

    /// Get lyrics with exact match
    async fn get_exact(
        &self,
        track_name: &str,
        artist_name: &str,
        album_name: Option<&str>,
        duration_secs: Option<u32>,
    ) -> anyhow::Result<Option<LrclibResponse>> {
        let mut url = format!(
            "{}/get?track_name={}&artist_name={}",
            self.base_url,
            urlencoding::encode(track_name),
            urlencoding::encode(artist_name)
        );

        if let Some(album) = album_name {
            url.push_str(&format!("&album_name={}", urlencoding::encode(album)));
        }

        if let Some(duration) = duration_secs {
            url.push_str(&format!("&duration={}", duration));
        }

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let lyrics: LrclibResponse = response.json().await?;
            Ok(Some(lyrics))
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            anyhow::bail!("LRCLIB API error: {}", response.status());
        }
    }

    /// Search for lyrics
    async fn search(
        &self,
        track_name: &str,
        artist_name: &str,
    ) -> anyhow::Result<Option<LrclibResponse>> {
        let query = format!("{} {}", track_name, artist_name);
        let url = format!(
            "{}/search?q={}",
            self.base_url,
            urlencoding::encode(&query)
        );

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let results: Vec<LrclibResponse> = response.json().await?;

            // Return the first result that has synced lyrics, or any result
            let best = results
                .iter()
                .find(|r| r.synced_lyrics.is_some())
                .or_else(|| results.first());

            Ok(best.cloned())
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            anyhow::bail!("LRCLIB search error: {}", response.status());
        }
    }
}

impl Default for LrclibClient {
    fn default() -> Self {
        Self::new()
    }
}
