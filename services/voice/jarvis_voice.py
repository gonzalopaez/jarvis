#!/usr/bin/env python3
"""Private, authenticated STT/TTS worker. It never logs audio, text, or credentials."""
from __future__ import annotations

import hmac
import io
import json
import os
import tempfile
import threading
import urllib.request
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from faster_whisper import WhisperModel
from piper import PiperVoice, SynthesisConfig

MAX_AUDIO_BYTES = 16 * 1024 * 1024
MAX_JSON_BYTES = 32 * 1024
MAX_TEXT_CHARS = 8_000


class Runtime:
    def __init__(self) -> None:
        credential = Path(os.environ.get("CREDENTIALS_DIRECTORY", "/etc/jarvis-voice")) / "voice-service-token"
        self.token = credential.read_text(encoding="utf-8").strip()
        if len(self.token) < 32:
            raise RuntimeError("voice credential is invalid")
        self.whisper = WhisperModel(
            os.environ.get("JARVIS_WHISPER_MODEL", "small"),
            device="cpu",
            compute_type="int8",
            cpu_threads=int(os.environ.get("JARVIS_WHISPER_THREADS", "4")),
        )
        self.fish_api_key = ""
        self.fish_voice_id = os.environ.get("JARVIS_FISH_VOICE_ID", "612b878b113047d9a770c069c8b4fdfe")
        if os.environ.get("JARVIS_TTS_PROVIDER", "piper").lower() == "fish":
            fish_key = credential.parent / "fish-api-key"
            self.fish_api_key = fish_key.read_text(encoding="utf-8").strip()
            if len(self.fish_api_key) < 20:
                raise RuntimeError("Fish Audio credential is invalid")
        self.piper = None if self.fish_api_key else PiperVoice.load(os.environ["JARVIS_PIPER_MODEL"])
        self.piper_config = SynthesisConfig(
            speaker_id=int(os.environ["JARVIS_PIPER_SPEAKER_ID"]) if os.environ.get("JARVIS_PIPER_SPEAKER_ID") else None,
            length_scale=float(os.environ.get("JARVIS_PIPER_LENGTH_SCALE", "1.05")),
            noise_scale=float(os.environ.get("JARVIS_PIPER_NOISE_SCALE", "0.55")),
            noise_w_scale=float(os.environ.get("JARVIS_PIPER_NOISE_W_SCALE", "0.8")),
        )
        self.lock = threading.Lock()


RUNTIME: Runtime


class Handler(BaseHTTPRequestHandler):
    server_version = "JarvisVoice/1"
    sys_version = ""

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def do_GET(self) -> None:
        if self.path != "/v1/health":
            self._json(HTTPStatus.NOT_FOUND, {"code": "NOT_FOUND"})
            return
        self._json(HTTPStatus.OK, {"status": "healthy", "version": "v1"})

    def do_POST(self) -> None:
        if not self._authorized():
            self._json(HTTPStatus.UNAUTHORIZED, {"code": "AUTHENTICATION_REQUIRED"})
            return
        if self.path == "/v1/transcribe":
            self._transcribe()
        elif self.path == "/v1/synthesize":
            self._synthesize()
        else:
            self._json(HTTPStatus.NOT_FOUND, {"code": "NOT_FOUND"})

    def _authorized(self) -> bool:
        value = self.headers.get("Authorization", "")
        return value.startswith("Bearer ") and hmac.compare_digest(value[7:], RUNTIME.token)

    def _body(self, maximum: int) -> bytes | None:
        try:
            length = int(self.headers.get("Content-Length", "-1"))
        except ValueError:
            length = -1
        if length < 1 or length > maximum:
            self._json(HTTPStatus.REQUEST_ENTITY_TOO_LARGE, {"code": "BODY_SIZE_REJECTED"})
            return None
        return self.rfile.read(length)

    def _transcribe(self) -> None:
        mime = self.headers.get_content_type()
        if mime not in {"audio/webm", "audio/ogg"}:
            self._json(HTTPStatus.UNSUPPORTED_MEDIA_TYPE, {"code": "AUDIO_TYPE_REJECTED"})
            return
        body = self._body(MAX_AUDIO_BYTES)
        if body is None:
            return
        suffix = ".webm" if mime == "audio/webm" else ".ogg"
        path = ""
        try:
            with tempfile.NamedTemporaryFile(prefix="jarvis-voice-", suffix=suffix, delete=False) as stream:
                stream.write(body)
                path = stream.name
            with RUNTIME.lock:
                segments, _info = RUNTIME.whisper.transcribe(path, language="es", beam_size=3, vad_filter=True)
                text = " ".join(segment.text.strip() for segment in segments).strip()
            if not text or len(text) > MAX_TEXT_CHARS:
                self._json(HTTPStatus.UNPROCESSABLE_ENTITY, {"code": "SPEECH_NOT_RECOGNIZED"})
                return
            self._json(HTTPStatus.OK, {"text": text})
        except Exception:
            self._json(HTTPStatus.SERVICE_UNAVAILABLE, {"code": "STT_FAILED"})
        finally:
            if path:
                Path(path).unlink(missing_ok=True)

    def _synthesize(self) -> None:
        body = self._body(MAX_JSON_BYTES)
        if body is None:
            return
        try:
            value = json.loads(body)
            if set(value) != {"text"} or not isinstance(value["text"], str):
                raise ValueError
            text = value["text"].strip()
            if not text or len(text) > MAX_TEXT_CHARS:
                raise ValueError
            if RUNTIME.fish_api_key:
                self._fish_synthesize(text)
                return
            output = io.BytesIO()
            import wave
            with wave.open(output, "wb") as wav:
                with RUNTIME.lock:
                    RUNTIME.piper.synthesize_wav(text, wav, syn_config=RUNTIME.piper_config)
            audio = output.getvalue()
            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", "audio/wav")
            self.send_header("Content-Length", str(len(audio)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(audio)
        except (ValueError, json.JSONDecodeError):
            self._json(HTTPStatus.BAD_REQUEST, {"code": "INVALID_REQUEST"})
        except Exception:
            self._json(HTTPStatus.SERVICE_UNAVAILABLE, {"code": "TTS_FAILED"})

    def _fish_synthesize(self, text: str) -> None:
        request_body = json.dumps({
            "text": text,
            "reference_id": RUNTIME.fish_voice_id,
            "temperature": 0.55,
            "top_p": 0.7,
            "prosody": {"speed": 0.92, "volume": 0, "normalize_loudness": True},
            "format": "wav",
            "sample_rate": 44100,
            "latency": "balanced",
            "max_new_tokens": 1024,
        }).encode()
        request = urllib.request.Request(
            os.environ.get("JARVIS_FISH_API_URL", "https://api.fish.audio/v1/tts"),
            data=request_body,
            headers={"Authorization": f"Bearer {RUNTIME.fish_api_key}", "Content-Type": "application/json", "model": os.environ.get("JARVIS_FISH_MODEL", "s2.1-pro-free")},
        )
        with urllib.request.urlopen(request, timeout=20) as response:
            audio = response.read(MAX_AUDIO_BYTES + 1)
        if len(audio) > MAX_AUDIO_BYTES or not audio:
            raise RuntimeError("Fish Audio response exceeded limit")
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", "audio/wav")
        self.send_header("Content-Length", str(len(audio)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(audio)

    def _json(self, status: HTTPStatus, value: dict[str, str]) -> None:
        body = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)


def main() -> None:
    global RUNTIME
    RUNTIME = Runtime()
    address = os.environ.get("JARVIS_VOICE_BIND", "127.0.0.1:4200")
    host, port = address.rsplit(":", 1)
    server = ThreadingHTTPServer((host, int(port)), Handler)
    server.daemon_threads = True
    server.serve_forever()


if __name__ == "__main__":
    main()
