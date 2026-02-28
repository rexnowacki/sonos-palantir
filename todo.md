# Todo

## ~~Grouped speaker display~~ ✓

In the Speakers panel, show group membership inline. **Done:** `◈` marks the coordinator, `↳` marks followers in the Speakers panel.

---

## ~~Now Playing for externally-started media~~ ✓

If music is already playing (started via the Sonos iOS app, Spotify, etc.), show that track info in the Now Playing panel. **Done:** Follower speakers now fall back to the coordinator's track info in `get_speaker_info` when their own track is empty.

---

## ~~Favorites refresh on startup~~ ✓

**Done:** `get_favorites()` added to `ApiClient`; any Sonos Favorite not already aliased in `config.yaml` is merged into the playlists panel on startup using its full title.

---

## ~~Set volume to a specific value~~ ✓

**Done:** Press `v` to enter volume input mode, type 0–3 digits, `Enter` to apply (clamped to 100), `Esc` to cancel. Help bar shows `Vol: [##▌]` prompt while in input mode.

---
