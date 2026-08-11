import { afterEach, describe, expect, it, vi } from "vitest";
import { preferredVoiceMimeType } from "./client";

describe("voice capture negotiation", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("selects only an explicitly supported Opus container", () => {
    vi.stubGlobal("MediaRecorder", class {
      static isTypeSupported(mime: string): boolean { return mime === "audio/ogg;codecs=opus"; }
    });
    expect(preferredVoiceMimeType()).toBe("audio/ogg;codecs=opus");
  });

  it("fails closed when Opus recording is unavailable", () => {
    vi.stubGlobal("MediaRecorder", class {
      static isTypeSupported(): boolean { return false; }
    });
    expect(preferredVoiceMimeType()).toBeNull();
  });
});
