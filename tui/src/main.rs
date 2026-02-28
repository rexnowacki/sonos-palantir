mod api;
mod app;
mod ui;

use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::prelude::*;
use crate::api::{ApiClient, Speaker};
use crate::app::App;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const TICK_RATE: Duration = Duration::from_millis(100);

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let client = Arc::new(ApiClient::new());
    let mut app = App::new();

    if let Ok(speakers) = client.get_speakers().await {
        app.speakers = speakers;
    }
    if let Ok(playlists) = client.get_playlists().await {
        app.playlists = playlists;
    }
    if let Ok(favs) = client.get_favorites().await {
        let existing: std::collections::HashSet<String> = app.playlists
            .iter()
            .map(|p| p.favorite_name.to_lowercase())
            .collect();
        for title in favs {
            if !existing.contains(&title.to_lowercase()) {
                app.playlists.push(crate::api::Playlist {
                    alias: title.clone(),
                    favorite_name: title,
                });
            }
        }
    }

    // Background refresh — never blocks the event loop
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<Speaker>>(1);
    let refresh_client = Arc::clone(&client);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            if let Ok(speakers) = refresh_client.get_speakers().await {
                let _ = tx.send(speakers).await;
            }
        }
    });

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // Apply any fresh speaker data without blocking
        if let Ok(speakers) = rx.try_recv() {
            app.speakers = speakers;
        }

        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut app, &client, key).await?;
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_key(app: &mut App, client: &ApiClient, key: KeyEvent) -> Result<()> {
    // Volume input mode intercepts all keys
    if app.volume_input.is_some() {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let input = app.volume_input.as_mut().unwrap();
                if input.len() < 3 {
                    input.push(c);
                }
            }
            KeyCode::Backspace => {
                app.volume_input.as_mut().unwrap().pop();
            }
            KeyCode::Enter => {
                if let Some(input) = app.volume_input.take() {
                    // Empty or non-numeric input silently cancels (same as Esc)
                    if let Ok(vol) = input.parse::<u8>() {
                        let vol = vol.min(100);
                        if let Some(id) = app.speaker_id() {
                            let _ = client.set_volume(&id, vol).await;
                        }
                    }
                }
            }
            KeyCode::Esc => {
                app.volume_input = None;
            }
            _ => {}
        }
        return Ok(());
    }

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => app.cycle_panel(),

        KeyCode::Up | KeyCode::Char('k') => app.prev_in_list(),
        KeyCode::Down | KeyCode::Char('j') => app.next_in_list(),

        KeyCode::Enter => {
            if let (Some(speaker_id), Some(playlist)) =
                (app.speaker_id(), app.selected_playlist())
            {
                let _ = client.play(&speaker_id, &playlist.alias).await;
                app.set_status(format!("Playing {} on {}", playlist.alias, speaker_id), 3);
            }
        }

        KeyCode::Char(' ') => {
            if let Some(sp) = app.selected_speaker() {
                let id = sp.alias.as_deref().unwrap_or(&sp.name);
                match sp.state.as_str() {
                    "PLAYING" => { let _ = client.pause(id).await; }
                    _ => { let _ = client.resume(id).await; }
                }
            }
        }

        KeyCode::Char('+') | KeyCode::Char('=') => {
            if let Some(sp) = app.selected_speaker() {
                let id = sp.alias.as_deref().unwrap_or(&sp.name).to_string();
                let new_vol = (sp.volume + 5).min(100);
                let _ = client.set_volume(&id, new_vol).await;
            }
        }
        KeyCode::Char('-') => {
            if let Some(sp) = app.selected_speaker() {
                let id = sp.alias.as_deref().unwrap_or(&sp.name).to_string();
                let new_vol = sp.volume.saturating_sub(5);
                let _ = client.set_volume(&id, new_vol).await;
            }
        }

        KeyCode::Char('n') => {
            if let Some(id) = app.speaker_id() {
                let _ = client.next(&id).await;
            }
        }
        KeyCode::Char('p') => {
            if let Some(id) = app.speaker_id() {
                let _ = client.previous(&id).await;
            }
        }

        KeyCode::Char('g') => {
            if app.is_grouped() {
                let _ = client.ungroup_all().await;
            } else {
                let _ = client.group_all().await;
            }
        }

        KeyCode::Char('v') => {
            app.volume_input = Some(String::new());
        }

        _ => {}
    }
    Ok(())
}
