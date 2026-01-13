mod app;
mod config;
mod input;
mod lyrics;
mod player;
mod storage;
mod tui;
mod ytm;

use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "kakariko", version, about = "YouTube Music TUI player (WIP)")]
struct Cli {
    /// Override config file path.
    #[arg(long)]
    config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the interactive TUI (default).
    Tui,
    /// Print Home tracks to stdout (headless).
    Home,
    /// Search tracks and print to stdout (headless).
    Search {
        query: String,
    },
    /// Browse a playlist and print to stdout (headless).
    Playlist {
        playlist_id: String,
    },
    /// Dump raw Search JSON to stdout (headless).
    SearchJson {
        query: String,
    },
    /// Dump raw Home JSON to stdout (headless).
    HomeJson,

    /// Configure authentication (so you don't need to export cookies manually).
    Auth {
        #[command(subcommand)]
        method: AuthCommand,
    },

    /// Audio output device management (mpv).
    Audio {
        #[command(subcommand)]
        cmd: AudioCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AuthCommand {
    /// Use yt-dlp `--cookies-from-browser` (recommended).
    Browser {
        /// Browser name: chrome, firefox, brave, etc.
        browser: String,
    },
    /// Configure cookies from Zen browser on macOS (auto-detect profile).
    Zen,
    /// Use a Netscape cookies file on disk.
    File {
        path: std::path::PathBuf,
    },
    /// Clear auth settings.
    Clear,
}

#[derive(Debug, Subcommand)]
enum AudioCommand {
    /// List mpv audio devices.
    List,
    /// Set mpv audio device (name as shown in list).
    Set { device: String },
    /// Clear mpv audio device override.
    Clear,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();
    let cfg = config::load(cli.config.as_deref()).context("load config")?;
    let cfg_path = match cli.config.clone() {
        Some(p) => p,
        None => config::default_config_path().context("default config path")?,
    };

    match cli.command.unwrap_or(Command::Tui) {
        Command::Tui => {
            let mut terminal = tui::TerminalGuard::enter().context("init terminal")?;
            let mut app = app::App::new(cfg, cfg_path)?;
            app.run(terminal.terminal_mut()).await?;
        }
        Command::Home => {
            let ytm = make_client(&cfg).await?;
            let tracks = ytm.browse_home_tracks().await?;
            print_tracks(&tracks);
        }
        Command::Search { query } => {
            let ytm = make_client(&cfg).await?;
            let tracks = ytm.search_tracks(&query).await?;
            print_tracks(&tracks);
        }
        Command::Playlist { playlist_id } => {
            let ytm = make_client(&cfg).await?;
            let tracks = ytm.browse_playlist_tracks(&playlist_id).await?;
            print_tracks(&tracks);
        }
        Command::SearchJson { query } => {
            let ytm = make_client(&cfg).await?;
            let v = ytm.search_raw(&query).await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Command::HomeJson => {
            let ytm = make_client(&cfg).await?;
            let v = ytm.browse_home_raw().await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Command::Auth { method } => {
            let mut cfg = cfg;
            match method {
                AuthCommand::Browser { browser } => {
                    cfg.ytm.cookies_from_browser = Some(browser);
                    cfg.ytm.cookies = None;
                }
                AuthCommand::Zen => {
                    let base_dirs = directories::BaseDirs::new().context("BaseDirs unavailable")?;
                    let base = base_dirs
                        .home_dir()
                        .join("Library")
                        .join("Application Support")
                        .join("zen");
                    // Try to auto-detect the default profile from profiles.ini.
                    let ini_path = base.join("profiles.ini");
                    let raw = std::fs::read_to_string(&ini_path)
                        .with_context(|| format!("read {}", ini_path.display()))?;
                    let default_rel = raw
                        .lines()
                        .find_map(|l| l.strip_prefix("Default="))
                        .map(|s| s.trim().to_string())
                        .context("no Default=... in zen profiles.ini")?;
                    let profile_dir = base.join(default_rel);
                    cfg.ytm.cookies_from_browser =
                        Some(format!("firefox:{}", profile_dir.display()));
                    cfg.ytm.cookies = None;
                }
                AuthCommand::File { path } => {
                    cfg.ytm.cookies = Some(path);
                    cfg.ytm.cookies_from_browser = None;
                }
                AuthCommand::Clear => {
                    cfg.ytm.cookies = None;
                    cfg.ytm.cookies_from_browser = None;
                }
            }
            config::save(&cfg, cli.config.as_deref()).context("save config")?;
            println!("Updated config auth settings.");
        }
        Command::Audio { cmd } => match cmd {
            AudioCommand::List => {
                let out = tokio::process::Command::new("mpv")
                    .args(["--audio-device=help", "--no-video", "--idle=no"])
                    .output()
                    .await
                    .context("run mpv --audio-device=help")?;
                // mpv prints help to stdout.
                print!("{}", String::from_utf8_lossy(&out.stdout));
                eprint!("{}", String::from_utf8_lossy(&out.stderr));
            }
            AudioCommand::Set { device } => {
                let mut cfg = cfg;
                cfg.player.audio_device = Some(device);
                config::save(&cfg, cli.config.as_deref()).context("save config")?;
                println!("Updated audio device in config.");
            }
            AudioCommand::Clear => {
                let mut cfg = cfg;
                cfg.player.audio_device = None;
                config::save(&cfg, cli.config.as_deref()).context("save config")?;
                println!("Cleared audio device override.");
            }
        },
    }

    Ok(())
}

async fn make_client(cfg: &config::Config) -> anyhow::Result<ytm::api::YtmClient> {
    let auth = match cfg.ytm.cookies.as_deref() {
        Some(p) if p.exists() => Some(ytm::auth::load_netscape_cookies(p)?),
        _ => None,
    };
    ytm::api::YtmClient::new(auth)
}

fn print_tracks(tracks: &[ytm::models::Track]) {
    for (i, t) in tracks.iter().enumerate() {
        let artists = if t.artists.is_empty() {
            "".to_string()
        } else {
            format!(" — {}", t.artists.join(", "))
        };
        println!(
            "{:02}. {}{}  (video_id={})",
            i + 1,
            t.title,
            artists,
            t.video_id
        );
    }
}
