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
    let mut entries = load();
    entries.push(PlayEntry {
        playlist: playlist.to_string(),
        played_at: now_unix(),
    });
    let cutoff = now_unix().saturating_sub(90 * 24 * 3600);
    entries.retain(|e| e.played_at > cutoff);
    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        fs::write(history_path(), json).ok();
    }
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
    let cutoff = now_unix().saturating_sub(7 * 24 * 3600);
    let mut counts = HashMap::new();
    for e in load() {
        if e.played_at > cutoff {
            *counts.entry(e.playlist).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_entry(playlist: &str, secs_ago: u64) -> PlayEntry {
        PlayEntry {
            playlist: playlist.to_string(),
            played_at: now_unix().saturating_sub(secs_ago),
        }
    }

    #[test]
    fn test_play_counts_7d_counts_recent() {
        let entries = vec![
            fake_entry("altwave", 3600),
            fake_entry("altwave", 3600 * 24),
            fake_entry("altwave", 3600 * 24 * 10), // outside 7d
            fake_entry("jazz", 3600 * 24 * 2),
        ];
        let cutoff = now_unix().saturating_sub(7 * 24 * 3600);
        let mut counts: HashMap<String, usize> = HashMap::new();
        for e in &entries {
            if e.played_at > cutoff {
                *counts.entry(e.playlist.clone()).or_insert(0) += 1;
            }
        }
        assert_eq!(counts["altwave"], 2);
        assert_eq!(counts["jazz"], 1);
        assert!(!counts.contains_key("old"));
    }

    #[test]
    fn test_popularity_sort_orders_by_count_desc() {
        use crate::api::Playlist;
        let mut playlists = vec![
            Playlist { alias: "jazz".to_string(), favorite_name: "Jazz".to_string() },
            Playlist { alias: "altwave".to_string(), favorite_name: "Alt Wave".to_string() },
        ];
        let mut counts: HashMap<String, usize> = HashMap::new();
        counts.insert("altwave".to_string(), 5);
        counts.insert("jazz".to_string(), 2);
        playlists.sort_by(|a, b| {
            let ca = counts.get(&a.alias).copied().unwrap_or(0);
            let cb = counts.get(&b.alias).copied().unwrap_or(0);
            cb.cmp(&ca).then(a.alias.cmp(&b.alias))
        });
        assert_eq!(playlists[0].alias, "altwave");
        assert_eq!(playlists[1].alias, "jazz");
    }
}
