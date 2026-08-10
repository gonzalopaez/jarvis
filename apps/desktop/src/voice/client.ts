import type { JarvisRuntimeClient } from "../runtime/client";

const MAX_CHUNK_BYTES = 64 * 1024;
const MAX_BUFFERED_BYTES = 512 * 1024;
const START_TIMEOUT_MS = 5_000;
const MAX_SESSION_MS = 120_000;

export interface VoiceClientCallbacks {
  unavailable(code: string): void;
  failed(message: string): void;
  transcript(text: string): void;
  response(text: string): void;
  output(audio: ArrayBuffer): void;
  processing(): void;
}

export class VoiceCaptureClient {
  private socket: WebSocket | null = null;
  private recorder: MediaRecorder | null = null;
  private sessionId: string | null = null;
  private sendQueue: Promise<void> = Promise.resolve();
  private durationTimer = 0;
  private awaitingAudio = false;

  constructor(private readonly runtime: JarvisRuntimeClient, private readonly callbacks: VoiceClientCallbacks) {}

  async start(stream: MediaStream): Promise<void> {
    if (this.socket || this.recorder) return;
    const mimeType = preferredVoiceMimeType();
    if (!mimeType) throw new Error("Opus recording is unavailable");
    const sessionId = crypto.randomUUID();
    const socket = new WebSocket(this.runtime.voiceWebsocketUrl());
    socket.binaryType = "arraybuffer";
    this.socket = socket;
    this.sessionId = sessionId;
    await new Promise<void>((resolve, reject) => {
      const timer = window.setTimeout(() => reject(new Error("Voice gateway timeout")), START_TIMEOUT_MS);
      const fail = (): void => {
        window.clearTimeout(timer);
        reject(new Error("Voice gateway unavailable"));
      };
      socket.onerror = fail;
      socket.onclose = fail;
      socket.onopen = () => socket.send(JSON.stringify({
        version: "v1", type: "voice.session.start", session_id: sessionId, mime_type: mimeType,
      }));
      socket.onmessage = (event) => {
        const message = parseVoiceMessage(event.data);
        if (message?.type !== "voice.session.ready") return fail();
        window.clearTimeout(timer);
        resolve();
      };
    }).catch((error) => {
      socket.close();
      this.socket = null;
      this.sessionId = null;
      throw error;
    });

    socket.onerror = () => this.callbacks.failed("VOICE_GATEWAY_ERROR");
    socket.onclose = () => { this.socket = null; };
    socket.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        if (!this.awaitingAudio || event.data.byteLength > 8 * 1024 * 1024) {
          this.callbacks.failed("VOICE_OUTPUT_REJECTED");
          return;
        }
        this.awaitingAudio = false;
        this.callbacks.output(event.data);
        return;
      }
      const message = parseVoiceMessage(event.data);
      if (message?.type === "voice.session.unavailable" && typeof message.code === "string") {
        this.callbacks.unavailable(message.code);
      } else if (message?.type === "voice.session.failed" && typeof message.code === "string") {
        this.callbacks.failed(message.code);
      } else if (message?.type === "voice.transcript" && typeof message.text === "string") {
        this.callbacks.transcript(message.text);
      } else if (message?.type === "voice.response" && typeof message.text === "string") {
        this.callbacks.response(message.text);
      } else if (message?.type === "voice.output.start" && message.mime_type === "audio/wav") {
        this.awaitingAudio = true;
      }
    };
    const recorder = new MediaRecorder(stream, { mimeType, audioBitsPerSecond: 32_000 });
    this.recorder = recorder;
    recorder.ondataavailable = (event) => this.enqueue(event.data);
    recorder.onerror = () => this.callbacks.failed("VOICE_RECORDER_ERROR");
    recorder.start(250);
    this.durationTimer = window.setTimeout(() => void this.stop(), MAX_SESSION_MS);
  }

  async stop(): Promise<void> {
    window.clearTimeout(this.durationTimer);
    const recorder = this.recorder;
    this.recorder = null;
    if (recorder && recorder.state !== "inactive") {
      await new Promise<void>((resolve) => {
        recorder.addEventListener("stop", () => resolve(), { once: true });
        recorder.stop();
      });
    }
    await this.sendQueue;
    const socket = this.socket;
    const sessionId = this.sessionId;
    if (socket?.readyState === WebSocket.OPEN && sessionId) {
      this.callbacks.processing();
      socket.send(JSON.stringify({ version: "v1", type: "voice.session.stop", session_id: sessionId }));
    }
    this.sessionId = null;
  }

  private enqueue(blob: Blob): void {
    if (!blob.size) return;
    if (blob.size > MAX_CHUNK_BYTES) {
      this.callbacks.failed("VOICE_CHUNK_TOO_LARGE");
      void this.stop();
      return;
    }
    this.sendQueue = this.sendQueue.then(async () => {
      const socket = this.socket;
      if (!socket || socket.readyState !== WebSocket.OPEN || socket.bufferedAmount > MAX_BUFFERED_BYTES) {
        throw new Error("Voice backpressure limit reached");
      }
      socket.send(await blob.arrayBuffer());
    }).catch(() => this.callbacks.failed("VOICE_TRANSPORT_BACKPRESSURE"));
  }
}

export function preferredVoiceMimeType(): string | null {
  if (typeof MediaRecorder === "undefined") return null;
  return ["audio/webm;codecs=opus", "audio/ogg;codecs=opus"]
    .find((mime) => MediaRecorder.isTypeSupported(mime)) ?? null;
}

function parseVoiceMessage(data: unknown): { type?: unknown; code?: unknown; text?: unknown; mime_type?: unknown } | null {
  if (typeof data !== "string" || data.length > 32 * 1024) return null;
  try {
    const value: unknown = JSON.parse(data);
    return value && typeof value === "object" && !Array.isArray(value)
      ? value as { type?: unknown; code?: unknown; text?: unknown; mime_type?: unknown } : null;
  } catch {
    return null;
  }
}
