import { describe, expect, it } from "vitest";
import { JarvisStore } from "./state";

describe("JarvisStore", () => {
  it("starts with the real 7-agent roster and no placeholder-only agents", () => {
    const store = new JarvisStore();
    const ids = store.snapshot().agents.map((agent) => agent.id);
    expect(ids).toEqual(["core", "codex", "voice", "mcp", "n8n", "wazuh", "proxmox"]);
    expect(ids).not.toContain("memory");
    expect(ids).not.toContain("monitor");
    expect(ids).not.toContain("security");
  });

  it("marks Proxmox Agent as not instrumented rather than faking a live check", () => {
    const store = new JarvisStore();
    const proxmox = store.snapshot().agents.find((agent) => agent.id === "proxmox");
    expect(proxmox?.detail).toBe("NOT INSTRUMENTED");
  });

  it("updates the Wazuh Agent tile from a real agent.status.changed event", () => {
    const store = new JarvisStore();
    store.bus.emit("realtime.agent.changed", {
      id: "wazuh",
      label: "WAZUH AGENT",
      status: "healthy",
      agentStatus: "realtime",
      version: "adapter",
    });
    const wazuh = store.snapshot().agents.find((agent) => agent.id === "wazuh");
    expect(wazuh?.state).toBe("realtime");
    expect(wazuh?.simulated).toBe(false);
  });

  it("uses the normalized operational state model", () => {
    const store = new JarvisStore();
    store.setState("executing", true);
    const snapshot = store.snapshot();
    expect(snapshot.state).toBe("executing");
    expect(snapshot.agents.find((agent) => agent.id === "codex")?.state).toBe("offline");
    expect(snapshot.activity[0].simulated).toBe(true);
  });

  it("keeps only a bounded activity history", () => {
    const store = new JarvisStore();
    for (let index = 0; index < 40; index += 1) store.addActivity("TEST", String(index));
    expect(store.snapshot().activity).toHaveLength(28);
  });

  it("tracks the real Core health boundary", () => {
    const store = new JarvisStore();
    store.bus.emit("core.health.updated", {
      online: true,
      apiVersion: "v1",
      status: "ready",
      latencyMs: 12,
    });
    const core = store.snapshot().agents.find((agent) => agent.id === "core");
    expect(core?.state).toBe("ready");
    expect(core?.simulated).toBe(false);
    expect(core?.detail).toContain("V1");
  });

  it("normalizes the aggregate component inventory without mocks", () => {
    const store = new JarvisStore();
    store.bus.emit("core.health.updated", {
      online: true,
      apiVersion: "v1",
      status: "degraded",
      state: "idle",
      latencyMs: 8,
      components: [
        {
          id: "core",
          label: "JARVIS CORE",
          status: "healthy",
          agentStatus: "ready",
          version: "0.1.0",
          lastSeenMs: 1,
        },
        {
          id: "codex",
          label: "CODEX CORE",
          status: "unavailable",
          agentStatus: "offline",
          version: "not_connected",
          error: "not_connected",
        },
      ],
    });
    const snapshot = store.snapshot();
    expect(snapshot.agents.find((agent) => agent.id === "core")?.state).toBe("ready");
    expect(snapshot.agents.find((agent) => agent.id === "codex")?.state).toBe("offline");
    expect(snapshot.agents.find((agent) => agent.id === "codex")?.simulated).toBe(false);
  });
});
