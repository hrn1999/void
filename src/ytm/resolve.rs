use anyhow::Context;
use std::path::Path;
use tokio::process::Command;

pub async fn resolve_audio_url(
    video_id: &str,
    cookies_netscape: Option<&Path>,
    cookies_from_browser: Option<&str>,
) -> anyhow::Result<String> {
    let mut cmd = Command::new("yt-dlp");
    cmd.args(["-f", "bestaudio", "--get-url", "--no-playlist"]);

    // Prefer browser cookies when configured (no manual export needed).
    if let Some(browser) = cookies_from_browser {
        cmd.arg("--cookies-from-browser").arg(browser);
    } else if let Some(cookies) = cookies_netscape {
        cmd.arg("--cookies").arg(cookies);
    }
    cmd.arg(format!(
        "https://music.youtube.com/watch?v={video_id}"
    ));

    let out = cmd.output().await.context("run yt-dlp")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("yt-dlp failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8(out.stdout).context("decode yt-dlp stdout")?;
    let url = stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .context("yt-dlp returned empty url")?;
    Ok(url.to_string())
}


