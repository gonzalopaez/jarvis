import { normalizeAudioLevel } from "./source";

export class TtsPlayback {
  private context: AudioContext | null = null;
  private source: AudioBufferSourceNode | null = null;
  private frame = 0;

  async play(data: ArrayBuffer, emit: (level: number) => void): Promise<void> {
    this.stop();
    const context = new AudioContext();
    this.context = context;
    const buffer = await context.decodeAudioData(data.slice(0));
    const source = context.createBufferSource();
    const analyser = context.createAnalyser();
    analyser.fftSize = 256;
    analyser.smoothingTimeConstant = 0.72;
    source.buffer = buffer;
    source.connect(analyser);
    analyser.connect(context.destination);
    this.source = source;
    const samples = new Uint8Array(analyser.frequencyBinCount);
    const sample = (): void => {
      analyser.getByteTimeDomainData(samples);
      let energy = 0;
      for (const value of samples) {
        const centered = (value - 128) / 128;
        energy += centered * centered;
      }
      emit(normalizeAudioLevel(Math.sqrt(energy / samples.length) * 4));
      this.frame = requestAnimationFrame(sample);
    };
    source.start();
    sample();
    await new Promise<void>((resolve) => { source.onended = () => resolve(); });
    this.stop();
    emit(0);
  }

  stop(): void {
    cancelAnimationFrame(this.frame);
    this.frame = 0;
    try { this.source?.stop(); } catch { /* already stopped */ }
    this.source?.disconnect();
    this.source = null;
    void this.context?.close();
    this.context = null;
  }
}
