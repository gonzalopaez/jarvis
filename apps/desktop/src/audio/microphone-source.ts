import type { AudioVisualizerReading, AudioVisualizerSource } from "./source";
import { normalizeAudioLevel } from "./source";

export class MicrophoneAudioVisualizerSource implements AudioVisualizerSource {
  readonly kind = "microphone" as const;
  private stream: MediaStream | null = null;
  private context: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private timer = 0;
  private emit: ((reading: AudioVisualizerReading) => void) | null = null;

  async start(emit: (reading: AudioVisualizerReading) => void): Promise<void> {
    if (this.stream) return;
    if (!navigator.mediaDevices?.getUserMedia) throw new Error("Microphone capture is unavailable");
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      video: false,
    });
    try {
      const context = new AudioContext({ latencyHint: "interactive" });
      const analyser = context.createAnalyser();
      analyser.fftSize = 256;
      analyser.smoothingTimeConstant = .72;
      context.createMediaStreamSource(stream).connect(analyser);
      this.stream = stream;
      this.context = context;
      this.analyser = analyser;
      this.emit = emit;
      this.sample();
    } catch (error) {
      stream.getTracks().forEach((track) => track.stop());
      throw error;
    }
  }

  mediaStream(): MediaStream | null {
    return this.stream;
  }

  stop(): void {
    window.clearTimeout(this.timer);
    this.timer = 0;
    this.stream?.getTracks().forEach((track) => track.stop());
    this.stream = null;
    this.analyser?.disconnect();
    this.analyser = null;
    void this.context?.close();
    this.context = null;
    this.emit?.({ source: "none", level: 0 });
    this.emit = null;
  }

  private sample = (): void => {
    if (!this.analyser || !this.emit) return;
    const samples = new Uint8Array(this.analyser.fftSize);
    this.analyser.getByteTimeDomainData(samples);
    let energy = 0;
    for (const value of samples) {
      const normalized = (value - 128) / 128;
      energy += normalized * normalized;
    }
    const rms = Math.sqrt(energy / samples.length);
    this.emit({ source: this.kind, level: normalizeAudioLevel(rms * 4.2) });
    this.timer = window.setTimeout(this.sample, document.hidden ? 500 : 50);
  };
}
