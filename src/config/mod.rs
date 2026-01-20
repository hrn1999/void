use anyhow::Context;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub mod defaults;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: Theme,
    pub input: InputConfig,
    pub paths: PathsConfig,
    pub ytm: YtmConfig,
    pub player: PlayerConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InputConfig {
    pub mouse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct YtmConfig {
    /// Path to a Netscape cookie file (yt-dlp compatible).
    pub cookies: Option<PathBuf>,
    /// Use yt-dlp `--cookies-from-browser` (e.g. "chrome", "firefox", "brave").
    pub cookies_from_browser: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerConfig {
    /// mpv audio device name (see `mpv --audio-device=help`)
    pub audio_device: Option<String>,
    /// Volume level (0-100)
    pub volume: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct UiConfig {
    /// Last visited screen (restored on startup)
    pub last_screen: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let proj = ProjectDirs::from("dev", "void", "void");
        let data_dir = proj
            .as_ref()
            .map(|p| p.data_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("void"));

        Self {
            theme: Theme {
                name: "nostalgic".to_string(),
            },
            input: InputConfig { mouse: true },
            paths: PathsConfig { data_dir },
            ytm: YtmConfig {
                cookies: None,
                cookies_from_browser: None,
            },
            player: PlayerConfig {
                audio_device: None,
                volume: 80,
            },
            ui: UiConfig { last_screen: None },
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "nostalgic".to_string(),
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self { mouse: true }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        let proj = ProjectDirs::from("dev", "void", "void");
        let data_dir = proj
            .as_ref()
            .map(|p| p.data_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("void"));
        Self { data_dir }
    }
}


impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            audio_device: None,
            volume: 80,
        }
    }
}


pub fn save(cfg: &Config, override_path: Option<&Path>) -> anyhow::Result<()> {
    let path = match override_path {
        Some(p) => p.to_path_buf(),
        None => default_config_path()?,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    let raw = toml::to_string_pretty(cfg).context("serialize config")?;
    fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn default_config_path() -> anyhow::Result<PathBuf> {
    let proj =
        ProjectDirs::from("dev", "void", "void").context("ProjectDirs unavailable")?;
    Ok(proj.config_dir().join("config.toml"))
}

pub fn load(override_path: Option<&Path>) -> anyhow::Result<Config> {
    let path = match override_path {
        Some(p) => p.to_path_buf(),
        None => default_config_path()?,
    };

    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
        }
        let cfg = defaults::defaults();
        let raw = toml::to_string_pretty(&cfg).context("serialize default config")?;
        fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        return Ok(cfg);
    }

    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let cfg = toml::from_str::<Config>(&raw).with_context(|| format!("parse {}", path.display()))?;
    Ok(cfg)
}

