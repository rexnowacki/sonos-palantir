# Todo

## Grouped speaker display

In the Speakers panel, show group membership inline:

```
► cthulhu (coordinator)
  family  (grouped)
```

A grouped speaker should show the coordinator's track info in the Now Playing panel when selected. Currently selecting `family` shows "Nothing playing" even though it's playing the same audio as `cthulhu`.

**What needs to change:**
- `ui.rs` speaker list rendering — detect `group_coordinator` and show a visual indicator
- `app.rs` `selected_speaker()` (or the Now Playing render) — when the selected speaker is a group follower, use the coordinator's track instead

---

## Now Playing for externally-started media

If music is already playing (started via the Sonos iOS app, Spotify, etc.), show that track info in the Now Playing panel immediately on launch. This should already work if the daemon's `/speakers` endpoint returns track data — investigate why it might not be showing up for grouped/follower speakers.

**What needs to change:**
- Possibly nothing if the root cause is the grouped display bug above
- If a speaker is PLAYING but `track` is null on the follower, fetch track from the coordinator in `get_speaker_info` in `sonos.py`

---

## Favorites refresh on startup

Currently the playlist list is fetched once on startup from `config.yaml` aliases. Add a call to `GET /favorites` on startup and merge any Sonos Favorites not already aliased into the playlist panel using their full title as the display name.

**What needs to change:**
- `api.rs` — already has `GET /favorites` endpoint available; add `get_favorites()` to `ApiClient`
- `app.rs` — store favorites alongside playlists, or merge them into the playlists list
- `main.rs` — call `get_favorites()` on startup and populate the list

---

## Set volume to a specific value

Allow typing an exact volume level (0–100) instead of only `+/-` stepping.

**What needs to change:**
- `app.rs` — add a volume input mode (e.g. a `volume_input: Option<String>` field)
- `main.rs` — on `v` keypress, enter volume input mode; capture digit keys; on `Enter` send the value; on `Esc` cancel
- `ui.rs` — when in volume input mode, show a small prompt (e.g. in the help bar or Now Playing panel): `Volume: [__]`
