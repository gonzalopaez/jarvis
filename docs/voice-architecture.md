# Voice architecture

Voice is a future backend service behind Jarvis Core:

    Desktop -> Jarvis Core -> Voice Service -> STT/TTS adapters

Desktop will not know Whisper, TTS provider, model, host or port details. Audio transport, retention, consent, redaction, authentication and latency budgets require explicit contracts before implementation. Temporary audio defaults to minimal retention and secure deletion where practical.
