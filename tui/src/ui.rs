use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
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

pub fn draw_splash(f: &mut Frame) {
    let area = f.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let v_offset = inner.height.saturating_sub(4) / 2;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_offset),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Span::styled(
            "S O N O S - P A L A N T I R",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )).alignment(ratatui::layout::Alignment::Center),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            "══════════════════════════",
            Style::default().fg(ACCENT),
        )).alignment(ratatui::layout::Alignment::Center),
        chunks[2],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            "Seeing through sound...",
            Style::default().fg(DIM),
        )).alignment(ratatui::layout::Alignment::Center),
        chunks[3],
    );
}

pub fn draw(f: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // top status bar
            Constraint::Min(1),     // main panels
            Constraint::Length(1),   // status line
            Constraint::Length(3),   // help bar / command input
        ])
        .split(f.area());

    draw_top_bar(f, app, outer[0]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(outer[1]);

    // Dynamic left column: Rooms takes what it needs, Playlists gets the rest
    let speaker_rows = if app.is_grouped() {
        let mut rows: u16 = 0;
        for coord in app.coordinators() {
            let members = app.group_members_of(&coord.name);
            rows += 1 + (members.len() as u16 * 2) + 1; // header + members*2 + blank
        }
        for _solo in app.solo_speakers() {
            rows += 2;
        }
        rows
    } else {
        app.speakers.len() as u16 * 2
    };
    let rooms_height = speaker_rows + 2; // +2 for border top/bottom
    // Cap rooms so playlists always gets at least 5 rows (border + 3 items)
    let left_height = main[0].height;
    let rooms_capped = rooms_height.min(left_height.saturating_sub(5));

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(rooms_capped), Constraint::Min(5)])
        .split(main[0]);

    draw_speakers(f, app, left[0]);
    draw_playlists(f, app, left[1]);
    draw_now_playing(f, app, main[1]);
    draw_status_line(f, app, outer[2]);
    draw_help_bar(f, app, outer[3]);

    if app.help_open {
        draw_help_overlay(f);
    }
    if app.episode_popup {
        draw_episode_popup(f, app);
    }
}

const TOP_BAR_BG: Color = Color::Rgb(30, 30, 45);

fn draw_top_bar(f: &mut Frame, app: &App, area: Rect) {
    let selected = app.selected_speaker();

    // Playing indicator dot
    let (dot, dot_color) = match selected.map(|s| s.state.as_str()) {
        Some("PLAYING") => ("●", PLAYING),
        Some("PAUSED_PLAYBACK") => ("●", PAUSED),
        _ => ("●", DIM),
    };

    // Speaker name
    let speaker_name = selected
        .map(|s| s.alias.as_deref().unwrap_or(&s.name).to_string())
        .unwrap_or_else(|| "—".to_string());

    // Track info
    let track_info = selected
        .and_then(|s| s.track.as_ref())
        .map(|t| format!("{} — {}", t.title, t.artist))
        .unwrap_or_default();

    // Volume
    let vol = selected.map(|s| format!("VOL {}%", s.volume)).unwrap_or_default();

    // Daemon status
    let daemon_status = if app.speakers.is_empty() {
        Span::styled("palantir:ERR", Style::default().fg(Color::Rgb(220, 80, 80)))
    } else {
        Span::styled("palantir:OK", Style::default().fg(PLAYING))
    };

    // Speaker count
    let count = format!("Sonos:{}", app.speakers.len());

    // Truncate track info to fit available space
    let right_len = vol.len() + 14 + count.len() + 6;
    let available = (area.width as usize)
        .saturating_sub(speaker_name.len())
        .saturating_sub(right_len)
        .saturating_sub(6);
    let track_display = if track_info.len() > available {
        truncate(&track_info, available)
    } else {
        track_info
    };

    let spans = vec![
        Span::styled(format!(" {} ", dot), Style::default().fg(dot_color).bg(TOP_BAR_BG)),
        Span::styled(format!("{} ", speaker_name), Style::default().fg(ACCENT).bg(TOP_BAR_BG).add_modifier(Modifier::BOLD)),
        Span::styled(track_display, Style::default().fg(FG).bg(TOP_BAR_BG)),
        Span::styled("  ", Style::default().bg(TOP_BAR_BG)),
        Span::styled(format!("{} ", vol), Style::default().fg(DIM).bg(TOP_BAR_BG)),
        Span::styled(" ", Style::default().bg(TOP_BAR_BG)),
        daemon_status.style(Style::default().bg(TOP_BAR_BG)),
        Span::styled(format!("  {} ", count), Style::default().fg(DIM).bg(TOP_BAR_BG)),
    ];

    let bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(TOP_BAR_BG));
    f.render_widget(bar, area);
}

fn panel_block(title: &str, active: bool) -> Block<'_> {
    let border_color = if active { BORDER_ACTIVE } else { BORDER_INACTIVE };
    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG))
}

/// Returns a color for the volume bar: green (0-50), yellow (51-80), red (81-100).
fn volume_color(vol: u8) -> Color {
    if vol <= 50 {
        Color::Rgb(120, 220, 140)
    } else if vol <= 80 {
        Color::Rgb(240, 200, 80)
    } else {
        Color::Rgb(220, 80, 80)
    }
}

/// Render a volume bar string using block characters.
fn volume_bar(vol: u8, width: usize) -> (String, Color) {
    let color = volume_color(vol);
    let filled = (vol as usize * width) / 100;
    let remainder = (vol as usize * width) % 100;
    let partial = if filled < width && remainder > 0 {
        if remainder > 66 { "▓" } else if remainder > 33 { "▒" } else { "░" }
    } else {
        ""
    };
    let empty = width.saturating_sub(filled).saturating_sub(if partial.is_empty() { 0 } else { 1 });
    let bar = format!("{}{}{}", "█".repeat(filled), partial, " ".repeat(empty));
    (bar, color)
}

fn draw_speakers(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Speakers;
    let block = panel_block("Rooms", active);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = vec![];
    let bar_width = (inner.width as usize).saturating_sub(6);

    if app.is_grouped() {
        for coord in app.coordinators() {
            let members = app.group_members_of(&coord.name);
            let member_names: Vec<&str> = members.iter()
                .map(|m| m.alias.as_deref().unwrap_or(&m.name))
                .collect();
            lines.push(Line::from(vec![
                Span::styled(" GROUPED ", Style::default().fg(DIM)),
                Span::styled(member_names.join(" + "), Style::default().fg(ACCENT)),
            ]));
            for m in &members {
                let sp_index = app.speakers.iter().position(|s| s.name == m.name);
                let is_selected = active && sp_index == Some(app.speaker_index);
                render_speaker_row(&mut lines, m, is_selected, bar_width);
            }
        }
        for sp in app.solo_speakers() {
            let sp_index = app.speakers.iter().position(|s| s.name == sp.name);
            let is_selected = active && sp_index == Some(app.speaker_index);
            render_speaker_row(&mut lines, sp, is_selected, bar_width);
        }
    } else {
        for (i, sp) in app.speakers.iter().enumerate() {
            let is_selected = active && i == app.speaker_index;
            render_speaker_row(&mut lines, sp, is_selected, bar_width);
        }
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

fn render_speaker_row(lines: &mut Vec<Line>, sp: &crate::api::Speaker, selected: bool, bar_width: usize) {
    let name = sp.alias.as_deref().unwrap_or(&sp.name);
    let marker = if selected { "▸" } else { " " };
    let (state_icon, state_color) = match sp.state.as_str() {
        "PLAYING" => ("▶", PLAYING),
        "PAUSED_PLAYBACK" => ("‖", PAUSED),
        _ => ("·", DIM),
    };
    let name_style = if selected {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG)
    };

    let name_line = Line::from(vec![
        Span::styled(format!(" {} ", marker), if selected { Style::default().fg(ACCENT) } else { Style::default().fg(DIM) }),
        Span::styled(format!("{:<12}", name), name_style),
        Span::styled(format!(" {} ", state_icon), Style::default().fg(state_color)),
        Span::styled(format!("{:>3}", sp.volume), Style::default().fg(DIM)),
    ]);
    lines.push(name_line);

    // Volume bar below speaker name
    let (bar, color) = volume_bar(sp.volume, bar_width);
    lines.push(Line::from(vec![
        Span::raw("   "),
        Span::styled(bar, Style::default().fg(color)),
    ]));
}

fn draw_playlists(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Playlists;
    if app.source_mode == crate::app::SourceMode::Podcasts {
        draw_podcasts_panel(f, app, area, active);
        return;
    }
    let block = panel_block("Playlists", active);
    let inner_width = area.width.saturating_sub(2) as usize;

    let items: Vec<ListItem> = app.playlists.iter().enumerate().map(|(i, pl)| {
        let selected = i == app.playlist_index;
        let style = if selected && active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG)
        };

        let marker = if selected { "▸" } else { " " };
        let display = truncate(&pl.alias, inner_width.saturating_sub(4));

        let line = Line::from(vec![
            Span::styled(format!(" {} ", marker), if selected { Style::default().fg(ACCENT) } else { Style::default().fg(DIM) }),
            Span::styled(display, style),
        ]);

        let mut item = ListItem::new(line);
        if selected && active {
            item = item.style(Style::default().bg(HIGHLIGHT_BG));
        }
        item
    }).collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default());

    let mut state = ListState::default();
    if !app.playlists.is_empty() {
        state.select(Some(app.playlist_index));
    }
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_podcasts_panel(f: &mut Frame, app: &App, area: Rect, active: bool) {
    if app.podcast_drill {
        // Episode list view
        let podcast_name = app.selected_podcast()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Episodes".to_string());
        let block = panel_block(&podcast_name, active);
        let inner_width = area.width.saturating_sub(2) as usize;

        let items: Vec<ListItem> = app.episodes.iter().enumerate().map(|(i, ep)| {
            let selected = i == app.episode_index;
            let style = if selected && active {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else if ep.played == 1 {
                Style::default().fg(DIM)
            } else {
                Style::default().fg(FG)
            };

            let marker = if selected { "▸" } else { " " };
            let played_marker = if ep.played == 1 { "✓" } else { " " };
            let duration_str = format_time(ep.duration);
            let title_max = inner_width.saturating_sub(12);
            let title = truncate(&ep.title, title_max);

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", marker),
                    if selected { Style::default().fg(ACCENT) } else { Style::default().fg(DIM) },
                ),
                Span::styled(title, style),
                Span::styled(format!(" {} ", played_marker), Style::default().fg(PLAYING)),
                Span::styled(duration_str, Style::default().fg(DIM)),
            ]);

            let mut item = ListItem::new(line);
            if selected && active {
                item = item.style(Style::default().bg(HIGHLIGHT_BG));
            }
            item
        }).collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default());

        let mut state = ListState::default();
        if !app.episodes.is_empty() {
            state.select(Some(app.episode_index));
        }
        f.render_stateful_widget(list, area, &mut state);
    } else {
        // Podcast list view
        let block = panel_block("Podcasts", active);
        let inner_width = area.width.saturating_sub(2) as usize;

        let items: Vec<ListItem> = app.podcasts.iter().enumerate().map(|(i, pod)| {
            let selected = i == app.podcast_index;
            let style = if selected && active {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG)
            };

            let marker = if selected { "▸" } else { " " };
            let badge = if pod.unplayed > 0 {
                format!("●{}", pod.unplayed)
            } else {
                String::new()
            };
            let name_max = inner_width.saturating_sub(badge.len() + 6);
            let name = truncate(&pod.name, name_max);

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", marker),
                    if selected { Style::default().fg(ACCENT) } else { Style::default().fg(DIM) },
                ),
                Span::styled(name, style),
                Span::styled(format!(" {}", badge), Style::default().fg(PLAYING)),
            ]);

            let mut item = ListItem::new(line);
            if selected && active {
                item = item.style(Style::default().bg(HIGHLIGHT_BG));
            }
            item
        }).collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default());

        let mut state = ListState::default();
        if !app.podcasts.is_empty() {
            state.select(Some(app.podcast_index));
        }
        f.render_stateful_widget(list, area, &mut state);
    }
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

/// Render a segmented progress bar: `═══════●─────────`.
fn segmented_progress(position: u64, duration: u64, width: usize) -> Line<'static> {
    if duration == 0 || width < 4 {
        return Line::from("");
    }
    let ratio = (position as f64 / duration as f64).min(1.0);
    let filled = (ratio * width as f64) as usize;
    let filled = filled.min(width.saturating_sub(1));

    let before = "═".repeat(filled);
    let after = "─".repeat(width.saturating_sub(filled + 1));

    Line::from(vec![
        Span::styled(before, Style::default().fg(ACCENT)),
        Span::styled("●", Style::default().fg(Color::Rgb(255, 255, 255))),
        Span::styled(after, Style::default().fg(DIM)),
    ])
}

fn draw_track_block(f: &mut Frame, sp: &crate::api::Speaker, area: Rect, _show_vol: bool) {
    if area.height == 0 {
        return;
    }
    // Speaker label
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
                Constraint::Length(1), // artist — album
                Constraint::Length(1), // spacer
                Constraint::Length(1), // source / quality
                Constraint::Length(1), // spacer
                Constraint::Length(1), // progress bar
                Constraint::Length(1), // time
                Constraint::Min(0),
            ])
            .split(content_area);

        // Track title
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ♫ ", Style::default().fg(PLAYING)),
                Span::styled(&track.title, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            ])),
            chunks[0],
        );
        // Artist — Album
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.artist, Style::default().fg(ACCENT)),
                Span::styled(" — ", Style::default().fg(DIM)),
                Span::styled(&track.album, Style::default().fg(DIM)),
            ])),
            chunks[1],
        );

        // Source / Quality line
        let source_line = if !track.source.is_empty() {
            let mut spans = vec![
                Span::raw("    "),
                Span::styled(format!("Source: {}", track.source), Style::default().fg(DIM)),
            ];
            if !track.quality.is_empty() {
                spans.push(Span::styled(format!(" · Quality: {}", track.quality), Style::default().fg(DIM)));
            }
            Line::from(spans)
        } else {
            Line::from("")
        };
        f.render_widget(Paragraph::new(source_line), chunks[3]);

        // Segmented progress bar
        let bar_width = chunks[5].width.saturating_sub(8) as usize;
        let progress = segmented_progress(track.position, track.duration, bar_width);
        let bar_area = Rect {
            x: chunks[5].x + 4,
            width: chunks[5].width.saturating_sub(8),
            ..chunks[5]
        };
        f.render_widget(Paragraph::new(progress), bar_area);

        // Time display
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("    {} / {}", format_time(track.position), format_time(track.duration)),
                Style::default().fg(DIM),
            )),
            chunks[6],
        );
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
        let speaker_names: Vec<String> = app.speakers
            .iter()
            .map(|s| s.alias.as_deref().unwrap_or(&s.name).to_string())
            .collect();
        let ghost = command::autocomplete(input, &playlist_names, &speaker_names);

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
            .border_type(BorderType::Rounded)
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
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(BG));
        f.render_widget(Paragraph::new(prompt).block(block), area);
        return;
    }

    let mut help_spans = vec![
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
        Span::styled("s", Style::default().fg(ACCENT)),
        Span::styled(" source  ", Style::default().fg(DIM)),
    ];

    if app.is_podcast_playing() {
        help_spans.push(Span::styled("f/→", Style::default().fg(ACCENT)));
        help_spans.push(Span::styled(format!(" +{}s  ", app.skip_forward), Style::default().fg(DIM)));
        help_spans.push(Span::styled("b/←", Style::default().fg(ACCENT)));
        help_spans.push(Span::styled(format!(" -{}s  ", app.skip_back), Style::default().fg(DIM)));
    } else {
        help_spans.push(Span::styled("n/p", Style::default().fg(ACCENT)));
        help_spans.push(Span::styled(" track  ", Style::default().fg(DIM)));
    }

    help_spans.extend([
        Span::styled(":", Style::default().fg(ACCENT)),
        Span::styled(" cmd  ", Style::default().fg(DIM)),
        Span::styled("?", Style::default().fg(ACCENT)),
        Span::styled(" help  ", Style::default().fg(DIM)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(DIM)),
    ]);

    let help = Line::from(help_spans);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_INACTIVE))
        .style(Style::default().bg(BG));
    let paragraph = Paragraph::new(help).block(block);
    f.render_widget(paragraph, area);
}

fn draw_help_overlay(f: &mut Frame) {
    let area = f.area();
    let block = Block::default()
        .title(" ? The Lore of sonos-palantir — Esc or ? to close ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  NAVIGATION", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  Tab        ", Style::default().fg(ACCENT)), Span::styled("Cycle panels — as the Fellowship moved between realms", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  ↑ / k      ", Style::default().fg(ACCENT)), Span::styled("Move up", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  ↓ / j      ", Style::default().fg(ACCENT)), Span::styled("Move down", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  Enter      ", Style::default().fg(ACCENT)), Span::styled("Play selected playlist on selected speaker", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  PLAYBACK", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  Space      ", Style::default().fg(ACCENT)), Span::styled("Pause / resume — even hobbits need rest", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  n          ", Style::default().fg(ACCENT)), Span::styled("Next track — onwards, to Rivendell", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  p          ", Style::default().fg(ACCENT)), Span::styled("Previous track — back to the Shire", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  + / =      ", Style::default().fg(ACCENT)), Span::styled("Volume up 5", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  -          ", Style::default().fg(ACCENT)), Span::styled("Volume down 5", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  v          ", Style::default().fg(ACCENT)), Span::styled("Set exact volume — speak your will", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  GROUPS", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  g          ", Style::default().fg(ACCENT)), Span::styled("Toggle group all speakers — assemble the Fellowship", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  PODCASTS", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  s          ", Style::default().fg(ACCENT)), Span::styled("Toggle source — Playlists / Podcasts", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  f / →      ", Style::default().fg(ACCENT)), Span::styled("Skip forward (when podcast playing)", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  b / ←      ", Style::default().fg(ACCENT)), Span::styled("Skip back (when podcast playing)", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  e          ", Style::default().fg(ACCENT)), Span::styled("Show full episode title", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :mark      ", Style::default().fg(ACCENT)), Span::styled("Toggle played/unplayed on selected episode", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  COMMAND MODE  (press : to enter)", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  :play <name> ", Style::default().fg(ACCENT)), Span::styled("Play a favorite — fuzzy matched", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :vol <0-100> ", Style::default().fg(ACCENT)), Span::styled("Set exact volume", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :group all   ", Style::default().fg(ACCENT)), Span::styled("Group all speakers", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :sleep <min> ", Style::default().fg(ACCENT)), Span::styled("Sleep timer — pause all after N minutes", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :reload      ", Style::default().fg(ACCENT)), Span::styled("Reload config — a wizard is never stale", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :source      ", Style::default().fg(ACCENT)), Span::styled("Toggle Playlists / Podcasts panel", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :podcast ref ", Style::default().fg(ACCENT)), Span::styled("Refresh podcast feeds", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :mark        ", Style::default().fg(ACCENT)), Span::styled("Toggle played / unplayed on episode", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  Tab          ", Style::default().fg(ACCENT)), Span::styled("Accept ghost text autocomplete suggestion", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  ?            ", Style::default().fg(ACCENT)), Span::styled("Toggle this help screen", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  q            ", Style::default().fg(ACCENT)), Span::styled("Quit — go back to the Shire", Style::default().fg(FG))]),
    ];

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

fn draw_episode_popup(f: &mut Frame, app: &App) {
    let ep = match app.selected_episode() {
        Some(ep) => ep,
        None => return,
    };

    let area = f.area();
    // Center a popup that's 60% wide, 5 rows tall
    let popup_w = (area.width * 60 / 100).max(30).min(area.width.saturating_sub(4));
    let popup_h: u16 = 5;
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    // Clear background
    f.render_widget(ratatui::widgets::Clear, popup_area);

    let block = Block::default()
        .title(" Episode — Esc to close ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let duration_str = format_time(ep.duration);
    let played = if ep.played == 1 { " (played)" } else { "" };

    let lines = vec![
        Line::from(vec![
            Span::styled("  ♫ ", Style::default().fg(PLAYING)),
            Span::styled(&ep.title, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(format!("    {}  {}", duration_str, played), Style::default().fg(DIM)),
        ]),
    ];

    let wrap = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(wrap, inner);
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
