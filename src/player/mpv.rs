use crate::app::events::{Event, PlayerEvent};
use anyhow::Context;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    process::{Child, Command},
    sync::mpsc,
};

#[derive(Debug)]
pub struct MpvHandle {
    child: Child,
    socket_path: PathBuf,
    writer: tokio::sync::Mutex<tokio::io::WriteHalf<UnixStream>>,
    request_id: AtomicU64,
}

impl MpvHandle {
    pub async fn spawn(
        event_tx: mpsc::Sender<Event>,
        audio_device: Option<&str>,
        log_file: Option<&std::path::Path>,
    ) -> anyhow::Result<Self> {
        let socket_path = std::env::temp_dir().join("kakariko-mpv.sock");
        let _ = std::fs::remove_file(&socket_path);

        let mut cmd = Command::new("mpv");
        cmd.args([
            "--no-video",
            "--idle=yes",
            "--input-terminal=no",
            // keep quiet, but we'll request log messages via IPC so we can show errors in UI
            "--really-quiet",
            // Audio quality optimizations
            "--audio-channels=stereo",
            "--audio-samplerate=48000",
            "--audio-format=s16",
        ]);
        if let Some(dev) = audio_device {
            cmd.arg(format!("--audio-device={dev}"));
        }
        if let Some(p) = log_file {
            cmd.arg(format!("--log-file={}", p.display()));
        }
        let child = cmd
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("spawn mpv")?;

        // Connect (mpv creates the socket shortly after starting).
        let stream = connect_with_retry(&socket_path).await?;
        let (reader, writer) = tokio::io::split(stream);

        // Pump mpv JSON events -> app events.
        tokio::spawn(read_events_loop(reader, event_tx.clone()));

        let this = Self {
            child,
            socket_path,
            writer: tokio::sync::Mutex::new(writer),
            request_id: AtomicU64::new(1),
        };

        // Ask mpv to send log-message events so we can surface load failures.
        this.command(json!({"command":["request_log_messages", "warn"]}))
            .await?;

        // Observe key properties.
        this.command(json!({"command":["observe_property", 1, "time-pos"]}))
            .await?;
        this.command(json!({"command":["observe_property", 2, "duration"]}))
            .await?;
        this.command(json!({"command":["observe_property", 3, "pause"]}))
            .await?;
        this.command(json!({"command":["observe_property", 4, "eof-reached"]}))
            .await?;

        Ok(this)
    }

    pub async fn load_url(&self, url: &str) -> anyhow::Result<()> {
        self.command(json!({"command":["loadfile", url, "replace"]})).await
    }

    pub async fn toggle_pause(&self) -> anyhow::Result<()> {
        self.command(json!({"command":["cycle", "pause"]})).await
    }

    pub async fn seek_relative(&self, seconds: f64) -> anyhow::Result<()> {
        self.command(json!({"command":["seek", seconds, "relative"]}))
            .await
    }

    pub async fn set_volume(&self, volume_0_100: u8) -> anyhow::Result<()> {
        self.command(json!({"command":["set_property", "volume", volume_0_100]}))
            .await
    }

    async fn command(&self, mut v: serde_json::Value) -> anyhow::Result<()> {
        // Tag requests so we can get structured errors back on the IPC stream.
        if v.get("request_id").is_none() {
            let id = self.request_id.fetch_add(1, Ordering::Relaxed);
            if let serde_json::Value::Object(ref mut o) = v {
                o.insert("request_id".to_string(), serde_json::Value::from(id));
            }
        }
        let mut w = self.writer.lock().await;
        let mut line = serde_json::to_vec(&v).context("encode mpv json")?;
        line.push(b'\n');
        w.write_all(&line).await.context("write mpv ipc")?;
        w.flush().await.context("flush mpv ipc")?;
        Ok(())
    }
}

impl Drop for MpvHandle {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

async fn connect_with_retry(path: &PathBuf) -> anyhow::Result<UnixStream> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match UnixStream::connect(path).await {
            Ok(s) => return Ok(s),
            Err(e) => {
                if tokio::time::Instant::now() > deadline {
                    return Err(e).with_context(|| format!("connect to mpv ipc {}", path.display()));
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
}

async fn read_events_loop(reader: tokio::io::ReadHalf<UnixStream>, event_tx: mpsc::Sender<Event>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            // mpv command replies: {"request_id":..., "error":"..."}
            if let (Some(_rid), Some(err)) = (v.get("request_id"), v.get("error"))
                && let Some(err_s) = err.as_str()
                    && err_s != "success" {
                        let _ = event_tx
                            .send(Event::Player(PlayerEvent::Error(format!(
                                "mpv ipc error: {err_s}"
                            ))))
                            .await;
                    }
            if let Some(pe) = map_mpv_event(&v) {
                let _ = event_tx.send(Event::Player(pe)).await;
            }
        }
    }
}

fn map_mpv_event(v: &serde_json::Value) -> Option<PlayerEvent> {
    // We mostly care about property-change events.
    match v.get("event")?.as_str()? {
        "property-change" => {
            let name = v.get("name")?.as_str()?;
            match name {
                "time-pos" => Some(PlayerEvent::Position {
                    seconds: v.get("data")?.as_f64().unwrap_or(0.0),
                }),
                "duration" => Some(PlayerEvent::Duration {
                    seconds: v.get("data")?.as_f64().unwrap_or(0.0),
                }),
                "pause" => {
                    let paused = v.get("data")?.as_bool().unwrap_or(false);
                    Some(if paused { PlayerEvent::Paused } else { PlayerEvent::Started })
                }
                "eof-reached" => {
                    let eof = v.get("data")?.as_bool().unwrap_or(false);
                    if eof { Some(PlayerEvent::Ended) } else { None }
                }
                _ => None,
            }
        }
        "end-file" => {
            // When mpv fails to play the stream, end-file comes with reason=error and an "error" string.
            let reason = v.get("reason").and_then(|x| x.as_str()).unwrap_or("");
            if reason == "error" {
                let err = v.get("error").and_then(|x| x.as_str()).unwrap_or("unknown");
                Some(PlayerEvent::Error(format!("mpv end-file error: {err}")))
            } else {
                Some(PlayerEvent::Ended)
            }
        }
        "log-message" => {
            let level = v.get("level")?.as_str().unwrap_or("info");
            let text = v.get("text")?.as_str().unwrap_or("").trim();
            if (level == "warn" || level == "error") && !text.is_empty() {
                Some(PlayerEvent::Error(format!("mpv {level}: {text}")))
            } else {
                None
            }
        }
        _ => None,
    }
}

