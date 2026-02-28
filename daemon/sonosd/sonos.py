import soco
from typing import Optional


class SonosManager:
    """Manages speaker discovery and provides control methods."""

    def __init__(self, config: dict):
        self.config = config
        self._speakers: dict[str, soco.SoCo] = {}
        self._alias_map: dict[str, str] = config.get("speakers", {})
        self._reverse_alias: dict[str, str] = {v: k for k, v in self._alias_map.items()}
        self._playlist_map: dict[str, str] = config.get("playlists", {})
        self.refresh()

    def refresh(self) -> None:
        """Re-discover speakers on the network."""
        discovered = soco.discover(timeout=5)
        if discovered:
            self._speakers = {sp.player_name: sp for sp in discovered}

    def get_speaker(self, name_or_alias: str) -> soco.SoCo:
        """Resolve alias or name to a SoCo instance."""
        real_name = self._alias_map.get(name_or_alias, name_or_alias)
        if real_name in self._speakers:
            return self._speakers[real_name]
        raise KeyError(f"Speaker not found: {name_or_alias}")

    def get_all_speakers(self) -> dict[str, soco.SoCo]:
        return self._speakers

    def get_speaker_info(self, speaker: soco.SoCo) -> dict:
        """Build the full status dict for a speaker."""
        info = speaker.get_current_transport_info()
        track_info = speaker.get_current_track_info()

        track = None
        if track_info.get("title"):
            track = {
                "title": track_info.get("title", ""),
                "artist": track_info.get("artist", ""),
                "album": track_info.get("album", ""),
                "duration": _parse_duration(track_info.get("duration", "0:00:00")),
                "position": _parse_duration(track_info.get("position", "0:00:00")),
                "art_uri": track_info.get("album_art", ""),
            }

        coordinator = speaker.group.coordinator.player_name if speaker.group else None

        return {
            "name": speaker.player_name,
            "alias": self._reverse_alias.get(speaker.player_name),
            "ip": speaker.ip_address,
            "volume": speaker.volume,
            "muted": speaker.mute,
            "state": info.get("current_transport_state", "UNKNOWN"),
            "group_coordinator": coordinator,
            "track": track,
        }

    def play_favorite(self, speaker: soco.SoCo, favorite_name: str) -> None:
        """Play a Sonos Favorite by exact name or alias."""
        resolved = self._playlist_map.get(favorite_name, favorite_name)

        favorites = speaker.music_library.get_sonos_favorites()
        match = None
        for fav in favorites:
            if fav.title.lower() == resolved.lower():
                match = fav
                break

        if not match:
            available = [f.title for f in favorites]
            raise KeyError(
                f"Favorite '{resolved}' not found. Available: {available}"
            )

        uri = match.reference.get_uri()
        meta = match.resource_meta_data
        speaker.play_uri(uri, meta)

    def group_speakers(self, names_or_aliases: list[str]) -> soco.SoCo:
        """Group speakers. First becomes coordinator."""
        if names_or_aliases == ["all"]:
            speakers = list(self._speakers.values())
        else:
            speakers = [self.get_speaker(n) for n in names_or_aliases]

        coordinator = speakers[0]
        for sp in speakers[1:]:
            sp.join(coordinator)
        return coordinator

    def ungroup(self, name_or_alias: str | None = None) -> None:
        """Ungroup a specific speaker, or all."""
        if name_or_alias is None or name_or_alias == "all":
            for sp in self._speakers.values():
                sp.unjoin()
        else:
            self.get_speaker(name_or_alias).unjoin()


def _parse_duration(time_str: str) -> int:
    """Parse 'H:MM:SS' to total seconds."""
    parts = time_str.split(":")
    if len(parts) == 3:
        try:
            return int(parts[0]) * 3600 + int(parts[1]) * 60 + int(parts[2])
        except ValueError:
            return 0
    return 0
