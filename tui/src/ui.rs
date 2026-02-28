use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};
use crate::app::{App, Panel};

const BG: Color = Color::Rgb(20, 20, 30);
const FG: Color = Color::Rgb(200, 200, 210);
const ACCENT: Color = Color::Rgb(130, 170, 255);
const PLAYING: Color = Color::Rgb(120, 220, 140);
const PAUSED: Color = Color::Rgb(240, 200, 80);
const DIM: Color = Color::Rgb(80, 80, 100);
const HIGHLIGHT_BG: Color = Color::Rgb(40, 45, 65);
const BORDER_ACTIVE: Color = ACCENT;
const BORDER_INACTIVE: Color = Color::Rgb(50, 50, 70);

pub fn draw(f: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(f.area());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(outer[0]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(main[0]);

    draw_speakers(f, app, left[0]);
    draw_playlists(f, app, left[1]);
    draw_now_playing(f, app, main[1]);
    draw_help_bar(f, app, outer[1]);
}

fn panel_block(title: &str, active: bool) -> Block<'_> {
    let border_color = if active { BORDER_ACTIVE } else { BORDER_INACTIVE };
    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG))
}

fn draw_speakers(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Speakers;
    let block = panel_block("Speakers", active);

    let items: Vec<ListItem> = app.speakers.iter().enumerate().map(|(i, sp)| {
        let state_icon = match sp.state.as_str() {
            "PLAYING" => Span::styled("▶", Style::default().fg(PLAYING)),
            "PAUSED_PLAYBACK" => Span::styled("⏸", Style::default().fg(PAUSED)),
            _ => Span::styled("·", Style::default().fg(DIM)),
        };

        let display_name = sp.alias.as_deref().unwrap_or(&sp.name);
        let name_style = if i == app.speaker_index && active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG)
        };

        let group_tag = match &sp.group_coordinator {
            None => Span::raw("  "),
            Some(coord) if coord == &sp.name => {
                Span::styled(" ◈", Style::default().fg(ACCENT))
            }
            Some(_) => Span::styled(" ↳", Style::default().fg(DIM)),
        };

        let line = Line::from(vec![
            Span::raw(if i == app.speaker_index { " ► " } else { "   " }),
            Span::styled(format!("{:<14}", display_name), name_style),
            group_tag,
            Span::styled(format!(" {:>3}", sp.volume), Style::default().fg(DIM)),
            Span::raw("  "),
            state_icon,
        ]);

        let mut item = ListItem::new(line);
        if i == app.speaker_index && active {
            item = item.style(Style::default().bg(HIGHLIGHT_BG));
        }
        item
    }).collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_playlists(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Playlists;
    let block = panel_block("Playlists", active);

    let items: Vec<ListItem> = app.playlists.iter().enumerate().map(|(i, pl)| {
        let style = if i == app.playlist_index && active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG)
        };

        let line = Line::from(vec![
            Span::raw(if i == app.playlist_index { " ► " } else { "   " }),
            Span::styled(format!("{:<10}", pl.alias), style),
            Span::styled(
                truncate(&pl.favorite_name, 24),
                Style::default().fg(DIM),
            ),
        ]);

        let mut item = ListItem::new(line);
        if i == app.playlist_index && active {
            item = item.style(Style::default().bg(HIGHLIGHT_BG));
        }
        item
    }).collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_now_playing(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::NowPlaying;
    let block = panel_block("Now Playing", active);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let speaker = app.selected_speaker();

    if let Some(sp) = speaker {
        if let Some(track) = &sp.track {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(2),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(2),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(inner);

            let title = Paragraph::new(Line::from(vec![
                Span::styled("  ♫ ", Style::default().fg(PLAYING)),
                Span::styled(
                    &track.title,
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                ),
            ]));
            f.render_widget(title, chunks[1]);

            let artist = Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.artist, Style::default().fg(ACCENT)),
            ]));
            f.render_widget(artist, chunks[2]);

            let album = Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.album, Style::default().fg(DIM)),
            ]));
            f.render_widget(album, chunks[3]);

            let ratio = if track.duration > 0 {
                track.position as f64 / track.duration as f64
            } else {
                0.0
            };
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(ACCENT).bg(Color::Rgb(40, 40, 55)))
                .ratio(ratio)
                .label("");
            let gauge_area = Rect {
                x: chunks[5].x + 4,
                width: chunks[5].width.saturating_sub(8),
                ..chunks[5]
            };
            f.render_widget(gauge, gauge_area);

            let time_str = format!(
                "    {}  /  {}",
                format_time(track.position),
                format_time(track.duration),
            );
            let time = Paragraph::new(Span::styled(time_str, Style::default().fg(DIM)));
            f.render_widget(time, chunks[6]);

            let vol_ratio = sp.volume as f64 / 100.0;
            let vol_gauge = Gauge::default()
                .gauge_style(Style::default().fg(PLAYING).bg(Color::Rgb(40, 40, 55)))
                .ratio(vol_ratio)
                .label(format!("Vol: {}", sp.volume));
            let vol_area = Rect {
                x: chunks[8].x + 4,
                width: chunks[8].width.saturating_sub(8),
                ..chunks[8]
            };
            f.render_widget(vol_gauge, vol_area);

            return;
        }
    }

    let idle = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Nothing playing",
            Style::default().fg(DIM),
        )),
    ]);
    f.render_widget(idle, inner);
}

fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
    if let Some(input) = &app.volume_input {
        let prompt = Line::from(vec![
            Span::styled("  Vol: ", Style::default().fg(ACCENT)),
            Span::styled(
                format!("[{}▌]", input),
                Style::default().fg(FG).add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Enter confirm   Esc cancel", Style::default().fg(DIM)),
        ]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(BG));
        f.render_widget(Paragraph::new(prompt).block(block), area);
        return;
    }

    let help = Line::from(vec![
        Span::styled(" Tab", Style::default().fg(ACCENT)),
        Span::styled(" panel  ", Style::default().fg(DIM)),
        Span::styled("↑↓", Style::default().fg(ACCENT)),
        Span::styled(" nav  ", Style::default().fg(DIM)),
        Span::styled("Enter", Style::default().fg(ACCENT)),
        Span::styled(" play  ", Style::default().fg(DIM)),
        Span::styled("Space", Style::default().fg(ACCENT)),
        Span::styled(" pause  ", Style::default().fg(DIM)),
        Span::styled("+/-", Style::default().fg(ACCENT)),
        Span::styled(" vol  ", Style::default().fg(DIM)),
        Span::styled("v", Style::default().fg(ACCENT)),
        Span::styled(" vol#  ", Style::default().fg(DIM)),
        Span::styled("n/p", Style::default().fg(ACCENT)),
        Span::styled(" track  ", Style::default().fg(DIM)),
        Span::styled("g", Style::default().fg(ACCENT)),
        Span::styled(" group  ", Style::default().fg(DIM)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(DIM)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_INACTIVE))
        .style(Style::default().bg(BG));
    let paragraph = Paragraph::new(help).block(block);
    f.render_widget(paragraph, area);
}

fn format_time(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max - 1])
    } else {
        s.to_string()
    }
}
