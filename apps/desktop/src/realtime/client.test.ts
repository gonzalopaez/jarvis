import { afterEach, describe, expect, it, vi } from "vitest";
import { EventBus } from "../core/event-bus";
import type { AppEvents } from "../core/types";
import type { JarvisRuntimeClient } from "../runtime/client";
import { RealtimeClient } from "./client";

class FakeSocket {
  readyState = 0;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  close = vi.fn();
}

afterEach(() => vi.unstubAllGlobals());

describe("RealtimeClient", () => {
  it("does not open a socket without an authenticated session", async () => {
    installDom();
    const bus = new EventBus<AppEvents>();
    const unavailable = vi.fn();
    bus.on("realtime.unavailable", unavailable);
    const factory = vi.fn(() => new FakeSocket());
    const client = new RealtimeClient(bus, runtime(false), factory);
    await client.start();
    expect(factory).not.toHaveBeenCalled();
    expect(unavailable).toHaveBeenCalledWith({ reason: "AUTHENTICATED_SESSION_REQUIRED" });
    client.stop();
  });

  it("connects when start is retried after browser authentication", async () => {
    installDom();
    const bus = new EventBus<AppEvents>();
    let authenticated = false;
    const fakeRuntime = runtime(false);
    fakeRuntime.hasSession = vi.fn(async () => authenticated);
    const factory = vi.fn(() => new FakeSocket());
    const client = new RealtimeClient(bus, fakeRuntime, factory);
    await client.start();
    expect(factory).not.toHaveBeenCalled();
    authenticated = true;
    await client.start();
    expect(factory).toHaveBeenCalledOnce();
    client.stop();
  });

  it("validates and forwards the initial realtime snapshot", async () => {
    installDom();
    const bus = new EventBus<AppEvents>();
    const health = vi.fn();
    bus.on("core.health.updated", health);
    const socket = new FakeSocket();
    const client = new RealtimeClient(bus, runtime(true), () => socket, () => 0);
    await client.start();
    socket.onopen?.(new Event("open"));
    socket.onmessage?.({ data: JSON.stringify(snapshotEnvelope()) } as MessageEvent);
    expect(health).toHaveBeenCalledOnce();
    expect(health.mock.calls[0][0].components).toHaveLength(8);
  });

  it("normalizes realtime telemetry without polling the browser", async () => {
    installDom();
    const bus = new EventBus<AppEvents>();
    const telemetry = vi.fn();
    bus.on("telemetry.updated", telemetry);
    const socket = new FakeSocket();
    const client = new RealtimeClient(bus, runtime(true), () => socket, () => 0);
    await client.start();
    socket.onmessage?.({ data: JSON.stringify({
      event_version: "v1",
      event_id: "event-0000000000000002",
      type: "telemetry.snapshot",
      timestamp_ms: 2,
      payload: {
        timestamp_ms: 2,
        host: "server-1",
        kernel: "linux-6.0",
        cpu_usage_percent: 25,
        memory_used_bytes: 50,
        memory_total_bytes: 100,
        load_average: [0.1, 0.2, 0.3],
        filesystem_used_bytes: 20,
        filesystem_total_bytes: 100,
        disk_read_bytes_per_second: 10,
        disk_write_bytes_per_second: 5,
        network_receive_bytes_per_second: 8,
        network_transmit_bytes_per_second: 4,
        uptime_seconds: 60,
        temperatures: [],
      },
    }) } as MessageEvent);
    expect(telemetry).toHaveBeenCalledWith(expect.objectContaining({
      hostname: "server-1",
      memoryUsage: 50,
      diskUsage: 20,
    }));
  });

  it("maps real Codex lifecycle events into Agent Matrix and Activity Stream", async () => {
    installDom();
    const bus = new EventBus<AppEvents>();
    const agent = vi.fn();
    const activity = vi.fn();
    bus.on("realtime.agent.changed", agent);
    bus.on("realtime.activity", activity);
    const socket = new FakeSocket();
    const client = new RealtimeClient(bus, runtime(true), () => socket, () => 0);
    await client.start();
    socket.onmessage?.({ data: JSON.stringify({
      event_version: "v1", event_id: "event-0000000000000003", type: "agent.status.changed", timestamp_ms: 3,
      payload: { id: "codex", label: "CODEX AGENT", status: "healthy", agent_status: "BUSY", version: "sdk" },
    }) } as MessageEvent);
    socket.onmessage?.({ data: JSON.stringify({
      event_version: "v1", event_id: "event-0000000000000004", type: "codex.task.analyzing", timestamp_ms: 4,
      payload: { status: "ANALYZING" },
    }) } as MessageEvent);
    expect(agent).toHaveBeenCalledWith(expect.objectContaining({ id: "codex", agentStatus: "busy" }));
    expect(activity).toHaveBeenCalledWith(expect.objectContaining({ component: "CODEX", message: "TASK ANALYZING" }));
  });
});

function runtime(session: boolean): JarvisRuntimeClient {
  return {
    kind: "browser",
    coreHealth: vi.fn(),
    conversation: vi.fn(),
    telemetry: vi.fn(),
    hasSession: vi.fn(async () => session),
    login: vi.fn(async () => undefined),
    logout: vi.fn(async () => undefined),
    websocketUrl: vi.fn(() => "wss://jarvis.example.internal/ws"),
    voiceWebsocketUrl: vi.fn(() => "wss://jarvis.example.internal/ws/voice"),
  };
}

function installDom(): void {
  vi.stubGlobal("document", {
    hidden: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  });
  vi.stubGlobal("window", {
    setTimeout,
    clearTimeout,
  });
}

function snapshotEnvelope(): object {
  const components = ["core", "codex", "voice", "memory", "n8n", "monitor", "security", "mcp"]
    .map((id, index) => ({
      id,
      label: id.toUpperCase(),
      status: index === 0 ? "healthy" : "unavailable",
      agent_status: index === 0 ? "READY" : "OFFLINE",
      version: index === 0 ? "0.1.0" : "not_connected",
      ...(index === 0 ? { last_seen_ms: 1 } : { error: "not_connected" }),
    }));
  return {
    event_version: "v1",
    event_id: "event-0000000000000001",
    type: "system.snapshot",
    timestamp_ms: 1,
    payload: { api_version: "v1", status: "degraded", state: "IDLE", components },
  };
}
