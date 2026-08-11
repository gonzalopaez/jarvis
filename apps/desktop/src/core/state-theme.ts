import type { JarvisState } from "./types";

export type StateThemeToken =
  | "neutral"
  | "listening"
  | "thinking"
  | "active"
  | "speaking"
  | "authorization"
  | "warning"
  | "error"
  | "offline";

export interface StatePresentation {
  label: string;
  mode: string;
  theme: StateThemeToken;
  waveformActivity: number;
  animateWhenIdle: boolean;
}

export interface AnimationProfile {
  pulseSeconds: number;
  innerRotationSeconds: number;
  processingRotationSeconds: number;
  routingRotationSeconds: number;
  executionRotationSeconds: number;
  telemetryRotationSeconds: number;
  outerRotationSeconds: number;
  sweepSeconds: number;
  glowIntensity: number;
  routingActivity: number;
  executionActivity: number;
  orbitalActivity: number;
}

export const STATE_PRESENTATION: Record<JarvisState, StatePresentation> = {
  idle: {
    label: "SYSTEM // STANDBY",
    mode: "SIGNAL // AMBIENT",
    theme: "neutral",
    waveformActivity: 0.04,
    animateWhenIdle: false,
  },
  listening: {
    label: "VOICE // LISTENING",
    mode: "VOICE // INPUT",
    theme: "listening",
    waveformActivity: 0.85,
    animateWhenIdle: true,
  },
  thinking: {
    label: "JARVIS // THINKING",
    mode: "NEURAL // PROCESSING",
    theme: "thinking",
    waveformActivity: 0.34,
    animateWhenIdle: true,
  },
  routing: {
    label: "INTENT // ROUTING",
    mode: "INTENT // CLASSIFYING",
    theme: "active",
    waveformActivity: 0.46,
    animateWhenIdle: true,
  },
  executing: {
    label: "AGENT // EXECUTING",
    mode: "TASK // ACTIVITY",
    theme: "active",
    waveformActivity: 0.76,
    animateWhenIdle: true,
  },
  speaking: {
    label: "JARVIS // SPEAKING",
    mode: "JARVIS // OUTPUT",
    theme: "speaking",
    waveformActivity: 0.9,
    animateWhenIdle: true,
  },
  "authorization-required": {
    label: "AUTHORIZATION // REQUIRED",
    mode: "SECURITY // HOLD",
    theme: "authorization",
    waveformActivity: 0.2,
    animateWhenIdle: true,
  },
  warning: {
    label: "SYSTEM // WARNING",
    mode: "SYSTEM // ANOMALY",
    theme: "warning",
    waveformActivity: 0.62,
    animateWhenIdle: true,
  },
  error: {
    label: "SYSTEM // ERROR",
    mode: "SYSTEM // FAULT",
    theme: "error",
    waveformActivity: 0.72,
    animateWhenIdle: true,
  },
  offline: {
    label: "SYSTEM // OFFLINE",
    mode: "SIGNAL // LOST",
    theme: "offline",
    waveformActivity: 0.02,
    animateWhenIdle: false,
  },
};

export const STATE_ANIMATION: Record<JarvisState, AnimationProfile> = {
  idle: profile(8.5, 14, 34, 48, 56, 38, 58, 11, .45, .08, .05, .18),
  listening: profile(1.8, 8, 22, 30, 44, 28, 48, 7, .9, .25, .12, .55),
  thinking: profile(2.6, 6.5, 9, 15, 24, 22, 40, 8, .82, .48, .24, .7),
  routing: profile(2.2, 8, 13, 5.5, 20, 18, 36, 6, .88, 1, .32, .85),
  executing: profile(1.5, 7, 11, 9, 4.8, 16, 32, 6, 1, .65, 1, .95),
  speaking: profile(1.25, 7.5, 15, 22, 28, 24, 44, 7, 1, .28, .22, .72),
  "authorization-required": profile(2.8, 18, 40, 52, 120, 42, 55, 12, .72, .12, 0, .24),
  warning: profile(1.7, 13, 24, 32, 38, 29, 48, 9, .85, .24, .18, .48),
  error: profile(1.15, 28, 19, 70, 120, 55, 90, 16, .82, .08, 0, .2),
  offline: profile(16, 180, 180, 180, 180, 180, 180, 30, .08, 0, 0, 0),
};

function profile(
  pulseSeconds: number,
  innerRotationSeconds: number,
  processingRotationSeconds: number,
  routingRotationSeconds: number,
  executionRotationSeconds: number,
  telemetryRotationSeconds: number,
  outerRotationSeconds: number,
  sweepSeconds: number,
  glowIntensity: number,
  routingActivity: number,
  executionActivity: number,
  orbitalActivity: number,
): AnimationProfile {
  return {
    pulseSeconds, innerRotationSeconds, processingRotationSeconds, routingRotationSeconds,
    executionRotationSeconds, telemetryRotationSeconds, outerRotationSeconds, sweepSeconds,
    glowIntensity, routingActivity, executionActivity, orbitalActivity,
  };
}
