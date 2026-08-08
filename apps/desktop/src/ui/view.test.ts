import { describe, expect, it } from "vitest";
import { formatBytes, formatDuration, formatRate } from "./view";

describe("HUD formatters", () => {
  it("formats real telemetry values compactly", () => {
    expect(formatBytes(16 * 1024 ** 3)).toBe("16.0 GB");
    expect(formatRate(1024)).toBe("1 KB/s");
    expect(formatDuration(90061)).toBe("01D 01:01");
  });
});
