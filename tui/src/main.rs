mod api;
mod app;
mod command;
mod history;
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

    terminal.draw(|f| ui::draw_splash(f))?;
    std::thread::sleep(std::time::Duration::from_secs(1));

    let result = run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let client = Arc::new(ApiClient::new());
    let mut app = App::new();

    match client.get_speakers().await {
        Ok(speakers) => app.speakers = speakers,
        Err(_) => app.set_status("The gates of Moria are sealed. Start sonosd.", 3600),
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

    if let Ok(sort) = client.get_playlist_sort().await {
        if sort == "popularity" {
            history::popularity_sort(&mut app.playlists);
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

        // Check sleep timer expiry
        if let Some(sleep_until) = app.sleep_until {
            if std::time::Instant::now() >= sleep_until {
                app.sleep_until = None;
                for sp in &app.speakers {
                    let id = sp.alias.as_deref().unwrap_or(&sp.name).to_string();
                    let _ = client.pause(&id).await;
                }
                app.set_status("The Fellowship rests. All speakers paused.", 5);
            }
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

async fn execute_command(app: &mut App, client: &ApiClient, input: &str) -> Result<()> {
    use command::Command;
    match command::parse(input) {
        Some(Command::Play(name)) => {
            if let Some(id) = app.speaker_id() {
                let playlist = app.playlists.iter().find(|p| {
                    p.alias.to_lowercase().contains(&name.to_lowercase())
                        || p.favorite_name.to_lowercase().contains(&name.to_lowercase())
                });
                if let Some(pl) = playlist {
                    let alias = pl.alias.clone();
                    let _ = client.play(&id, &alias).await;
                    history::record_play(&alias);
                    app.set_status(format!("Playing {} on {}", alias, id), 3);
                } else {
                    app.set_status("Not all those who wander are found in this network.", 4);
                }
            }
        }
        Some(Command::Volume(target, v)) => {
            let ids: Vec<String> = match target.as_deref() {
                None => app.speaker_id().into_iter().collect(),
                Some("all") => app.speakers.iter()
                    .map(|s| s.alias.as_deref().unwrap_or(&s.name).to_string())
                    .collect(),
                Some(name) => vec![name.to_string()],
            };
            if !ids.is_empty() {
                for id in &ids {
                    let _ = client.set_volume(id, v).await;
                }
                for sp in &mut app.speakers {
                    let sp_id = sp.alias.as_deref().unwrap_or(&sp.name).to_string();
                    if ids.contains(&sp_id) {
                        sp.volume = v;
                    }
                }
                let status = if v == 100 {
                    "You shall not pass... 100.".to_string()
                } else {
                    match target.as_deref() {
                        None => format!("Volume set to {}.", v),
                        Some("all") => format!("Volume set to {} on all speakers.", v),
                        Some(name) => format!("Volume set to {} on {}.", v, name),
                    }
                };
                app.set_status(status, 2);
            }
        }
        Some(Command::GroupAll) => {
            let _ = client.group_all().await;
            app.set_status("The fellowship is assembled.", 3);
        }
        Some(Command::Ungroup) => {
            let _ = client.ungroup_all().await;
            app.set_status("The company is scattered to the winds.", 3);
        }
        Some(Command::Next) => {
            if let Some(id) = app.speaker_id() {
                match client.next(&id).await {
                    Ok(()) => app.set_status("Onward, into shadow.", 2),
                    Err(_) => app.set_status("The road goes ever on — but not to the next track.", 3),
                }
            }
        }
        Some(Command::Prev) => {
            if let Some(id) = app.speaker_id() {
                match client.previous(&id).await {
                    Ok(()) => app.set_status("Back to the beginning.", 2),
                    Err(_) => app.set_status("The road goes ever on — but not to the previous track.", 3),
                }
            }
        }
        Some(Command::Sleep(mins)) => {
            app.sleep_until = Some(
                std::time::Instant::now()
                    + std::time::Duration::from_secs(mins as u64 * 60)
            );
        }
        Some(Command::SleepCancel) => {
            app.sleep_until = None;
            app.set_status("The Palantir's dream is dispelled — sleep cancelled.", 3);
        }
        Some(Command::Reload) => {
            let _ = client.reload().await;
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
            app.set_status("The scrolls are refreshed. Reloaded config.yaml.", 3);
        }
        Some(Command::Unknown(_)) | None => {
            app.set_status("Speak, friend — but speak clearly.", 3);
        }
    }
    Ok(())
}

async fn handle_key(app: &mut App, client: &ApiClient, key: KeyEvent) -> Result<()> {
    // Command mode intercepts all keys
    if app.command_input.is_some() {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                app.command_input.as_mut().unwrap().push(c);
            }
            KeyCode::Backspace => {
                let input = app.command_input.as_mut().unwrap();
                if input.is_empty() {
                    app.command_input = None; // backspace on empty exits
                } else {
                    input.pop();
                }
            }
            KeyCode::Tab => {
                let playlist_names: Vec<String> = app.playlists
                    .iter()
                    .map(|p| p.favorite_name.clone())
                    .collect();
                let current = app.command_input.as_ref().unwrap().clone();
                if let Some(ghost) = command::autocomplete(&current, &playlist_names) {
                    if ghost.starts_with(" → ") {
                        // contains-match ghost: replace query with full name
                        let parts: Vec<&str> = current.splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            let completed = format!("{} {}", parts[0], &ghost[" → ".len()..]);
                            *app.command_input.as_mut().unwrap() = completed;
                        }
                    } else {
                        app.command_input.as_mut().unwrap().push_str(&ghost);
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(input) = app.command_input.take() {
                    execute_command(app, client, &input).await?;
                }
            }
            KeyCode::Esc => {
                app.command_input = None;
            }
            _ => {}
        }
        return Ok(());
    }

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
                history::record_play(&playlist.alias);
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
                match client.next(&id).await {
                    Ok(()) => app.set_status("Onward, into shadow.", 2),
                    Err(_) => app.set_status("The road goes ever on — but not to the next track.", 3),
                }
            }
        }
        KeyCode::Char('p') => {
            if let Some(id) = app.speaker_id() {
                match client.previous(&id).await {
                    Ok(()) => app.set_status("Back to the beginning.", 2),
                    Err(_) => app.set_status("The road goes ever on — but not to the previous track.", 3),
                }
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

        KeyCode::Char(':') => {
            app.command_input = Some(String::new());
            app.volume_input = None; // mutually exclusive
        }
        KeyCode::Char('?') => {
            app.help_open = !app.help_open;
        }
        KeyCode::Esc => {
            if app.help_open {
                app.help_open = false;
            }
        }

        _ => {}
    }
    Ok(())
}
