import { describe, expect, it } from "vitest";
import { STATE_ANIMATION, STATE_PRESENTATION } from "./state-theme";
import type { JarvisState } from "./types";

describe("state presentation", () => {
  it("covers the canonical state machine without business-specific states", () => {
    const states: JarvisState[] = [
      "idle",
      "listening",
      "thinking",
      "routing",
      "executing",
      "speaking",
      "authorization-required",
      "warning",
      "error",
      "offline",
    ];
    expect(Object.keys(STATE_PRESENTATION).sort()).toEqual([...states].sort());
  });

  it("maps speaking to its reusable visual token", () => {
    expect(STATE_PRESENTATION.speaking.theme).toBe("speaking");
    expect(STATE_PRESENTATION.speaking.waveformActivity).toBeGreaterThan(0.8);
  });

  it("does not continuously animate idle or offline states", () => {
    expect(STATE_PRESENTATION.idle.animateWhenIdle).toBe(false);
    expect(STATE_PRESENTATION.offline.animateWhenIdle).toBe(false);
  });

  it("uses distinct functional motion profiles", () => {
    expect(STATE_ANIMATION.routing.routingActivity).toBe(1);
    expect(STATE_ANIMATION.executing.executionActivity).toBe(1);
    expect(STATE_ANIMATION.thinking.processingRotationSeconds)
      .toBeLessThan(STATE_ANIMATION.idle.processingRotationSeconds);
    expect(STATE_ANIMATION.offline.outerRotationSeconds).toBeGreaterThan(120);
    expect(STATE_ANIMATION["authorization-required"].executionActivity).toBe(0);
  });

  it("keeps speaking energetic without reusing the routing profile", () => {
    expect(STATE_PRESENTATION.speaking.theme).toBe("speaking");
    expect(STATE_ANIMATION.speaking.glowIntensity).toBe(1);
    expect(STATE_ANIMATION.speaking.routingActivity).toBeLessThan(.5);
  });
});
