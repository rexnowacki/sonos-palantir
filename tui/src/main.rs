mod api;
mod app;
mod ui;

use std::time::Duration;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::prelude::*;
use crate::api::ApiClient;
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
    let client = ApiClient::new();
    let mut app = App::new();

    if let Ok(speakers) = client.get_speakers().await {
        app.speakers = speakers;
    }
    if let Ok(playlists) = client.get_playlists().await {
        app.playlists = playlists;
    }

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut app, &client, key).await?;
            }
        }

        if app.last_refresh.elapsed() >= POLL_INTERVAL {
            if let Ok(speakers) = client.get_speakers().await {
                app.speakers = speakers;
            }
            app.last_refresh = std::time::Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_key(app: &mut App, client: &ApiClient, key: KeyEvent) -> Result<()> {
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
                app.status_message = Some(format!(
                    "Playing {} on {}", playlist.alias, speaker_id
                ));
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

        _ => {}
    }
    Ok(())
}
