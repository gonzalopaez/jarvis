import { describe, expect, it, vi } from "vitest";
import { WebRuntimeClient } from "./web-client";

describe("WebRuntimeClient", () => {
  it("accepts only the minimal versioned Core health envelope", async () => {
    const fetcher = vi.fn(async () => new Response(
      JSON.stringify(healthFixture()),
      { status: 200, headers: { "content-type": "application/json" } },
    ));
    const health = await new WebRuntimeClient(fetcher as typeof fetch).coreHealth();
    expect(health.online).toBe(true);
    expect(health.apiVersion).toBe("v1");
    expect(health.status).toBe("degraded");
    expect(health.components).toHaveLength(7);
    expect(fetcher).toHaveBeenCalledWith("/api/v1/health", expect.objectContaining({
      credentials: "same-origin",
      redirect: "error",
    }));
  });

  it("rejects unexpected health fields", async () => {
    const fetcher = vi.fn(async () => new Response(
      JSON.stringify({ ...healthFixture(), internal: "leak" }),
      { status: 200 },
    ));
    await expect(new WebRuntimeClient(fetcher as typeof fetch).coreHealth()).rejects.toThrow(
      "Core health response is invalid",
    );
  });

  it("exchanges an access key for an HttpOnly-backed session", async () => {
    const fetcher = vi.fn()
      .mockResolvedValueOnce(new Response("{}", { status: 201 }))
      .mockResolvedValueOnce(new Response(JSON.stringify({
        api_version: "v1", authenticated: true, csrf_token: "a".repeat(64),
      }), { status: 200 }));
    const client = new WebRuntimeClient(fetcher as typeof fetch);
    await client.login("k".repeat(32));
    expect(fetcher).toHaveBeenNthCalledWith(1, "/api/v1/session", expect.objectContaining({
      method: "POST", credentials: "same-origin",
      headers: expect.objectContaining({ Authorization: `Bearer ${"k".repeat(32)}` }),
    }));
  });

  it("sends browser conversations with CSRF and validates correlation", async () => {
    const fetcher = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({
        api_version: "v1", authenticated: true, csrf_token: "b".repeat(64),
      }), { status: 200 }))
      .mockImplementationOnce(async (_url: string, init: RequestInit) => {
        const request = JSON.parse(String(init.body));
        return new Response(JSON.stringify({
          api_version: "v1", request_id: request.request_id, status: "completed",
          audit_id: "audit-1", data: { message: "Ready.", mode: "mock" },
        }), { status: 200 });
      });
    const client = new WebRuntimeClient(fetcher as typeof fetch);
    await expect(client.conversation("status")).resolves.toMatchObject({ message: "Ready." });
    expect(fetcher).toHaveBeenLastCalledWith("/api/v1/requests", expect.objectContaining({
      method: "POST", headers: expect.objectContaining({ "x-jarvis-csrf": "b".repeat(64) }),
    }));
  });

  it("requires a valid in-memory CSRF value for an authenticated session", async () => {
    const fetcher = vi.fn(async () => new Response(JSON.stringify({
      api_version: "v1",
      authenticated: true,
      csrf_token: "a".repeat(64),
    }), { status: 200 }));
    await expect(new WebRuntimeClient(fetcher as typeof fetch).hasSession()).resolves.toBe(true);
  });

  it("builds realtime URLs only from HTTPS locations", () => {
    const client = new WebRuntimeClient(vi.fn() as unknown as typeof fetch);
    expect(client.websocketUrl({ protocol: "https:", host: "jarvis.example.internal" }))
      .toBe("wss://jarvis.example.internal/ws");
    expect(client.voiceWebsocketUrl({ protocol: "https:", host: "jarvis.example.internal" }))
      .toBe("wss://jarvis.example.internal/ws/voice");
    expect(() => client.websocketUrl({ protocol: "http:", host: "localhost" })).toThrow(
      "Realtime requires HTTPS",
    );
  });
});

function healthFixture(): object {
  const components = ["core", "codex", "voice", "mcp", "n8n", "wazuh", "proxmox"]
    .map((id, index) => ({
      id,
      label: id.toUpperCase(),
      status: index === 0 ? "healthy" : "unavailable",
      agent_status: index === 0 ? "READY" : "OFFLINE",
      version: index === 0 ? "0.1.0" : "not_connected",
      ...(index === 0 ? { last_seen_ms: 1 } : { error: "not_connected" }),
    }));
  return { api_version: "v1", status: "degraded", state: "IDLE", components };
}
