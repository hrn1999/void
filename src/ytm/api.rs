use crate::ytm::auth::AuthState;
use crate::ytm::models::{Playlist, SearchItem, Track};
use anyhow::Context;
use reqwest::header::{
    HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT,
};
use serde_json::json;
use sha1::{Digest, Sha1};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Search results with optional continuation token for pagination
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub tracks: Vec<Track>,
    pub continuation: Option<String>,
}

/// Search results with mixed types (tracks + playlists)
#[derive(Debug, Clone)]
pub struct SearchAllResult {
    pub items: Vec<SearchItem>,
    pub continuation: Option<String>,
}

#[derive(Debug)]
struct Inner {
    http: reqwest::Client,
    auth: Option<AuthState>,
    bootstrap: OnceCell<Bootstrap>,
}

#[derive(Debug, Clone)]
pub struct YtmClient {
    inner: Arc<Inner>,
}

#[derive(Debug, Clone)]
struct Bootstrap {
    api_key: String,
    client_version: String,
    visitor_data: Option<String>,
}

impl YtmClient {
    pub fn new(auth: Option<AuthState>) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36"),
        );
        headers.insert(ORIGIN, HeaderValue::from_static("https://music.youtube.com"));
        headers.insert(REFERER, HeaderValue::from_static("https://music.youtube.com/"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(a) = &auth {
            if !a.cookie_header.is_empty() {
                headers.insert(COOKIE, HeaderValue::from_str(&a.cookie_header)?);
            }
            if let Some(sapisid) = &a.sapisid {
                let authz = make_sapisid_hash_auth("https://music.youtube.com", sapisid);
                headers.insert(AUTHORIZATION, HeaderValue::from_str(&authz)?);
            }
        }

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .context("build reqwest client")?;

        Ok(Self {
            inner: Arc::new(Inner {
                http,
                auth,
                bootstrap: OnceCell::new(),
            }),
        })
    }

    pub async fn search_tracks(&self, query: &str) -> anyhow::Result<Vec<Track>> {
        let result = self.search_with_continuation(query).await?;
        Ok(result.tracks)
    }

    /// Search with continuation token support for pagination
    pub async fn search_with_continuation(&self, query: &str) -> anyhow::Result<SearchResult> {
        let v = self.search_raw(query).await?;
        let tracks = extract_tracks_from_search(&v);
        let continuation = extract_continuation_token(&v);
        Ok(SearchResult { tracks, continuation })
    }

    /// Continue search using a continuation token
    pub async fn search_continue(&self, continuation: &str) -> anyhow::Result<SearchResult> {
        let b = self.bootstrap().await?;

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "continuation": continuation
        });

        let v: serde_json::Value = self
            .innertube_post("search", &b)
            .json(&body)
            .send()
            .await
            .context("send search continuation request")?
            .error_for_status()
            .context("search continuation http status")?
            .json()
            .await
            .context("parse search continuation json")?;

        let tracks = extract_tracks_from_continuation(&v);
        let next_continuation = extract_continuation_token(&v);
        Ok(SearchResult { tracks, continuation: next_continuation })
    }

    pub async fn search_raw(&self, query: &str) -> anyhow::Result<serde_json::Value> {
        let b = self.bootstrap().await?;

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "query": query,
            // This params value is commonly used to bias towards songs in YTM.
            // We'll keep it optional if YouTube changes behavior; search still returns items.
            "params": "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D"
        });

        let v: serde_json::Value = self
            .innertube_post("search", &b)
            .json(&body)
            .send()
            .await
            .context("send search request")?
            .error_for_status()
            .context("search http status")?
            .json()
            .await
            .context("parse search json")?;
        Ok(v)
    }

    /// Search for playlists only
    pub async fn search_playlists_raw(&self, query: &str) -> anyhow::Result<serde_json::Value> {
        let b = self.bootstrap().await?;

        // Params for playlists filter: EgeKAQQoAEABagoQAxAEEAoQCRAF
        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "query": query,
            "params": "EgeKAQQoAEABagoQAxAEEAoQCRAF"
        });

        let v: serde_json::Value = self
            .innertube_post("search", &b)
            .json(&body)
            .send()
            .await
            .context("send search playlists request")?
            .error_for_status()
            .context("search playlists http status")?
            .json()
            .await
            .context("parse search playlists json")?;
        Ok(v)
    }

    /// Search and return mixed results (tracks with pagination + playlists prepended)
    /// This runs both a songs search (with pagination) and a playlists search,
    /// merging the results so playlists appear first.
    pub async fn search_all(&self, query: &str) -> anyhow::Result<SearchAllResult> {
        // Run songs search (with pagination support) and playlists search in parallel
        let (songs_result, playlists_result) = tokio::join!(
            self.search_with_continuation(query),
            self.search_playlists_raw(query)
        );

        let songs = songs_result?;
        let playlists_json = playlists_result.ok();

        // Extract playlists from playlist search
        let playlists: Vec<SearchItem> = playlists_json
            .as_ref()
            .map(|v| extract_playlists_from_search(v))
            .unwrap_or_default()
            .into_iter()
            .take(5) // Limit to top 5 playlists to not overwhelm results
            .map(SearchItem::Playlist)
            .collect();

        // Convert tracks to SearchItems
        let tracks: Vec<SearchItem> = songs.tracks.into_iter().map(SearchItem::Track).collect();

        // Merge: playlists first, then tracks
        let mut items = playlists;
        items.extend(tracks);

        Ok(SearchAllResult {
            items,
            continuation: songs.continuation,
        })
    }

    pub async fn browse_home_tracks(&self) -> anyhow::Result<Vec<Track>> {
        let v = self.browse_home_raw().await?;
        Ok(extract_tracks_generic(&v))
    }

    pub async fn browse_home_raw(&self) -> anyhow::Result<serde_json::Value> {
        let b = self.bootstrap().await?;
        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "browseId": "FEmusic_home"
        });

        let v: serde_json::Value = self
            .innertube_post("browse", &b)
            .json(&body)
            .send()
            .await
            .context("send browse home request")?
            .error_for_status()
            .context("browse home http status")?
            .json()
            .await
            .context("parse browse home json")?;
        Ok(v)
    }

    pub async fn browse_playlist_tracks(&self, playlist_id: &str) -> anyhow::Result<Vec<Track>> {
        let b = self.bootstrap().await?;
        let browse_id = if playlist_id.starts_with("VL") {
            playlist_id.to_string()
        } else {
            format!("VL{}", playlist_id)
        };

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "browseId": browse_id
        });

        let v: serde_json::Value = self
            .innertube_post("browse", &b)
            .json(&body)
            .send()
            .await
            .context("send browse playlist request")?
            .error_for_status()
            .context("browse playlist http status")?
            .json()
            .await
            .context("parse browse playlist json")?;

        Ok(extract_tracks_generic(&v))
    }

    /// Get user's liked music playlist (requires authentication)
    pub async fn get_liked_music(&self) -> anyhow::Result<Vec<Track>> {
        let b = self.bootstrap().await?;

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "browseId": "FEmusic_liked_videos"
        });

        let v: serde_json::Value = self
            .innertube_post("browse", &b)
            .json(&body)
            .send()
            .await
            .context("send browse liked music request")?
            .error_for_status()
            .context("browse liked music http status")?
            .json()
            .await
            .context("parse browse liked music json")?;

        Ok(extract_tracks_generic(&v))
    }

    /// Get user's playlists (requires authentication)
    #[allow(dead_code)]
    pub async fn get_user_playlists(&self) -> anyhow::Result<Vec<Playlist>> {
        let b = self.bootstrap().await?;

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "browseId": "FEmusic_library_privately_owned_playlists"
        });

        let v: serde_json::Value = self
            .innertube_post("browse", &b)
            .json(&body)
            .send()
            .await
            .context("send browse playlists request")?
            .error_for_status()
            .context("browse playlists http status")?
            .json()
            .await
            .context("parse browse playlists json")?;

        Ok(extract_playlists(&v))
    }

    /// Get user's saved albums (requires authentication)
    #[allow(dead_code)]
    pub async fn get_user_albums(&self) -> anyhow::Result<Vec<Playlist>> {
        let b = self.bootstrap().await?;

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "browseId": "FEmusic_library_albums"
        });

        let v: serde_json::Value = self
            .innertube_post("browse", &b)
            .json(&body)
            .send()
            .await
            .context("send browse albums request")?
            .error_for_status()
            .context("browse albums http status")?
            .json()
            .await
            .context("parse browse albums json")?;

        Ok(extract_playlists(&v))
    }

    /// Get radio/automix tracks based on a seed video ID.
    /// Returns tracks similar to the given video for endless playback.
    #[allow(dead_code)]
    pub async fn get_radio_tracks(&self, video_id: &str) -> anyhow::Result<Vec<Track>> {
        let v = self.get_radio_raw(video_id).await?;
        Ok(extract_radio_tracks(&v))
    }

    /// Get raw JSON response from the radio/next endpoint
    #[allow(dead_code)]
    pub async fn get_radio_raw(&self, video_id: &str) -> anyhow::Result<serde_json::Value> {
        let b = self.bootstrap().await?;

        // Radio playlist ID format: RDAMVM{videoId}
        let playlist_id = format!("RDAMVM{}", video_id);

        let body = json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": b.client_version,
                }
            },
            "videoId": video_id,
            "playlistId": playlist_id,
            "isAudioOnly": true
        });

        let v: serde_json::Value = self
            .innertube_post("next", &b)
            .json(&body)
            .send()
            .await
            .context("send radio/next request")?
            .error_for_status()
            .context("radio/next http status")?
            .json()
            .await
            .context("parse radio/next json")?;

        Ok(v)
    }

    async fn bootstrap(&self) -> anyhow::Result<Bootstrap> {
        self.inner
            .bootstrap
            .get_or_try_init(|| async {
                let html = self
                    .inner
                    .http
                    .get("https://music.youtube.com/")
                    .send()
                    .await
                    .context("fetch music.youtube.com for bootstrap")?
                    .error_for_status()
                    .context("bootstrap http status")?
                    .text()
                    .await
                    .context("read bootstrap html")?;

                let api_key = parse_ytcfg_value(&html, "INNERTUBE_API_KEY")
                    .context("parse INNERTUBE_API_KEY")?;
                let client_version = parse_ytcfg_value(&html, "INNERTUBE_CLIENT_VERSION")
                    .context("parse INNERTUBE_CLIENT_VERSION")?;
                let visitor_data = parse_ytcfg_value(&html, "VISITOR_DATA");

                Ok(Bootstrap {
                    api_key,
                    client_version,
                    visitor_data,
                })
            })
            .await
            .cloned()
    }

    fn innertube_post(&self, path: &str, b: &Bootstrap) -> reqwest::RequestBuilder {
        let url = format!(
            "https://music.youtube.com/youtubei/v1/{path}?key={}&prettyPrint=false",
            b.api_key
        );

        let mut rb = self
            .inner
            .http
            .post(url)
            .header("X-Youtube-Client-Name", "67")
            .header("X-Youtube-Client-Version", b.client_version.as_str())
            .header(
                "X-Youtube-Bootstrap-Logged-In",
                if self.inner.auth.is_some() {
                    "true"
                } else {
                    "false"
                },
            );

        if let Some(v) = b.visitor_data.as_deref() {
            rb = rb.header("X-Goog-Visitor-Id", v);
        }

        rb
    }
}

fn make_sapisid_hash_auth(origin: &str, sapisid: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let input = format!("{ts} {sapisid} {origin}");
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    let out = hasher.finalize();
    format!("SAPISIDHASH {ts}_{}", hex::encode(out))
}

fn extract_tracks_from_search(v: &serde_json::Value) -> Vec<Track> {
    // Best-effort extraction; YouTube's structure changes often.
    // We scan for `musicResponsiveListItemRenderer` nodes that contain a `watchEndpoint.videoId`.
    let mut out = Vec::new();
    scan_value(v, &mut |node| {
        let r = node.get("musicResponsiveListItemRenderer")?;
        let video_id = extract_video_id_from_item(r)?;

        let title = r
            .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
            .and_then(|x| x.as_str())
            .unwrap_or("Unknown title")
            .to_string();

        let artists = r
            .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
            .and_then(|x| x.as_array())
            .map(|runs| {
                runs.iter()
                    .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                    .filter(|t| *t != " • ")
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Some(Track {
            video_id,
            title,
            artists,
            album: None,
            duration_seconds: None,
        })
    }, &mut out);
    out
}

fn extract_tracks_generic(v: &serde_json::Value) -> Vec<Track> {
    // Generic extraction used by browse/home/playlist responses.
    let mut out = Vec::new();
    scan_value(v, &mut |node| {
        let r = node.get("musicResponsiveListItemRenderer")?;
        let video_id = extract_video_id_from_item(r)?;

        let title = r
            .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
            .and_then(|x| x.as_str())
            .unwrap_or("Unknown title")
            .to_string();

        let artists = r
            .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
            .and_then(|x| x.as_array())
            .map(|runs| {
                runs.iter()
                    .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                    .filter(|t| *t != " • ")
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Some(Track {
            video_id,
            title,
            artists,
            album: None,
            duration_seconds: None,
        })
    }, &mut out);
    out
}

#[allow(dead_code)]
fn extract_radio_tracks(v: &serde_json::Value) -> Vec<Track> {
    // Radio/next response has a different structure with playlistPanelVideoRenderer
    let mut out = Vec::new();

    scan_value(v, &mut |node| {
        // Try playlistPanelVideoRenderer (common in radio/next responses)
        if let Some(r) = node.get("playlistPanelVideoRenderer") {
            let video_id = r
                .pointer("/navigationEndpoint/watchEndpoint/videoId")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())?;

            let title = r
                .pointer("/title/runs/0/text")
                .and_then(|x| x.as_str())
                .unwrap_or("Unknown title")
                .to_string();

            let artists = r
                .pointer("/shortBylineText/runs")
                .and_then(|x| x.as_array())
                .map(|runs| {
                    runs.iter()
                        .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                        .filter(|t| *t != " • " && *t != " & ")
                        .map(|t| t.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            // Extract duration from lengthText
            let duration_seconds = r
                .pointer("/lengthText/runs/0/text")
                .and_then(|x| x.as_str())
                .and_then(parse_duration_text);

            return Some(Track {
                video_id,
                title,
                artists,
                album: None,
                duration_seconds,
            });
        }

        // Also try automixPreviewVideoRenderer
        if let Some(r) = node.get("automixPreviewVideoRenderer") {
            let video_id = r
                .pointer("/content/automixPlaylistVideoRenderer/navigationEndpoint/watchEndpoint/videoId")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())?;

            let title = r
                .pointer("/content/automixPlaylistVideoRenderer/title/runs/0/text")
                .and_then(|x| x.as_str())
                .unwrap_or("Unknown title")
                .to_string();

            return Some(Track {
                video_id,
                title,
                artists: vec![],
                album: None,
                duration_seconds: None,
            });
        }

        None
    }, &mut out);

    out
}

/// Parse duration text like "3:45" or "1:23:45" into seconds
#[allow(dead_code)]
fn parse_duration_text(text: &str) -> Option<u32> {
    let parts: Vec<&str> = text.split(':').collect();
    match parts.len() {
        2 => {
            // MM:SS
            let mins: u32 = parts[0].parse().ok()?;
            let secs: u32 = parts[1].parse().ok()?;
            Some(mins * 60 + secs)
        }
        3 => {
            // HH:MM:SS
            let hours: u32 = parts[0].parse().ok()?;
            let mins: u32 = parts[1].parse().ok()?;
            let secs: u32 = parts[2].parse().ok()?;
            Some(hours * 3600 + mins * 60 + secs)
        }
        _ => None,
    }
}

fn extract_video_id_from_item(r: &serde_json::Value) -> Option<String> {
    // Seen variants:
    // - musicResponsiveListItemRenderer.navigationEndpoint.watchEndpoint.videoId
    // - musicResponsiveListItemRenderer.flexColumns[0]...runs[0].navigationEndpoint.watchEndpoint.videoId
    r.pointer("/navigationEndpoint/watchEndpoint/videoId")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            r.pointer(
                "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/videoId",
            )
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
        })
}

fn parse_ytcfg_value(html: &str, key: &str) -> Option<String> {
    // We look for `"KEY":"value"` occurrences in the initial HTML ytcfg payload.
    let needle = format!("{key}\":\"");
    let idx = html.find(&needle)?;
    let start = idx + needle.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn scan_value<F>(v: &serde_json::Value, f: &mut F, out: &mut Vec<Track>)
where
    F: FnMut(&serde_json::Value) -> Option<Track>,
{
    if let Some(t) = f(v) {
        out.push(t);
        // keep scanning; duplicates are possible but tolerable for MVP
    }
    match v {
        serde_json::Value::Array(a) => {
            for x in a {
                scan_value(x, f, out);
            }
        }
        serde_json::Value::Object(o) => {
            for (_, x) in o {
                scan_value(x, f, out);
            }
        }
        _ => {}
    }
}

/// Extract continuation token from search response
fn extract_continuation_token(v: &serde_json::Value) -> Option<String> {
    // Continuation token can be found in various places:
    // - contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.continuations[0].nextContinuationData.continuation
    // - continuationContents.musicShelfContinuation.continuations[0].nextContinuationData.continuation

    let mut token: Option<String> = None;

    scan_for_continuation(v, &mut |node| {
        if let Some(cont) = node.get("nextContinuationData")
            .and_then(|c| c.get("continuation"))
            .and_then(|c| c.as_str())
        {
            token = Some(cont.to_string());
            return true;
        }
        if let Some(cont) = node.get("continuationEndpoint")
            .and_then(|c| c.get("continuationCommand"))
            .and_then(|c| c.get("token"))
            .and_then(|c| c.as_str())
        {
            token = Some(cont.to_string());
            return true;
        }
        false
    });

    token
}

/// Extract tracks from continuation response
fn extract_tracks_from_continuation(v: &serde_json::Value) -> Vec<Track> {
    // Continuation responses have tracks in continuationContents.musicShelfContinuation.contents
    let mut out = Vec::new();
    scan_value(v, &mut |node| {
        let r = node.get("musicResponsiveListItemRenderer")?;
        let video_id = extract_video_id_from_item(r)?;

        let title = r
            .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
            .and_then(|x| x.as_str())
            .unwrap_or("Unknown title")
            .to_string();

        let artists = r
            .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
            .and_then(|x| x.as_array())
            .map(|runs| {
                runs.iter()
                    .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                    .filter(|t| *t != " • ")
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Some(Track {
            video_id,
            title,
            artists,
            album: None,
            duration_seconds: None,
        })
    }, &mut out);
    out
}

/// Recursively scan for continuation tokens
fn scan_for_continuation<F>(v: &serde_json::Value, f: &mut F) -> bool
where
    F: FnMut(&serde_json::Value) -> bool,
{
    if f(v) {
        return true;
    }
    match v {
        serde_json::Value::Array(a) => {
            for x in a {
                if scan_for_continuation(x, f) {
                    return true;
                }
            }
        }
        serde_json::Value::Object(o) => {
            for (_, x) in o {
                if scan_for_continuation(x, f) {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

/// Extract playlists from search response (playlist-filtered)
fn extract_playlists_from_search(v: &serde_json::Value) -> Vec<Playlist> {
    let mut out = Vec::new();
    scan_playlists(v, &mut |node| {
        // In search results, playlists appear as musicResponsiveListItemRenderer
        if let Some(r) = node.get("musicResponsiveListItemRenderer") {
            // Check if this is a playlist (has browseId starting with VL or PL)
            if let Some(browse_id) = r
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(|x| x.as_str())
            {
                if browse_id.starts_with("VL") || browse_id.starts_with("PL") {
                    let playlist_id = browse_id.strip_prefix("VL").unwrap_or(browse_id).to_string();

                    let title = r
                        .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Unknown playlist")
                        .to_string();

                    // Get author from second flex column
                    let author = r
                        .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());

                    // Try to get track count from subtitle runs
                    let track_count = r
                        .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
                        .and_then(|x| x.as_array())
                        .and_then(|runs| {
                            runs.iter()
                                .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                                .find_map(|text| {
                                    // Look for patterns like "45 songs" or "45 tracks"
                                    if text.contains("song") || text.contains("track") {
                                        text.split_whitespace()
                                            .next()
                                            .and_then(|n| n.parse::<u32>().ok())
                                    } else {
                                        None
                                    }
                                })
                        });

                    return Some(Playlist {
                        id: playlist_id,
                        title,
                        author,
                        track_count,
                        thumbnail_url: None,
                    });
                }
            }
        }

        // Also check musicTwoRowItemRenderer
        if let Some(r) = node.get("musicTwoRowItemRenderer") {
            if let Some(browse_id) = r
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(|x| x.as_str())
            {
                if browse_id.starts_with("VL") || browse_id.starts_with("PL") {
                    let playlist_id = browse_id.strip_prefix("VL").unwrap_or(browse_id).to_string();

                    let title = r
                        .pointer("/title/runs/0/text")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Unknown playlist")
                        .to_string();

                    let author = r
                        .pointer("/subtitle/runs/0/text")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());

                    return Some(Playlist {
                        id: playlist_id,
                        title,
                        author,
                        track_count: None,
                        thumbnail_url: None,
                    });
                }
            }
        }

        None
    }, &mut out);
    out
}

/// Extract playlists from library browse response
fn extract_playlists(v: &serde_json::Value) -> Vec<Playlist> {
    let mut out = Vec::new();
    scan_playlists(v, &mut |node| {
        // Look for musicTwoRowItemRenderer which is used for playlists/albums in library
        if let Some(r) = node.get("musicTwoRowItemRenderer") {
            let playlist_id = r
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(|x| x.as_str())
                // Remove "VL" prefix if present
                .map(|s| s.strip_prefix("VL").unwrap_or(s).to_string())?;

            let title = r
                .pointer("/title/runs/0/text")
                .and_then(|x| x.as_str())
                .unwrap_or("Unknown playlist")
                .to_string();

            let author = r
                .pointer("/subtitle/runs/0/text")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());

            // Try to extract track count from subtitle
            let track_count = r
                .pointer("/subtitle/runs")
                .and_then(|x| x.as_array())
                .and_then(|runs| {
                    runs.iter()
                        .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                        .find_map(|text| {
                            // Look for patterns like "45 songs" or "45 tracks"
                            let parts: Vec<&str> = text.split_whitespace().collect();
                            if parts.len() >= 2
                                && (parts[1].contains("song") || parts[1].contains("track"))
                            {
                                parts[0].parse::<u32>().ok()
                            } else {
                                None
                            }
                        })
                });

            let thumbnail_url = r
                .pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails/0/url")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());

            return Some(Playlist {
                id: playlist_id,
                title,
                author,
                track_count,
                thumbnail_url,
            });
        }

        // Also try musicResponsiveListItemRenderer for some playlist views
        if let Some(r) = node.get("musicResponsiveListItemRenderer") {
            let playlist_id = r
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(|x| x.as_str())
                .map(|s| s.strip_prefix("VL").unwrap_or(s).to_string())?;

            let title = r
                .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
                .and_then(|x| x.as_str())
                .unwrap_or("Unknown playlist")
                .to_string();

            let author = r
                .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());

            return Some(Playlist {
                id: playlist_id,
                title,
                author,
                track_count: None,
                thumbnail_url: None,
            });
        }

        None
    }, &mut out);
    out
}

fn scan_playlists<F>(v: &serde_json::Value, f: &mut F, out: &mut Vec<Playlist>)
where
    F: FnMut(&serde_json::Value) -> Option<Playlist>,
{
    if let Some(p) = f(v) {
        out.push(p);
    }
    match v {
        serde_json::Value::Array(a) => {
            for x in a {
                scan_playlists(x, f, out);
            }
        }
        serde_json::Value::Object(o) => {
            for (_, x) in o {
                scan_playlists(x, f, out);
            }
        }
        _ => {}
    }
}

/// Extract search items (tracks and playlists) from search response
#[allow(dead_code)]
fn extract_search_items(v: &serde_json::Value) -> Vec<SearchItem> {
    let mut out = Vec::new();
    scan_search_items(v, &mut |node| {
        // Try to extract as a track (musicResponsiveListItemRenderer with video_id)
        if let Some(r) = node.get("musicResponsiveListItemRenderer") {
            // Check if this is a track (has videoId in overlay or playlistItemData)
            if let Some(video_id) = extract_video_id_from_item(r) {
                let title = r
                    .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
                    .and_then(|x| x.as_str())
                    .unwrap_or("Unknown title")
                    .to_string();

                let artists = r
                    .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
                    .and_then(|x| x.as_array())
                    .map(|runs| {
                        runs.iter()
                            .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                            .filter(|t| *t != " • " && *t != " & ")
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                return Some(SearchItem::Track(Track {
                    video_id,
                    title,
                    artists,
                    album: None,
                    duration_seconds: None,
                }));
            }

            // Check if this is a playlist (has browseId starting with VL or is a playlist type)
            if let Some(browse_id) = r
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(|x| x.as_str())
            {
                // Playlists have browseId starting with "VL" or have specific page type
                if browse_id.starts_with("VL") || browse_id.starts_with("PL") {
                    let playlist_id = browse_id.strip_prefix("VL").unwrap_or(browse_id).to_string();

                    let title = r
                        .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Unknown playlist")
                        .to_string();

                    let author = r
                        .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());

                    return Some(SearchItem::Playlist(Playlist {
                        id: playlist_id,
                        title,
                        author,
                        track_count: None,
                        thumbnail_url: None,
                    }));
                }
            }
        }

        // Try musicTwoRowItemRenderer (used for some playlist/album results)
        if let Some(r) = node.get("musicTwoRowItemRenderer") {
            if let Some(browse_id) = r
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(|x| x.as_str())
            {
                if browse_id.starts_with("VL") || browse_id.starts_with("PL") {
                    let playlist_id = browse_id.strip_prefix("VL").unwrap_or(browse_id).to_string();

                    let title = r
                        .pointer("/title/runs/0/text")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Unknown playlist")
                        .to_string();

                    let author = r
                        .pointer("/subtitle/runs/0/text")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());

                    return Some(SearchItem::Playlist(Playlist {
                        id: playlist_id,
                        title,
                        author,
                        track_count: None,
                        thumbnail_url: None,
                    }));
                }
            }
        }

        None
    }, &mut out);
    out
}

#[allow(dead_code)]
fn scan_search_items<F>(v: &serde_json::Value, f: &mut F, out: &mut Vec<SearchItem>)
where
    F: FnMut(&serde_json::Value) -> Option<SearchItem>,
{
    if let Some(item) = f(v) {
        out.push(item);
        // Don't recurse into this node since we already extracted an item
        return;
    }
    match v {
        serde_json::Value::Array(a) => {
            for x in a {
                scan_search_items(x, f, out);
            }
        }
        serde_json::Value::Object(o) => {
            for (_, x) in o {
                scan_search_items(x, f, out);
            }
        }
        _ => {}
    }
}

