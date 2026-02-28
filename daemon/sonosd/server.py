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


@app.on_event("startup")
def startup():
    global manager
    config_path = Path(__file__).parent.parent / "config.yaml"
    with open(config_path) as f:
        config = yaml.safe_load(f)
    manager = SonosManager(config)


@app.get("/speakers")
def get_speakers():
    manager.refresh()
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
    return {"playlists": manager._playlist_map}


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


def main():
    config_path = Path(__file__).parent.parent / "config.yaml"
    with open(config_path) as f:
        config = yaml.safe_load(f)
    host = config.get("host", "127.0.0.1")
    port = config.get("port", 9271)
    uvicorn.run(app, host=host, port=port)
