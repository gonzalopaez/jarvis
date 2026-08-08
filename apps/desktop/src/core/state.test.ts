import { describe, expect, it } from "vitest";
import { JarvisStore } from "./state";

describe("JarvisStore", () => {
  it("maps Codex states into the mocked agent status", () => {
    const store = new JarvisStore();
    store.setState("codex-executing", true);
    const snapshot = store.snapshot();
    expect(snapshot.state).toBe("codex-executing");
    expect(snapshot.agents.find((agent) => agent.id === "codex")?.state).toBe("active");
    expect(snapshot.activity[0].simulated).toBe(true);
  });

  it("keeps only a bounded activity history", () => {
    const store = new JarvisStore();
    for (let index = 0; index < 40; index += 1) store.addActivity("TEST", String(index));
    expect(store.snapshot().activity).toHaveLength(28);
  });
});
