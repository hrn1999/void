use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub domain: String,
    pub path: String,
    pub name: String,
    pub value: String,
    pub secure: bool,
    pub expires_utc: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AuthState {
    #[allow(dead_code)]
    cookies: Vec<Cookie>,
    pub cookie_header: String,
    pub sapisid: Option<String>,
}

pub fn load_netscape_cookies(path: &Path) -> anyhow::Result<AuthState> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut cookies = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Netscape format: domain \t flag \t path \t secure \t expiration \t name \t value
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 7 {
            continue;
        }

        let domain = parts[0].to_string();
        let path = parts[2].to_string();
        let secure = parts[3].eq_ignore_ascii_case("TRUE");
        let expires_utc = parts[4].parse::<i64>().ok();
        let name = parts[5].to_string();
        let value = parts[6].to_string();

        cookies.push(Cookie {
            domain,
            path,
            name,
            value,
            secure,
            expires_utc,
        });
    }

    let cookie_header = cookies
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ");

    // For signed requests, YouTube uses SAPISID (sometimes __Secure-3PAPISID works too).
    let sapisid = cookies
        .iter()
        .find(|c| c.name == "SAPISID")
        .map(|c| c.value.clone())
        .or_else(|| {
            cookies
                .iter()
                .find(|c| c.name == "__Secure-3PAPISID")
                .map(|c| c.value.clone())
        });

    Ok(AuthState {
        cookies,
        cookie_header,
        sapisid,
    })
}


