# sono-palantir — Dream Features

> *"All that is gold does not glitter, not all those who wander are lost..."*
> *...but a good TUI should never make you search for the volume control.*

```
        /\
       /  \
      / /\ \
     / /  \ \
    /_/ == \_\
    |  (o)(o) |
    |    __   |
    |   /  \  |
    \   \__/  /
     \       /
      \_____/
  YOU SHALL NOT SKIP
```

---

## Visual & Interface

### Palantir Mode (The Seeing Stone)
Replace the standard layout with a large central "orb" — a circular ASCII art visualization
that pulses/animates with the beat or track progress. Activate with `O`. Very dramatic.
Speakers glow in the orb when active.

### LOTR Color Theme
A dark mystical palette: deep midnight blue background, gold/amber for active elements,
mithril silver for inactive, ember red for errors ("Into the fire!"). Optional "Shire"
light theme for daytime use. Configurable in `config.yaml`.

### Ornate Unicode Borders
Swap plain box-drawing for double-line or custom rune-like characters on panel borders.
Header: `╔═══ The Fellowship ═══╗` style. Speakers panel becomes "The Company."

### Album Art in the Terminal
Render album art using half-block characters (▄▀) or sixel (if the terminal supports it).
A 20×10 pixel image alongside track info would be stunning. Use `chafa` or inline sixel.

### Waveform / VU Meter Animation
An animated bar in the Now Playing panel that bounces with simulated activity while
a track plays — even a fake one based on track position ticks looks slick.
`▁▂▃▄▅▆▇█▇▆▅▄▃▂▁`

---

## Playback & Control

### Queue Management
Show the current play queue. Navigate it, reorder tracks, remove items.
`Q` to open, `d` to delete, `u`/`d` to move up/down.

### Sleep Timer
"Cast a sleeping spell." Set a timer (15, 30, 60 min) after which all speakers pause.
Show a countdown in the status bar. Mapped to `S`.

### Crossfade Indicator
If Sonos crossfade is enabled, show a ⟿ icon. Toggle it from the TUI.

### Volume Sync Across Group
When grouped, show each speaker's volume individually and offer a "normalize" command
to set them all to the same level. `N` to normalize.

### Alarm / Wake-Up Scheduler
"The dawn will take you all!" — schedule a start time + playlist + volume ramp.
Runs via a background cron-style task in the daemon.

---

## Discovery & Library

### Favorites Refresh + Search
Pull all Sonos Favorites on startup. Add `/` to fuzzy-search playlists and favorites
inline — type to filter the playlist list in real time.

### Recently Played
Track the last 10 (playlist, speaker) combos in `~/.sonos_history`. Browse with `H`.
"The road goes ever on and on..."

### Sonos Radio Support
Tunein/Sonos Radio stations alongside favorites. Detect them from the library and
list under a "Radio" section.

---

## Multi-Room / Group UX

### Group Topology Visualizer
A small ASCII map showing which speakers are grouped together:
```
  ╔═ Fellowship ══╗
  │ ► cthulhu     │
  │   family      │
  ╚═══════════════╝
    hermit (solo)
```

### Per-Speaker Now Playing
When multiple groups exist, show each group's currently playing track in a
scrollable multi-pane view. "Many eyes, one vision."

### Stereo Pair Support
Detect stereo-paired speakers and show them as a single logical unit with an icon.

---

## Wizard-Tier Features

### ASCII Gandalf on Idle
If nothing plays for 5 minutes, Gandalf appears in the Now Playing panel with a
rotating quote from the books. Pressing any key dismisses him ("He is never late...").

### "Speak, Friend" Command Mode
Press `:` to enter a command prompt at the bottom (vim-style).
Commands: `:play altwave`, `:vol 40`, `:group all`, `:sleep 30`, `:next`, etc.
"Speak, friend, and enter."

### Last.fm Scrobbling
POST to Last.fm API when a track hits 50% played. Configurable in `config.yaml`.
"Your listening history, preserved in the archives of Minas Tirith."

### mpris / macOS Media Key Support
Hook into MPRIS (Linux) or Now Playing daemon (macOS) so hardware media keys
(play/pause, next, volume) control sono-palantir directly.

### Config Hot-Reload
Watch `config.yaml` for changes. Reload aliases and playlists without restarting.
"A wizard is never outdated, nor is he ever stale."

### Notification on Track Change
Send a desktop notification (macOS `osascript` / Linux `notify-send`) when the
track changes, with title, artist, and album art URL.

---

## Polish

### Startup Splash
A one-second splash on launch:
```
  S O N O - P A L A N T I R
  ══════════════════════════
  Seeing through sound...
```

### Error Messages with Character
- Connection refused: *"The gates of Moria are sealed."*
- Speaker not found: *"Not all those who wander are found in this network."*
- UPnP error: *"Even the very wise cannot see all ends."*
- Volume at 100: *"You shall not pass... 100."*

### Help Screen (`?`)
Full-screen keybinding reference with LOTR flavor text. Toggle with `?`.
