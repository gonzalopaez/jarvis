import { describe, expect, it } from "vitest";
import { normalizeAudioLevel, SilentAudioVisualizerSource } from "./source";

describe("AudioVisualizerSource", () => {
  it("clamps invalid and out-of-range levels", () => {
    expect(normalizeAudioLevel(-1)).toBe(0);
    expect(normalizeAudioLevel(.42)).toBe(.42);
    expect(normalizeAudioLevel(9)).toBe(1);
    expect(normalizeAudioLevel(Number.NaN)).toBe(0);
  });

  it("defaults production visualization to an explicit silent source", () => {
    const readings: unknown[] = [];
    new SilentAudioVisualizerSource().start((reading) => readings.push(reading));
    expect(readings).toEqual([{ source: "none", level: 0 }]);
  });
});
