use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct PlayEntry {
    pub playlist: String,
    pub played_at: u64,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn history_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".config/sono-palantir");
    fs::create_dir_all(&dir).ok();
    dir.join("history.json")
}

pub fn load() -> Vec<PlayEntry> {
    fs::read_to_string(history_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn record_play(playlist: &str) {
    let now = now_unix();
    let path = history_path();
    let mut entries: Vec<PlayEntry> = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    entries.push(PlayEntry { playlist: playlist.to_string(), played_at: now });
    let cutoff = now.saturating_sub(90 * 24 * 3600);
    entries.retain(|e| e.played_at > cutoff);
    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        fs::write(&path, json).ok();
    }
}

pub fn popularity_sort_from(playlists: &mut Vec<crate::api::Playlist>, entries: &[PlayEntry], now: u64) {
    let counts = play_counts_7d_from(entries, now);
    playlists.sort_by(|a, b| {
        let ca = counts.get(&a.alias).copied().unwrap_or(0);
        let cb = counts.get(&b.alias).copied().unwrap_or(0);
        cb.cmp(&ca).then(a.alias.cmp(&b.alias))
    });
}

pub fn popularity_sort(playlists: &mut Vec<crate::api::Playlist>) {
    let counts = play_counts_7d();
    playlists.sort_by(|a, b| {
        let ca = counts.get(&a.alias).copied().unwrap_or(0);
        let cb = counts.get(&b.alias).copied().unwrap_or(0);
        cb.cmp(&ca).then(a.alias.cmp(&b.alias))
    });
}

fn play_counts_7d() -> HashMap<String, usize> {
    play_counts_7d_from(&load(), now_unix())
}

fn play_counts_7d_from(entries: &[PlayEntry], now: u64) -> HashMap<String, usize> {
    let cutoff = now.saturating_sub(7 * 24 * 3600);
    let mut counts = HashMap::new();
    for e in entries {
        if e.played_at > cutoff {
            *counts.entry(e.playlist.clone()).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Playlist;

    #[test]
    fn test_play_counts_7d_from_counts_recent() {
        let now = now_unix();
        let entries = vec![
            PlayEntry { playlist: "altwave".to_string(), played_at: now - 3600 },
            PlayEntry { playlist: "altwave".to_string(), played_at: now - 3600 * 24 },
            PlayEntry { playlist: "altwave".to_string(), played_at: now - 3600 * 24 * 10 }, // >7d
            PlayEntry { playlist: "jazz".to_string(),    played_at: now - 3600 * 24 * 2 },
        ];
        let counts = play_counts_7d_from(&entries, now);
        assert_eq!(counts["altwave"], 2);
        assert_eq!(counts["jazz"], 1);
        assert!(!counts.contains_key("old"));
    }

    #[test]
    fn test_popularity_sort_from_orders_by_count_desc() {
        let now = now_unix();
        let entries = vec![
            PlayEntry { playlist: "altwave".to_string(), played_at: now - 3600 },
            PlayEntry { playlist: "altwave".to_string(), played_at: now - 7200 },
            PlayEntry { playlist: "altwave".to_string(), played_at: now - 10800 },
            PlayEntry { playlist: "altwave".to_string(), played_at: now - 14400 },
            PlayEntry { playlist: "altwave".to_string(), played_at: now - 18000 },
            PlayEntry { playlist: "jazz".to_string(),    played_at: now - 3600 },
            PlayEntry { playlist: "jazz".to_string(),    played_at: now - 7200 },
        ];
        let mut playlists = vec![
            Playlist { alias: "jazz".to_string(),    favorite_name: "Jazz".to_string() },
            Playlist { alias: "altwave".to_string(), favorite_name: "Alt Wave".to_string() },
        ];
        popularity_sort_from(&mut playlists, &entries, now);
        assert_eq!(playlists[0].alias, "altwave");
        assert_eq!(playlists[1].alias, "jazz");
    }
}
