use crate::api::{Speaker, Playlist};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Speakers,
    Playlists,
    NowPlaying,
}

pub struct App {
    pub speakers: Vec<Speaker>,
    pub playlists: Vec<Playlist>,
    pub active_panel: Panel,
    pub speaker_index: usize,
    pub playlist_index: usize,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub volume_input: Option<String>,
    pub command_input: Option<String>,
    pub sleep_until: Option<std::time::Instant>,
    pub status_until: Option<std::time::Instant>,
    pub help_open: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            speakers: vec![],
            playlists: vec![],
            active_panel: Panel::Speakers,
            speaker_index: 0,
            playlist_index: 0,
            should_quit: false,
            status_message: None,
            volume_input: None,
            command_input: None,
            sleep_until: None,
            status_until: None,
            help_open: false,
        }
    }

    pub fn selected_speaker(&self) -> Option<&Speaker> {
        self.speakers.get(self.speaker_index)
    }

    pub fn selected_playlist(&self) -> Option<&Playlist> {
        self.playlists.get(self.playlist_index)
    }

    pub fn speaker_id(&self) -> Option<String> {
        self.selected_speaker().map(|s| {
            s.alias.clone().unwrap_or_else(|| s.name.clone())
        })
    }

    pub fn next_in_list(&mut self) {
        match self.active_panel {
            Panel::Speakers => {
                if !self.speakers.is_empty() {
                    self.speaker_index = (self.speaker_index + 1) % self.speakers.len();
                }
            }
            Panel::Playlists => {
                if !self.playlists.is_empty() {
                    self.playlist_index = (self.playlist_index + 1) % self.playlists.len();
                }
            }
            _ => {}
        }
    }

    pub fn prev_in_list(&mut self) {
        match self.active_panel {
            Panel::Speakers => {
                if !self.speakers.is_empty() {
                    self.speaker_index = self.speaker_index
                        .checked_sub(1)
                        .unwrap_or(self.speakers.len() - 1);
                }
            }
            Panel::Playlists => {
                if !self.playlists.is_empty() {
                    self.playlist_index = self.playlist_index
                        .checked_sub(1)
                        .unwrap_or(self.playlists.len() - 1);
                }
            }
            _ => {}
        }
    }

    pub fn cycle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Speakers => Panel::Playlists,
            Panel::Playlists => Panel::NowPlaying,
            Panel::NowPlaying => Panel::Speakers,
        };
    }

    pub fn set_status(&mut self, msg: impl Into<String>, secs: u64) {
        self.status_message = Some(msg.into());
        self.status_until = Some(
            std::time::Instant::now() + std::time::Duration::from_secs(secs)
        );
    }

    pub fn active_status(&self) -> String {
        // Sleep countdown takes lowest priority — shown only when no timed message
        if let Some(until) = self.status_until {
            if until > std::time::Instant::now() {
                return self.status_message.clone().unwrap_or_default();
            }
        }
        if let Some(sleep_until) = self.sleep_until {
            let now = std::time::Instant::now();
            if let Some(remaining) = sleep_until.checked_duration_since(now) {
                let secs = remaining.as_secs();
                return format!("Sleep: {}:{:02} remaining", secs / 60, secs % 60);
            }
        }
        String::new()
    }

    pub fn is_grouped(&self) -> bool {
        // A speaker is a group follower when its coordinator differs from its own name.
        // If any follower exists, speakers are grouped.
        self.speakers.iter().any(|s| {
            s.group_coordinator
                .as_deref()
                .map(|coord| coord != s.name)
                .unwrap_or(false)
        })
    }

    /// Returns all speakers whose coordinator is `coordinator_name`.
    pub fn group_members_of<'a>(&'a self, coordinator_name: &str) -> Vec<&'a Speaker> {
        self.speakers.iter().filter(|s| {
            s.group_coordinator.as_deref() == Some(coordinator_name)
        }).collect()
    }

    /// Returns speakers with no group_coordinator (truly ungrouped/solo).
    pub fn solo_speakers(&self) -> Vec<&Speaker> {
        self.speakers.iter().filter(|s| s.group_coordinator.is_none()).collect()
    }

    /// Returns coordinator speakers (group_coordinator == their own name).
    pub fn coordinators(&self) -> Vec<&Speaker> {
        self.speakers.iter().filter(|s| {
            s.group_coordinator.as_deref() == Some(s.name.as_str())
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Speaker;

    fn make_speaker(name: &str, coordinator: Option<&str>) -> Speaker {
        Speaker {
            name: name.to_string(),
            alias: None,
            ip: "0.0.0.0".to_string(),
            volume: 25,
            muted: false,
            state: "PLAYING".to_string(),
            group_coordinator: coordinator.map(|s| s.to_string()),
            track: None,
        }
    }

    #[test]
    fn test_is_grouped_when_follower_present() {
        let mut app = App::new();
        app.speakers = vec![
            make_speaker("Family Room", Some("Family Room")),
            make_speaker("cthulhu", Some("Family Room")),
        ];
        assert!(app.is_grouped());
    }

    #[test]
    fn test_is_not_grouped_when_all_self_coordinating() {
        let mut app = App::new();
        app.speakers = vec![
            make_speaker("Family Room", Some("Family Room")),
            make_speaker("cthulhu", Some("cthulhu")),
        ];
        assert!(!app.is_grouped());
    }

    #[test]
    fn test_is_not_grouped_when_coordinators_null() {
        let mut app = App::new();
        app.speakers = vec![
            make_speaker("Family Room", None),
            make_speaker("cthulhu", None),
        ];
        assert!(!app.is_grouped());
    }

    #[test]
    fn test_volume_input_starts_none() {
        let app = App::new();
        assert!(app.volume_input.is_none());
    }

    #[test]
    fn test_volume_input_can_be_set() {
        let mut app = App::new();
        app.volume_input = Some(String::from("42"));
        assert_eq!(app.volume_input.as_deref(), Some("42"));
    }

    #[test]
    fn test_active_status_returns_empty_when_nothing_set() {
        let app = App::new();
        assert_eq!(app.active_status(), "");
    }

    #[test]
    fn test_set_status_returns_message_immediately() {
        let mut app = App::new();
        app.set_status("The gates of Moria are sealed.", 5);
        assert_eq!(app.active_status(), "The gates of Moria are sealed.");
    }

    #[test]
    fn test_active_status_returns_empty_when_expired() {
        let mut app = App::new();
        // Set a status that already expired
        app.status_message = Some("old message".to_string());
        app.status_until = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        assert_eq!(app.active_status(), "");
    }

    #[test]
    fn test_active_status_returns_sleep_countdown() {
        let mut app = App::new();
        app.sleep_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(90));
        let status = app.active_status();
        assert!(status.starts_with("Sleep: 1:"), "Expected 'Sleep: 1:xx remaining', got: {}", status);
    }

    #[test]
    fn test_coordinators_returns_only_coordinators() {
        let mut app = App::new();
        app.speakers = vec![
            make_speaker("cthulhu", Some("cthulhu")),
            make_speaker("family", Some("cthulhu")),
            make_speaker("hermit", None),
        ];
        let coords = app.coordinators();
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0].name, "cthulhu");
    }

    #[test]
    fn test_solo_speakers_returns_ungrouped() {
        let mut app = App::new();
        app.speakers = vec![
            make_speaker("cthulhu", Some("cthulhu")),
            make_speaker("hermit", None),
        ];
        let solos = app.solo_speakers();
        assert_eq!(solos.len(), 1);
        assert_eq!(solos[0].name, "hermit");
    }

    #[test]
    fn test_group_members_of_returns_all_members() {
        let mut app = App::new();
        app.speakers = vec![
            make_speaker("cthulhu", Some("cthulhu")),
            make_speaker("family", Some("cthulhu")),
            make_speaker("hermit", None),
        ];
        let members = app.group_members_of("cthulhu");
        assert_eq!(members.len(), 2);
    }
}
