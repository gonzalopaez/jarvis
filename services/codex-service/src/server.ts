import { createHash, timingSafeEqual } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { isIP } from "node:net";
import { CodexTaskService, ServiceError, type CodexExecutor } from "./service.js";
import { parseCreateTask } from "./contracts.js";
import { Codex } from "@openai/codex-sdk";

const MAX_BODY = 32 * 1024;
const bind = process.env.JARVIS_CODEX_BIND ?? "127.0.0.1";
const port = boundedInteger(process.env.JARVIS_CODEX_PORT, 4400, 1, 65535);
const credentialPath = process.env.JARVIS_CODEX_TOKEN_FILE;
if (!isPrivateBind(bind)) throw new Error("JARVIS Codex Service may bind only to loopback or private IPv4 addresses");
if (!credentialPath) throw new Error("JARVIS_CODEX_TOKEN_FILE is required");
const tokenDigest = digest(readCredential(credentialPath, 20));
const apiKeyPath = process.env.JARVIS_CODEX_GATEWAY_TOKEN_FILE;
const baseUrl = process.env.JARVIS_CODEX_BASE_URL ?? "http://127.0.0.1:4000/v1";
// LiteLLM exposes the OpenAI-compatible HTTPS Responses API, but not the
// Codex-specific Responses WebSocket upgrade. Force the SDK's supported HTTP
// transport while retaining the gateway as the only model/auth boundary.
const gatewayToken = apiKeyPath ? readCredential(apiKeyPath, 20) : undefined;
const mcpTokenPath = process.env.JARVIS_MCP_TOKEN_FILE;
const mcpToken = mcpTokenPath ? readCredential(mcpTokenPath, 20) : undefined;
const mcpUrl = process.env.JARVIS_MCP_URL;
const codex: CodexExecutor = gatewayToken ? createLiteLlmExecutor(baseUrl, gatewayToken, mcpUrl, mcpToken) : createSdkExecutor(baseUrl);
const service = new CodexTaskService(codex, boundedInteger(process.env.JARVIS_CODEX_MAX_CONCURRENT, 2, 1, 8), boundedInteger(process.env.JARVIS_CODEX_TASK_TIMEOUT_MS, 120_000, 10_000, 600_000));

createServer(async (request, response) => {
  response.setHeader("content-type", "application/json; charset=utf-8");
  response.setHeader("cache-control", "no-store");
  response.setHeader("x-content-type-options", "nosniff");
  try {
    if (!authorized(request)) return send(response, 401, { error: { code: "AUTHENTICATION_REQUIRED", message: "Valid service authentication is required" } });
    if (request.method === "GET" && request.url === "/health") return send(response, 200, { status: service.health(), version: "0.1.0" });
    if (request.method === "POST" && request.url === "/v1/tasks") {
      const taskRequest = parseCreateTask(await readJson(request));
      if (!taskRequest) return send(response, 400, { error: { code: "INVALID_REQUEST", message: "Task contract validation failed" } });
      return send(response, 202, service.create(taskRequest));
    }
    const match = request.method === "GET" ? request.url?.match(/^\/v1\/tasks\/(codex-[0-9a-f-]{36})$/) : null;
    if (match?.[1]) {
      const task = service.get(match[1]);
      return task ? send(response, 200, task) : send(response, 404, { error: { code: "TASK_NOT_FOUND", message: "Task was not found" } });
    }
    return send(response, 404, { error: { code: "NOT_FOUND", message: "Resource was not found" } });
  } catch (error) {
    if (error instanceof ServiceError) return send(response, error.status, { error: { code: error.code, message: "Request could not be completed" } });
    return send(response, 500, { error: { code: "INTERNAL_ERROR", message: "Request could not be completed" } });
  }
}).listen(port, bind, () => process.stderr.write(JSON.stringify({ level: "INFO", service: "codex-service", event: "service.ready", bind, port }) + "\n"));

function authorized(request: IncomingMessage): boolean {
  const value = request.headers.authorization;
  if (!value?.startsWith("Bearer ")) return false;
  const candidate = digest(value.slice(7));
  return candidate.length === tokenDigest.length && timingSafeEqual(candidate, tokenDigest);
}

async function readJson(request: IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > MAX_BODY) throw new ServiceError("PAYLOAD_TOO_LARGE", 413);
    chunks.push(chunk);
  }
  try { return JSON.parse(Buffer.concat(chunks).toString("utf8")); }
  catch { throw new ServiceError("INVALID_JSON", 400); }
}

function send(response: ServerResponse, status: number, body: unknown): void {
  response.statusCode = status;
  response.end(JSON.stringify(body));
}

function digest(value: string): Buffer { return createHash("sha256").update(value).digest(); }
function readCredential(path: string, minimumLength: number): string {
  const metadata = statSync(path);
  if (!metadata.isFile() || metadata.size > 4 * 1024) throw new Error("Credential file is invalid");
  const value = readFileSync(path, "utf8").trim();
  if (value.length < minimumLength) throw new Error("Credential is invalid");
  return value;
}
function boundedInteger(value: string | undefined, fallback: number, min: number, max: number): number {
  const parsed = value === undefined ? fallback : Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < min || parsed > max) throw new Error("Invalid bounded integer configuration");
  return parsed;
}
function isPrivateBind(value: string): boolean {
  if (value === "127.0.0.1" || value === "::1") return true;
  if (isIP(value) !== 4) return false;
  const [a, b] = value.split(".").map(Number);
  return a === 10 || (a === 172 && b! >= 16 && b! <= 31) || (a === 192 && b === 168);
}

function createSdkExecutor(baseUrl: string): CodexExecutor {
  const sdk = new Codex({ baseUrl });
  return { async run(prompt, model, signal) {
    const thread = sdk.startThread({ model, sandboxMode: "read-only", approvalPolicy: "never", networkAccessEnabled: false, webSearchMode: "disabled", skipGitRepoCheck: true });
    const result = await thread.run(prompt, { signal });
    return { finalResponse: result.finalResponse, threadId: thread.id ?? undefined };
  }};
}

function createLiteLlmExecutor(baseUrl: string, token: string, mcpUrl?: string, mcpToken?: string): CodexExecutor {
  const tools = [{ type: "function", function: { name: "proxmox_vm_list", description: "Listar el inventario permitido de contenedores JARVIS en Proxmox.", parameters: { type: "object", properties: {}, additionalProperties: false } } }, { type: "function", function: { name: "proxmox_vm_status", description: "Consultar el estado de un contenedor JARVIS permitido.", parameters: { type: "object", properties: { vmid: { type: "integer", enum: [124, 125] } }, required: ["vmid"], additionalProperties: false } } }, { type: "function", function: { name: "prometheus_server_central_telemetry", description: "Leer telemetría operacional normalizada de Server Central desde Prometheus.", parameters: { type: "object", properties: {}, additionalProperties: false } } }, { type: "function", function: { name: "wazuh_security_alerts", description: "Consultar alertas Wazuh reales por equipo y severidad.", parameters: { type: "object", properties: { host: { type: "string", maxLength: 128 }, severity: { type: "string", enum: ["low", "medium", "high", "critical"] }, limit: { type: "integer", minimum: 1, maximum: 20 } }, additionalProperties: false } } }];
  return { async run(prompt, model, signal) {
    let evidenceAttached = false;
    let userContent = prompt;
    if (mcpUrl && mcpToken) {
      const request = explicitInfrastructureTool(prompt);
      if (request) {
        const evidence = await callMcpTool(mcpUrl, mcpToken, request.name, request.arguments, signal);
        userContent += `\n\nEVIDENCIA REAL OBTENIDA POR MCP (no inventar ni modificar):\n${JSON.stringify(evidence)}`;
        evidenceAttached = true;
      }
    }
    const messages: Array<Record<string, unknown>> = [{ role: "system", content: "Eres el agente técnico de JARVIS. Responde siempre en español claro y directo. No inventes datos ni ejecuciones. Si falta información, pide el dato exacto. Explica primero el diagnóstico y luego pasos concretos. Usa únicamente la evidencia MCP proporcionada." }, { role: "user", content: userContent }];
    for (let turn = 0; turn < 3; turn += 1) {
      const allowTools = !evidenceAttached && turn === 0 && Boolean(mcpUrl && mcpToken);
      const response = await fetch(`${baseUrl.replace(/\/$/, "")}/chat/completions`, { method: "POST", signal, headers: { authorization: `Bearer ${token}`, "content-type": "application/json" }, body: JSON.stringify({ model, messages, tools: allowTools ? tools : undefined, tool_choice: allowTools ? "auto" : undefined, max_tokens: 600, temperature: 0.1, top_p: 0.8 }) });
      if (!response.ok) throw new Error(`gateway_http_${response.status}`);
      const payload = await response.json() as { id?: string; choices?: Array<{ message?: { content?: unknown; tool_calls?: Array<{ id: string; function?: { name?: string; arguments?: string } }> } }> };
      const message = payload.choices?.[0]?.message;
      const calls = message?.tool_calls ?? [];
      if (!calls.length || !mcpUrl || !mcpToken) {
        const finalResponse = typeof message?.content === "string" ? message.content.trim() : "";
        if (!finalResponse) throw new Error("gateway_empty_response");
        return { finalResponse, threadId: payload.id };
      }
      messages.push({ role: "assistant", content: message?.content ?? null, tool_calls: calls });
      for (const call of calls.slice(0, 2)) {
        const name = call.function?.name ?? "";
        const args = call.function?.arguments ? JSON.parse(call.function.arguments) : {};
        const result = await callMcpTool(mcpUrl, mcpToken, name, args, signal);
        messages.push({ role: "tool", tool_call_id: call.id, content: JSON.stringify(result) });
      }
    }
    throw new Error("tool_round_limit");
  }};
}

function explicitInfrastructureTool(prompt: string): { name: string; arguments: unknown } | null {
  const value = prompt.toLocaleLowerCase("es-AR");
  if (/(wazuh|alertas?|amenazas?|incidentes?)/.test(value) && /(cr[ií]tic|equipo|host|servidor|alertas?)/.test(value)) {
    const host = value.match(/(?:equipo|host|servidor)\s+(?:de\s+nombre\s+)?([a-z0-9][a-z0-9._-]{1,127})/i)?.[1];
    return { name: "wazuh_security_alerts", arguments: { ...(host ? { host } : {}), ...(/(cr[ií]tic)/.test(value) ? { severity: "critical" } : {}), limit: 10 } };
  }
  if (/(telemetr|prometheus|server central|servidor central)/.test(value)) return { name: "prometheus_server_central_telemetry", arguments: {} };
  const vmid = value.match(/\b(124|125)\b/);
  if (vmid && /(proxmox|contenedor|lxc|estado|servicio)/.test(value)) return { name: "proxmox_vm_status", arguments: { vmid: Number(vmid[1]) } };
  if (/(proxmox|contenedores|máquinas virtuales|inventario)/.test(value)) return { name: "proxmox_vm_list", arguments: {} };
  return null;
}

async function callMcpTool(url: string, token: string, name: string, argumentsValue: unknown, signal: AbortSignal): Promise<unknown> {
  const mapping: Record<string, string> = { proxmox_vm_list: "proxmox.vm.list", proxmox_vm_status: "proxmox.vm.status", prometheus_server_central_telemetry: "prometheus.server_central.telemetry", wazuh_security_alerts: "wazuh.security.alerts" };
  const upstream = mapping[name];
  if (!upstream) throw new Error("tool_not_allowed");
  if (name === "proxmox_vm_list" || name === "prometheus_server_central_telemetry") argumentsValue = {};
  if (name === "wazuh_security_alerts" && (!argumentsValue || typeof argumentsValue !== "object" || !Object.keys(argumentsValue as object).every((key) => ["host", "severity", "limit"].includes(key)))) throw new Error("tool_arguments_rejected");
  if (name === "proxmox_vm_status" && (!argumentsValue || typeof argumentsValue !== "object" || ![124, 125].includes((argumentsValue as { vmid?: unknown }).vmid as number))) throw new Error("tool_arguments_rejected");
  const response = await fetch(url, { method: "POST", signal, headers: { authorization: `Bearer ${token}`, "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: "codex-tool", method: "tools/call", params: { name: upstream, arguments: argumentsValue } }) });
  if (!response.ok) throw new Error(`mcp_http_${response.status}`);
  const payload = await response.json() as { result?: unknown; error?: unknown };
  if (payload.error || !payload.result) throw new Error("mcp_tool_failed");
  return payload.result;
}
