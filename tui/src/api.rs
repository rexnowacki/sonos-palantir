use serde::{Deserialize, Serialize};

const BASE_URL: &str = "http://127.0.0.1:9271";

#[derive(Debug, Clone, Deserialize)]
pub struct Speaker {
    pub name: String,
    pub alias: Option<String>,
    pub ip: String,
    pub volume: u8,
    pub muted: bool,
    pub state: String,
    pub group_coordinator: Option<String>,
    pub track: Option<Track>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u64,
    pub position: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Playlist {
    pub alias: String,
    pub favorite_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayRequest {
    pub speaker: String,
    pub playlist: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeakerRequest {
    pub speaker: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VolumeRequest {
    pub speaker: String,
    pub volume: u8,
}

pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl ApiClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: BASE_URL.to_string(),
        }
    }

    pub async fn get_speakers(&self) -> anyhow::Result<Vec<Speaker>> {
        let resp: serde_json::Value = self.client
            .get(format!("{}/speakers", self.base_url))
            .send().await?
            .json().await?;
        let speakers: Vec<Speaker> = serde_json::from_value(resp["speakers"].clone())?;
        Ok(speakers)
    }

    pub async fn get_playlists(&self) -> anyhow::Result<Vec<Playlist>> {
        let resp: serde_json::Value = self.client
            .get(format!("{}/playlists", self.base_url))
            .send().await?
            .json().await?;
        let map: std::collections::HashMap<String, String> =
            serde_json::from_value(resp["playlists"].clone())?;
        Ok(map.into_iter().map(|(alias, favorite_name)| {
            Playlist { alias, favorite_name }
        }).collect())
    }

    pub async fn get_favorites(&self) -> anyhow::Result<Vec<String>> {
        let resp: serde_json::Value = self.client
            .get(format!("{}/favorites", self.base_url))
            .send().await?
            .json().await?;
        let favs = resp["favorites"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(favs.iter()
            .filter_map(|f| f["title"].as_str().map(|s| s.to_string()))
            .collect())
    }

    pub async fn play(&self, speaker: &str, playlist: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/play", self.base_url))
            .json(&PlayRequest {
                speaker: speaker.to_string(),
                playlist: playlist.to_string(),
            })
            .send().await?;
        Ok(())
    }

    pub async fn pause(&self, speaker: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/pause", self.base_url))
            .json(&SpeakerRequest { speaker: speaker.to_string() })
            .send().await?;
        Ok(())
    }

    pub async fn resume(&self, speaker: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/resume", self.base_url))
            .json(&SpeakerRequest { speaker: speaker.to_string() })
            .send().await?;
        Ok(())
    }

    pub async fn set_volume(&self, speaker: &str, volume: u8) -> anyhow::Result<()> {
        self.client.post(format!("{}/volume", self.base_url))
            .json(&VolumeRequest {
                speaker: speaker.to_string(),
                volume,
            })
            .send().await?;
        Ok(())
    }

    pub async fn next(&self, speaker: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/next", self.base_url))
            .json(&SpeakerRequest { speaker: speaker.to_string() })
            .send().await?;
        Ok(())
    }

    pub async fn previous(&self, speaker: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/previous", self.base_url))
            .json(&SpeakerRequest { speaker: speaker.to_string() })
            .send().await?;
        Ok(())
    }

    pub async fn group_all(&self) -> anyhow::Result<()> {
        self.client.post(format!("{}/group", self.base_url))
            .json(&serde_json::json!({"speakers": ["all"]}))
            .send().await?;
        Ok(())
    }

    pub async fn ungroup_all(&self) -> anyhow::Result<()> {
        self.client.post(format!("{}/ungroup", self.base_url))
            .json(&SpeakerRequest { speaker: "all".to_string() })
            .send().await?;
        Ok(())
    }
}
