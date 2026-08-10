export type AudioVisualizerSourceKind = "none" | "microphone" | "tts" | "dev";

export interface AudioVisualizerReading {
  source: AudioVisualizerSourceKind;
  level: number;
}

export interface AudioVisualizerSource {
  readonly kind: AudioVisualizerSourceKind;
  start(emit: (reading: AudioVisualizerReading) => void): Promise<void> | void;
  stop(): void;
}

export function normalizeAudioLevel(level: number): number {
  return Math.max(0, Math.min(Number.isFinite(level) ? level : 0, 1));
}

/** Default production source until microphone or TTS analysers are explicitly connected. */
export class SilentAudioVisualizerSource implements AudioVisualizerSource {
  readonly kind = "none" as const;
  start(emit: (reading: AudioVisualizerReading) => void): void {
    emit({ source: this.kind, level: 0 });
  }
  stop(): void {}
}
