from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
import uvicorn
import yaml
from pathlib import Path
from soco.exceptions import SoCoUPnPException
from .sonos import SonosManager

app = FastAPI(title="sonosd")
manager: SonosManager = None


class PlayRequest(BaseModel):
    speaker: str
    playlist: str


class SpeakerRequest(BaseModel):
    speaker: str = "all"


class VolumeRequest(BaseModel):
    speaker: str
    volume: int


class GroupRequest(BaseModel):
    speakers: list[str]


class PlayUriRequest(BaseModel):
    speaker: str
    uri: str
    title: str = ""


class SkipRequest(BaseModel):
    speaker: str
    seconds: int


class SeekRequest(BaseModel):
    speaker: str
    position: int


class EpisodeProgressRequest(BaseModel):
    episode_id: str
    position: int
    played: bool = False


podcast_manager = None  # PodcastManager, set on startup


@app.on_event("startup")
async def startup():
    global manager, podcast_manager
    config_path = Path(__file__).parent.parent / "config.yaml"
    with open(config_path) as f:
        config = yaml.safe_load(f)
    manager = SonosManager(config)

    from .podcast import PodcastManager
    podcasts = config.get("podcasts", {})
    podcast_manager = PodcastManager(
        podcasts=podcasts,
        skip_forward=config.get("podcast_skip_forward", 30),
        skip_back=config.get("podcast_skip_back", 10),
        refresh_minutes=config.get("podcast_refresh_minutes", 30),
    )
    await podcast_manager.init_db()
    if podcasts:
        await podcast_manager.refresh_all_feeds()
        podcast_manager.start_background_refresh()


@app.get("/speakers")
def get_speakers():
    speakers = []
    for name, sp in manager.get_all_speakers().items():
        try:
            speakers.append(manager.get_speaker_info(sp))
        except Exception:
            speakers.append({"name": name, "error": "unreachable"})
    return {"speakers": speakers}


@app.get("/favorites")
def get_favorites():
    speakers = list(manager.get_all_speakers().values())
    if not speakers:
        raise HTTPException(404, "No speakers found")
    favs = speakers[0].music_library.get_sonos_favorites()
    return {"favorites": [{"title": f.title} for f in favs]}


@app.get("/playlists")
def get_playlists():
    return {"playlists": manager.get_playlists_map()}


@app.post("/reload")
async def reload_config():
    manager.reload_config()
    if podcast_manager is not None:
        podcast_manager.podcasts = manager.config.get("podcasts", {})
        podcast_manager.skip_forward = manager.config.get("podcast_skip_forward", 30)
        podcast_manager.skip_back = manager.config.get("podcast_skip_back", 10)
    return {"status": "reloaded"}


@app.get("/config")
def get_config():
    raw = manager.config.get("playlist_sort", "alphabetical")
    sort = raw if raw in ("alphabetical", "popularity") else "alphabetical"
    return {"playlist_sort": sort}


@app.post("/play")
def play(req: PlayRequest):
    try:
        if req.speaker == "all":
            speaker = manager.group_speakers(["all"])
        else:
            speaker = manager.get_speaker(req.speaker)
        manager.play_favorite(speaker, req.playlist)
        return {"status": "playing", "speaker": speaker.player_name, "playlist": req.playlist}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/pause")
def pause(req: SpeakerRequest):
    try:
        if req.speaker == "all":
            for sp in manager.get_all_speakers().values():
                sp.pause()
        else:
            manager.get_coordinator(req.speaker).pause()
        return {"status": "paused"}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/resume")
def resume(req: SpeakerRequest):
    try:
        if req.speaker == "all":
            for sp in manager.get_all_speakers().values():
                sp.play()
        else:
            manager.get_coordinator(req.speaker).play()
        return {"status": "resumed"}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/stop")
def stop(req: SpeakerRequest):
    try:
        if req.speaker == "all":
            for sp in manager.get_all_speakers().values():
                sp.stop()
        else:
            manager.get_coordinator(req.speaker).stop()
        return {"status": "stopped"}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/volume")
def set_volume(req: VolumeRequest):
    try:
        vol = max(0, min(100, req.volume))
        if req.speaker == "all":
            for sp in manager.get_all_speakers().values():
                sp.volume = vol
        else:
            manager.get_speaker(req.speaker).volume = vol
        return {"status": "ok", "volume": vol}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/group")
def group(req: GroupRequest):
    try:
        coordinator = manager.group_speakers(req.speakers)
        return {"status": "grouped", "coordinator": coordinator.player_name}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/ungroup")
def ungroup(req: SpeakerRequest):
    manager.ungroup(req.speaker)
    return {"status": "ungrouped"}


@app.post("/next")
def next_track(req: SpeakerRequest):
    try:
        manager.get_coordinator(req.speaker).next()
        return {"status": "ok"}
    except KeyError as e:
        raise HTTPException(404, str(e))
    except SoCoUPnPException as e:
        raise HTTPException(422, str(e))


@app.post("/previous")
def prev_track(req: SpeakerRequest):
    try:
        manager.get_coordinator(req.speaker).previous()
        return {"status": "ok"}
    except KeyError as e:
        raise HTTPException(404, str(e))
    except SoCoUPnPException as e:
        raise HTTPException(422, str(e))


@app.get("/podcasts")
async def get_podcasts():
    if podcast_manager is None:
        return {"podcasts": []}
    podcasts = await podcast_manager.list_podcasts()
    return {"podcasts": podcasts}


@app.get("/podcasts/{alias}/episodes")
async def get_podcast_episodes(alias: str):
    if podcast_manager is None:
        return {"episodes": []}
    episodes = await podcast_manager.list_episodes(alias)
    return {"episodes": episodes}


@app.post("/play_uri")
def play_uri(req: PlayUriRequest):
    try:
        speaker = manager.get_coordinator(req.speaker)
        speaker.play_uri(req.uri, title=req.title)
        return {"status": "playing", "uri": req.uri}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/skip")
def skip(req: SkipRequest):
    try:
        speaker = manager.get_coordinator(req.speaker)
        track_info = speaker.get_current_track_info()
        from .sonos import _parse_duration
        current = _parse_duration(track_info.get("position", "0:00:00"))
        duration = _parse_duration(track_info.get("duration", "0:00:00"))
        target = max(0, min(current + req.seconds, duration))
        h = target // 3600
        m = (target % 3600) // 60
        s = target % 60
        speaker.seek(f"{h}:{m:02}:{s:02}")
        return {"status": "ok", "position": target}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/seek")
def seek(req: SeekRequest):
    try:
        speaker = manager.get_coordinator(req.speaker)
        pos = max(0, req.position)
        h = pos // 3600
        m = (pos % 3600) // 60
        s = pos % 60
        speaker.seek(f"{h}:{m:02}:{s:02}")
        return {"status": "ok", "position": pos}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/podcasts/episode/progress")
async def save_episode_progress(req: EpisodeProgressRequest):
    if podcast_manager is None:
        raise HTTPException(503, "Podcast manager not initialized")
    await podcast_manager.save_progress(req.episode_id, req.position, req.played)
    return {"status": "saved"}


@app.post("/podcasts/refresh")
async def refresh_podcasts():
    if podcast_manager is None:
        raise HTTPException(503, "Podcast manager not initialized")
    await podcast_manager.refresh_all_feeds()
    return {"status": "refreshed"}


def main():
    config_path = Path(__file__).parent.parent / "config.yaml"
    with open(config_path) as f:
        config = yaml.safe_load(f)
    host = config.get("host", "127.0.0.1")
    port = config.get("port", 9271)
    uvicorn.run(app, host=host, port=port)
