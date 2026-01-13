//! LRC format parser
//!
//! Parses synchronized lyrics in LRC format:
//! [mm:ss.xx] Lyrics line here
//!
//! Example:
//! [00:12.34] Hello world
//! [00:15.00] Another line

/// A single line of lyrics with timestamp
#[derive(Debug, Clone)]
pub struct LrcLine {
    /// Timestamp in milliseconds from start
    pub time_ms: u64,
    /// The lyrics text
    pub text: String,
}

impl LrcLine {
    pub fn new(time_ms: u64, text: String) -> Self {
        Self { time_ms, text }
    }
}

/// Parsed lyrics with metadata
#[derive(Debug, Clone)]
pub struct ParsedLyrics {
    /// Individual lyrics lines
    pub lines: Vec<LrcLine>,
    /// Whether the lyrics are synchronized
    pub synced: bool,
}

impl ParsedLyrics {
    /// Parse LRC formatted lyrics
    pub fn parse(content: &str, synced: bool) -> Self {
        let mut lines = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Skip metadata tags like [ti:Title]
            if Self::parse_metadata(line).is_some() {
                continue;
            }

            // Try to parse timestamped line like [00:12.34]Lyrics
            if synced
                && let Some(parsed) = Self::parse_timed_line(line) {
                    lines.extend(parsed);
                    continue;
                }

            // Plain text line (no timestamp)
            if !line.starts_with('[') {
                lines.push(LrcLine::new(0, line.to_string()));
            }
        }

        // Sort by timestamp
        lines.sort_by_key(|l| l.time_ms);

        Self { lines, synced }
    }

    /// Parse metadata tag like [ti:Title]
    fn parse_metadata(line: &str) -> Option<(String, String)> {
        if !line.starts_with('[') || !line.contains(':') {
            return None;
        }

        let end = line.find(']')?;
        let tag_content = &line[1..end];

        // Check if it looks like a metadata tag (not a timestamp)
        let colon_pos = tag_content.find(':')?;
        let tag = &tag_content[..colon_pos];

        // Metadata tags are typically 2-3 chars
        if tag.len() <= 3 && tag.chars().all(|c| c.is_ascii_alphabetic()) {
            let value = tag_content[colon_pos + 1..].trim().to_string();
            return Some((tag.to_string(), value));
        }

        None
    }

    /// Parse a timed line like [00:12.34]Lyrics or [00:12.34][00:15.00]Lyrics
    fn parse_timed_line(line: &str) -> Option<Vec<LrcLine>> {
        let mut timestamps = Vec::new();
        let mut pos = 0;

        // Extract all timestamps at the beginning
        while pos < line.len() && line[pos..].starts_with('[') {
            if let Some(end) = line[pos..].find(']') {
                let timestamp_str = &line[pos + 1..pos + end];
                if let Some(ms) = Self::parse_timestamp(timestamp_str) {
                    timestamps.push(ms);
                    pos += end + 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if timestamps.is_empty() {
            return None;
        }

        // The rest is the lyrics text
        let text = line[pos..].trim().to_string();

        // Create a line for each timestamp
        let lines = timestamps
            .into_iter()
            .map(|ts| LrcLine::new(ts, text.clone()))
            .collect();

        Some(lines)
    }

    /// Parse timestamp string like "00:12.34" or "00:12:34" to milliseconds
    fn parse_timestamp(s: &str) -> Option<u64> {
        // Format: mm:ss.xx or mm:ss:xx or mm:ss
        let parts: Vec<&str> = s.split([':', '.']).collect();

        match parts.len() {
            2 => {
                // mm:ss
                let min: u64 = parts[0].parse().ok()?;
                let sec: u64 = parts[1].parse().ok()?;
                Some(min * 60 * 1000 + sec * 1000)
            }
            3 => {
                // mm:ss.xx or mm:ss:xx
                let min: u64 = parts[0].parse().ok()?;
                let sec: u64 = parts[1].parse().ok()?;
                let ms_str = parts[2];
                // Handle both "34" (centiseconds) and "340" (milliseconds)
                let ms: u64 = match ms_str.len() {
                    1 => ms_str.parse::<u64>().ok()? * 100,
                    2 => ms_str.parse::<u64>().ok()? * 10,
                    3 => ms_str.parse().ok()?,
                    _ => return None,
                };
                Some(min * 60 * 1000 + sec * 1000 + ms)
            }
            _ => None,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp() {
        assert_eq!(ParsedLyrics::parse_timestamp("00:12"), Some(12000));
        assert_eq!(ParsedLyrics::parse_timestamp("01:30"), Some(90000));
        assert_eq!(ParsedLyrics::parse_timestamp("00:12.34"), Some(12340));
        assert_eq!(ParsedLyrics::parse_timestamp("00:12.340"), Some(12340));
        assert_eq!(ParsedLyrics::parse_timestamp("00:12:34"), Some(12340));
    }

    #[test]
    fn test_parse_lrc() {
        let lrc = r#"
[ti:Test Song]
[ar:Test Artist]
[00:12.34]First line
[00:15.00]Second line
"#;
        let parsed = ParsedLyrics::parse(lrc, true);
        assert_eq!(parsed.lines.len(), 2);
        assert_eq!(parsed.lines[0].time_ms, 12340);
        assert_eq!(parsed.lines[0].text, "First line");
    }
}
