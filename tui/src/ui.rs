use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};
use crate::app::{App, Panel};
use crate::command;

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
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
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
    draw_status_line(f, app, outer[1]);
    draw_help_bar(f, app, outer[2]);
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

    if app.is_grouped() {
        draw_topology(f, app, block, area);
    } else {
        draw_speaker_list(f, app, block, area);
    }
}

fn draw_speaker_list(f: &mut Frame, app: &App, block: Block, area: Rect) {
    let active = app.active_panel == Panel::Speakers;
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
            Some(coord) if coord == &sp.name => Span::styled(" ◈", Style::default().fg(ACCENT)),
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

fn draw_topology(f: &mut Frame, app: &App, block: Block<'_>, area: Rect) {
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = vec![];

    for coord in app.coordinators() {
        let display = coord.alias.as_deref().unwrap_or(&coord.name);
        let members = app.group_members_of(&coord.name);
        let max_name_len = members.iter()
            .map(|s| s.alias.as_deref().unwrap_or(&s.name).len())
            .max()
            .unwrap_or(0)
            .max(display.len());
        // bar width = max_name_len + 4 (tag + space + state) + 2 (║ inner padding) = +6
        let bar = "═".repeat(max_name_len + 6);

        lines.push(Line::from(Span::styled(
            format!(" ╔{}╗", bar),
            Style::default().fg(ACCENT),
        )));
        for m in &members {
            let name = m.alias.as_deref().unwrap_or(&m.name);
            let tag = if m.group_coordinator.as_deref() == Some(m.name.as_str()) {
                " ◈"
            } else {
                " ↳"
            };
            let (state_str, state_color) = match m.state.as_str() {
                "PLAYING"          => ("▶", PLAYING),
                "PAUSED_PLAYBACK"  => ("⏸", PAUSED),
                _                  => ("·", DIM),
            };
            lines.push(Line::from(vec![
                Span::styled(" ║ ", Style::default().fg(ACCENT)),
                Span::styled(format!("{:<width$}", name, width = max_name_len), Style::default().fg(FG)),
                Span::styled(tag, Style::default().fg(DIM)),
                Span::raw(" "),
                Span::styled(state_str, Style::default().fg(state_color)),
                Span::styled(" ║", Style::default().fg(ACCENT)),
            ]));
        }
        lines.push(Line::from(Span::styled(
            format!(" ╚{}╝", bar),
            Style::default().fg(ACCENT),
        )));
        lines.push(Line::from(""));
    }

    for sp in app.solo_speakers() {
        let name = sp.alias.as_deref().unwrap_or(&sp.name);
        let state = match sp.state.as_str() {
            "PLAYING" => Span::styled("▶", Style::default().fg(PLAYING)),
            "PAUSED_PLAYBACK" => Span::styled("⏸", Style::default().fg(PAUSED)),
            _ => Span::styled("·", Style::default().fg(DIM)),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("   {} ", name), Style::default().fg(DIM)),
            state,
            Span::styled(" (solo)", Style::default().fg(DIM)),
        ]));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
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

    let entities = app.playing_entities();

    if entities.is_empty() {
        let idle = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("  Nothing playing", Style::default().fg(DIM))),
        ]);
        f.render_widget(idle, inner);
        return;
    }

    if entities.len() == 1 {
        draw_track_block(f, entities[0], inner, true);
        return;
    }

    // Stacked view: divide inner area equally among entities
    let chunk_h = inner.height / entities.len() as u16;
    if chunk_h == 0 {
        // Terminal too small to stack — render only the first entity
        draw_track_block(f, entities[0], inner, false);
        return;
    }
    for (i, sp) in entities.iter().enumerate() {
        let is_last = i == entities.len() - 1;
        let height = if is_last {
            inner.height - chunk_h * (entities.len() as u16 - 1)
        } else {
            chunk_h
        };
        let chunk = Rect {
            y: inner.y + i as u16 * chunk_h,
            height,
            ..inner
        };
        draw_track_block(f, sp, chunk, false);
    }
}

fn draw_track_block(f: &mut Frame, sp: &crate::api::Speaker, area: Rect, show_vol: bool) {
    if area.height == 0 {
        return;
    }
    // Group/speaker label (dim)
    let label_area = Rect { y: area.y, height: 1, ..area };
    let label = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("  {} ", sp.alias.as_deref().unwrap_or(&sp.name)),
            Style::default().fg(DIM),
        ),
    ]));
    f.render_widget(label, label_area);

    let content_area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };

    if let Some(track) = &sp.track {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // title
                Constraint::Length(1), // artist
                Constraint::Length(1), // album
                Constraint::Length(1), // spacer
                Constraint::Length(1), // progress bar
                Constraint::Length(1), // time
                Constraint::Length(1), // spacer
                Constraint::Length(1), // volume (optional)
                Constraint::Min(0),
            ])
            .split(content_area);

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ♫ ", Style::default().fg(PLAYING)),
                Span::styled(&track.title, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            ])),
            chunks[0],
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.artist, Style::default().fg(ACCENT)),
            ])),
            chunks[1],
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.album, Style::default().fg(DIM)),
            ])),
            chunks[2],
        );

        let ratio = if track.duration > 0 {
            (track.position as f64 / track.duration as f64).min(1.0)
        } else {
            0.0
        };
        let gauge_area = Rect {
            x: chunks[4].x + 4,
            width: chunks[4].width.saturating_sub(8),
            ..chunks[4]
        };
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(ACCENT).bg(Color::Rgb(40, 40, 55)))
                .ratio(ratio)
                .label(""),
            gauge_area,
        );
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("    {} / {}", format_time(track.position), format_time(track.duration)),
                Style::default().fg(DIM),
            )),
            chunks[5],
        );

        if show_vol {
            let vol_area = Rect {
                x: chunks[7].x + 4,
                width: chunks[7].width.saturating_sub(8),
                ..chunks[7]
            };
            f.render_widget(
                Gauge::default()
                    .gauge_style(Style::default().fg(PLAYING).bg(Color::Rgb(40, 40, 55)))
                    .ratio((sp.volume as f64 / 100.0).min(1.0))
                    .label(format!("Vol: {}", sp.volume)),
                vol_area,
            );
        }
    } else {
        f.render_widget(
            Paragraph::new(Span::styled("  Nothing playing", Style::default().fg(DIM))),
            content_area,
        );
    }
}

fn draw_status_line(f: &mut Frame, app: &App, area: Rect) {
    let msg = app.active_status();
    let style = if msg.is_empty() {
        Style::default().fg(DIM).bg(BG)
    } else {
        Style::default().fg(ACCENT).bg(BG)
    };
    let para = Paragraph::new(format!(" {}", msg)).style(style);
    f.render_widget(para, area);
}

fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
    if let Some(input) = &app.command_input {
        let playlist_names: Vec<String> = app.playlists
            .iter()
            .map(|p| p.favorite_name.clone())
            .collect();
        let ghost = command::autocomplete(input, &playlist_names);

        let mut spans = vec![
            Span::styled("  :", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(input.clone(), Style::default().fg(FG)),
        ];
        if let Some(g) = ghost {
            spans.push(Span::styled(g, Style::default().fg(DIM)));
        }
        spans.push(Span::styled("▌", Style::default().fg(ACCENT)));

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(BG));
        f.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
        return;
    }

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
        Span::styled(":", Style::default().fg(ACCENT)),
        Span::styled(" cmd  ", Style::default().fg(DIM)),
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
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max.saturating_sub(1)).collect();
    if chars.next().is_some() {
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}
