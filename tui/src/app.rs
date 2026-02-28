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
    pub last_refresh: std::time::Instant,
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
            last_refresh: std::time::Instant::now(),
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
}
