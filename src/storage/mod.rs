use anyhow::Context;
use rusqlite::{params, Connection};
use std::path::Path;

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }

        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        let s = Self { conn };
        s.init_schema()?;
        Ok(s)
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        self.conn
            .execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS tracks (
  video_id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  artists_json TEXT NOT NULL,
  album TEXT,
  duration_seconds INTEGER,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS last_searches (
  query TEXT PRIMARY KEY,
  results_json TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS stream_cache (
  video_id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS play_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  video_id TEXT NOT NULL,
  title TEXT NOT NULL,
  artists_json TEXT NOT NULL,
  album TEXT,
  duration_seconds INTEGER,
  played_at INTEGER NOT NULL,
  duration_listened INTEGER,
  completed INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_history_played_at ON play_history(played_at DESC);
CREATE INDEX IF NOT EXISTS idx_history_video_id ON play_history(video_id);

CREATE TABLE IF NOT EXISTS lyrics_cache (
  video_id TEXT PRIMARY KEY,
  lrc_content TEXT,
  synced INTEGER DEFAULT 0,
  fetched_at INTEGER NOT NULL
);
"#,
            )
            .context("init schema")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn cache_search(&self, query: &str, results_json: &str, now_unix: i64) -> anyhow::Result<()> {
        self.conn
            .execute(
                r#"
INSERT INTO last_searches(query, results_json, updated_at)
VALUES(?1, ?2, ?3)
ON CONFLICT(query) DO UPDATE SET
  results_json=excluded.results_json,
  updated_at=excluded.updated_at
"#,
                params![query, results_json, now_unix],
            )
            .context("cache search")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_cached_search(&self, query: &str) -> anyhow::Result<Option<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT results_json, updated_at FROM last_searches WHERE query=?1")
            .context("prepare cached search")?;
        let mut rows = stmt.query(params![query]).context("query cached search")?;
        if let Some(row) = rows.next().context("read cached search row")? {
            let json: String = row.get(0)?;
            let ts: i64 = row.get(1)?;
            Ok(Some((json, ts)))
        } else {
            Ok(None)
        }
    }

    pub fn get_stream_url(&self, video_id: &str, now_unix: i64) -> anyhow::Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, expires_at FROM stream_cache WHERE video_id=?1")
            .context("prepare stream cache")?;
        let mut rows = stmt.query(params![video_id]).context("query stream cache")?;
        if let Some(row) = rows.next().context("read stream cache row")? {
            let url: String = row.get(0)?;
            let exp: i64 = row.get(1)?;
            if exp > now_unix {
                Ok(Some(url))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn cache_stream_url(
        &self,
        video_id: &str,
        url: &str,
        expires_at: i64,
        now_unix: i64,
    ) -> anyhow::Result<()> {
        self.conn
            .execute(
                r#"
INSERT INTO stream_cache(video_id, url, expires_at, updated_at)
VALUES(?1, ?2, ?3, ?4)
ON CONFLICT(video_id) DO UPDATE SET
  url=excluded.url,
  expires_at=excluded.expires_at,
  updated_at=excluded.updated_at
"#,
                params![video_id, url, expires_at, now_unix],
            )
            .context("cache stream url")?;
        Ok(())
    }

    /// Add a track to play history
    pub fn add_to_history(
        &self,
        track: &crate::ytm::models::Track,
        played_at: i64,
    ) -> anyhow::Result<()> {
        let artists_json = serde_json::to_string(&track.artists).unwrap_or_else(|_| "[]".into());
        self.conn
            .execute(
                r#"
INSERT INTO play_history(video_id, title, artists_json, album, duration_seconds, played_at)
VALUES(?1, ?2, ?3, ?4, ?5, ?6)
"#,
                params![
                    track.video_id,
                    track.title,
                    artists_json,
                    track.album,
                    track.duration_seconds,
                    played_at
                ],
            )
            .context("add to history")?;
        Ok(())
    }

    /// Get play history (most recent first, unique tracks only)
    pub fn get_history(&self, limit: usize) -> anyhow::Result<Vec<crate::ytm::models::Track>> {
        let mut stmt = self.conn.prepare(
            r#"
SELECT video_id, title, artists_json, album, duration_seconds, MAX(played_at) as last_played
FROM play_history
GROUP BY video_id
ORDER BY last_played DESC
LIMIT ?1
"#,
        )?;

        let tracks = stmt
            .query_map(params![limit as i64], |row| {
                let video_id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let artists_json: String = row.get(2)?;
                let album: Option<String> = row.get(3)?;
                let duration_seconds: Option<u32> = row.get(4)?;

                let artists: Vec<String> =
                    serde_json::from_str(&artists_json).unwrap_or_default();

                Ok(crate::ytm::models::Track {
                    video_id,
                    title,
                    artists,
                    album,
                    duration_seconds,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(tracks)
    }

    /// Cache lyrics for a track
    pub fn cache_lyrics(
        &self,
        video_id: &str,
        lrc_content: &str,
        synced: bool,
        now_unix: i64,
    ) -> anyhow::Result<()> {
        self.conn
            .execute(
                r#"
INSERT INTO lyrics_cache(video_id, lrc_content, synced, fetched_at)
VALUES(?1, ?2, ?3, ?4)
ON CONFLICT(video_id) DO UPDATE SET
  lrc_content=excluded.lrc_content,
  synced=excluded.synced,
  fetched_at=excluded.fetched_at
"#,
                params![video_id, lrc_content, synced as i32, now_unix],
            )
            .context("cache lyrics")?;
        Ok(())
    }

    /// Get cached lyrics
    pub fn get_lyrics(&self, video_id: &str) -> anyhow::Result<Option<(String, bool)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT lrc_content, synced FROM lyrics_cache WHERE video_id=?1")?;
        let mut rows = stmt.query(params![video_id])?;
        if let Some(row) = rows.next()? {
            let content: String = row.get(0)?;
            let synced: i32 = row.get(1)?;
            Ok(Some((content, synced != 0)))
        } else {
            Ok(None)
        }
    }
}


