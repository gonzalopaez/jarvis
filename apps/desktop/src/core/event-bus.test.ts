import { describe, expect, it, vi } from "vitest";
import { EventBus } from "./event-bus";

interface TestEvents {
  "status.changed": { ready: boolean };
  message: string;
}

describe("EventBus", () => {
  it("delivers typed events and supports unsubscribe", () => {
    const bus = new EventBus<TestEvents>();
    const listener = vi.fn();
    const unsubscribe = bus.on("status.changed", listener);
    bus.emit("status.changed", { ready: true });
    unsubscribe();
    bus.emit("status.changed", { ready: false });
    expect(listener).toHaveBeenCalledOnce();
    expect(listener).toHaveBeenCalledWith({ ready: true });
  });
});
