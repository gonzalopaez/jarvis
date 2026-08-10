# Voice transport

The browser is the only endpoint component that captures and plays audio. Capture starts only after an authenticated operator presses the microphone control. Stopping capture releases every `MediaStreamTrack` and closes its `AudioContext`.

`GET /ws/voice` is a same-origin, session-authenticated WebSocket behind Nginx. Nginx permits one concurrent voice connection per client. The Core accepts only `audio/webm;codecs=opus` or `audio/ogg;codecs=opus`, with a 64 KiB frame limit, a 16 MiB session limit, and a 120 second deadline.

Client start message:

```json
{"version":"v1","type":"voice.session.start","session_id":"UUID","mime_type":"audio/webm;codecs=opus"}
```

Binary Opus fragments follow only after `voice.session.ready`. The client ends with:

```json
{"version":"v1","type":"voice.session.stop","session_id":"UUID"}
```

Audio is not logged or retained by Core. After capture stops, the production pipeline remains entirely server-side:

Production voice follows a server-owned path:

```text
Browser microphone (Opus) -> authenticated WSS /ws/voice -> JARVIS Core
  -> private Voice Service (faster-whisper, CPU/int8)
  -> private LiteLLM model gateway
  -> private Voice Service (Piper es_AR)
  -> browser WAV playback + real output analyser
```

The browser never receives service credentials and cannot access Voice Service or LiteLLM directly. Audio is bounded to 16 MiB/120 seconds, remains in memory in Core, and the Voice Service deletes its temporary input after transcription. Neither audio nor transcript content is logged.

Voice Service exposes only `GET /v1/health`, authenticated `POST /v1/transcribe`, and authenticated `POST /v1/synthesize` on the controlled server network. It has no Nginx route or internal DNS entry intended for users.

The frontend drives `LISTENING`, `THINKING`, and `SPEAKING` from protocol events. During playback, Web Audio measures the actual TTS signal and feeds `voice.output.level`; there is no production fake waveform.

Deployment examples are in `deploy/systemd/jarvis-voice.service`, `jarvis-voice.env.example`, and the JARVIS Core systemd unit. Raw tokens must be provisioned outside Git. Core uses a scoped LiteLLM virtual key restricted to the selected conversational model.
