# sonos-palantir TUI Visual Redesign — Design

**Goal:** Redesign the TUI for a sleek, modern aesthetic — "nvim but for Sonos" — while preserving all existing functionality.

**Architecture:** Pure UI layer changes in `tui/src/ui.rs` with minor additions to daemon response payloads (`source`, `quality` fields). No changes to event loop, API client, or command parsing.

---

## 1. Layout

**Outer structure** (4-row vertical):

```
╭─ Top Status Bar ──────────────────────────────────────────────╮
│ ● cthulhu  ▶ Track — Artist     VOL 45%  palantir:OK  Sonos:2│
╰───────────────────────────────────────────────────────────────╯
╭─ Rooms ──────────╮╭─ Now Playing ─────────────────────────────╮
│ ▸ cthulhu   ▶ 45 ││ Track Title                               │
│   [██████▌   ]   ││ Artist — Album                            │
│   family    ‖ 30 ││                                           │
│   [████▌     ]   ││ Source: Spotify · Quality: 320kbps OGG    │
│                  ││                                           │
│ GROUPED          ││ ═══════════●─────────── 2:14 / 4:02       │
│   cth + family   ││                                           │
├──────────────────┤│                                           │
│ Playlists        ││                                           │
│ ▸ Alt Wave       ││                                           │
│   Jazz Classics  ││                                           │
│   Lo-Fi Beats    ││                                           │
│   ...            ││                                           │
╰──────────────────╯╰──────────────────────────────────────────╯
╭─ Status / Command ────────────────────────────────────────────╮
│ :vol cthulhu 30                                               │
╰───────────────────────────────────────────────────────────────╯
  space:⏯  +/-:vol  v:set  n/p:trk  g:grp  ::cmd  ?:help
```

- 2-column main area (left: Rooms + Playlists stacked, right: Now Playing).
- Left column width: ~30% of terminal.
- Borders: `BorderType::Rounded` (`╭╮╰╯`) everywhere.

## 2. Rooms Panel

- Per-speaker row: `▸ alias STATE vol` (selected marker `▸`, state icon `▶`/`‖`/`■`, volume number).
- Below each speaker: colored volume bar using block gradient characters (`█▓▒░`), colored green→yellow→red as volume increases.
- **Groups as subsections**: `GROUPED` header line + `cth + family` member list (no box topology). Coordinator shown first.
- **Dynamic height**: Rooms takes only the rows it needs (speakers × 2 lines + group headers), Playlists gets all remaining space via `Constraint::Min(0)`.

## 3. Now Playing Panel

- Large track title, artist — album on second line.
- **Source/quality metadata line**: `Source: Spotify · Quality: 320kbps OGG` (requires daemon changes).
- **Segmented progress bar**: `═` filled, `─` unfilled, `●` playhead cursor. Time display: `2:14 / 4:02`.
- Art URI not rendered (terminal limitation); space used for clean typography.

## 4. Top Status Bar

Single line, left-to-right:
- `●` playing indicator (green when playing, yellow when paused, dim when stopped)
- Active speaker alias
- `▶`/`‖` + current track — artist (truncated to fit)
- `VOL XX%`
- `palantir:OK` or `palantir:ERR` daemon connection status
- `Sonos: N` speaker count

## 5. Visual Reskin

- **Borders**: `BorderType::Rounded` replacing current `BorderType::Plain`.
- **Colors**: Keep current dark palette (`Rgb(20,20,30)` bg, `Rgb(200,200,210)` fg, `Rgb(130,170,255)` accent). Refine with more contrast for active/inactive panels.
- **Active panel indicator**: Brighter border color on focused panel; dim border on unfocused.
- **Fix project name**: "sono-palantir" → "sonos-palantir" in splash screen and help overlay.

## 6. Daemon Changes

Add to `get_speaker_info()` response:
- `source`: string — extracted from track URI scheme (e.g., `"Spotify"`, `"Apple Music"`, `"Local Library"`, `"Line-In"`).
- `quality`: string — best-effort from transport metadata (e.g., `"320kbps OGG"`, `"256kbps AAC"`, or empty string if unavailable).

TUI `Speaker`/`Track` structs updated to deserialize these optional fields.

## 7. Preserved Functionality

All existing features remain unchanged:
- Panel cycling (Tab), speaker/playlist selection (j/k/arrows), Enter to play
- Space to toggle play/pause, +/- for volume, v for exact volume input
- : command mode with autocomplete (play, vol, group, ungroup, next, prev, sleep, reload)
- g to toggle group/ungroup all
- n/p for next/previous track
- Sleep timer
- Help overlay (?)
- Background 2s refresh polling
- Splash screen (with corrected name)
