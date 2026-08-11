#!/usr/bin/env python3
"""Private MCP gateway exposing allow-listed, schema-bound JARVIS capabilities."""
from __future__ import annotations

import hmac
import json
import os
import ssl
import sys
import urllib.parse
import urllib.request
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

MAX_BODY = 32 * 1024
ALLOWED_VMIDS = {124, 125}
PROTOCOL_VERSIONS = {"2025-06-18", "2025-11-25"}


class Runtime:
    def __init__(self) -> None:
        directory = Path(os.environ.get("JARVIS_MCP_CREDENTIALS", "/etc/jarvis-mcp"))
        self.gateway_token = (directory / "gateway-token").read_text().strip()
        self.read_token = (directory / "proxmox-read-token").read_text().strip()
        if min(len(self.gateway_token), len(self.read_token)) < 32:
            raise RuntimeError("MCP credentials are invalid")
        self.pve_url = os.environ.get("JARVIS_PROXMOX_URL", "https://192.168.1.5:8006/api2/json/")
        self.pve_identity = "jarvis-mcp@pve!read"
        self.prometheus_url = os.environ.get("JARVIS_PROMETHEUS_URL", "http://192.168.1.24:9090").rstrip("/")
        parsed_prometheus = urllib.parse.urlparse(self.prometheus_url)
        if parsed_prometheus.scheme != "http" or parsed_prometheus.hostname not in {"192.168.1.24", "127.0.0.1"}:
            raise RuntimeError("Prometheus URL is not an allowed internal endpoint")
        self.wazuh_url = os.environ.get("JARVIS_WAZUH_RELAY_URL", "http://192.168.1.10:5515/alerts")
        parsed_wazuh = urllib.parse.urlparse(self.wazuh_url)
        if parsed_wazuh.scheme != "http" or parsed_wazuh.hostname != "192.168.1.10":
            raise RuntimeError("Wazuh relay URL is not an allowed internal endpoint")
        self.wazuh_token = (directory / "wazuh-relay-token").read_text().strip()
        if len(self.wazuh_token) < 32:
            raise RuntimeError("Wazuh relay credential is invalid")
        self.ssl_context = ssl.create_default_context(cafile=str(directory / "proxmox-ca.pem"))
        # Proxmox's private CA lacks the extension required by OpenSSL strict mode.
        # Chain and hostname validation remain enabled.
        self.ssl_context.verify_flags &= ~ssl.VERIFY_X509_STRICT

    def pve_get(self, path: str) -> Any:
        url = urllib.parse.urljoin(self.pve_url, path.lstrip("/"))
        request = urllib.request.Request(url, headers={
            "Authorization": f"PVEAPIToken={self.pve_identity}={self.read_token}",
            "Accept": "application/json",
        })
        with urllib.request.urlopen(request, timeout=8, context=self.ssl_context) as response:
            if response.status != HTTPStatus.OK:
                raise RuntimeError("Proxmox rejected request")
            body = response.read(128 * 1024 + 1)
        if len(body) > 128 * 1024:
            raise RuntimeError("Proxmox response exceeded limit")
        return json.loads(body)["data"]

    def prometheus_scalar(self, query: str) -> float:
        encoded = urllib.parse.urlencode({"query": query})
        request = urllib.request.Request(f"{self.prometheus_url}/api/v1/query?{encoded}", headers={"Accept": "application/json"})
        with urllib.request.urlopen(request, timeout=5) as response:
            if response.status != HTTPStatus.OK:
                raise RuntimeError("Prometheus rejected request")
            body = response.read(32 * 1024 + 1)
        if len(body) > 32 * 1024:
            raise RuntimeError("Prometheus response exceeded limit")
        payload = json.loads(body)
        results = payload.get("data", {}).get("result", [])
        if payload.get("status") != "success" or len(results) != 1:
            raise RuntimeError("Prometheus returned no unique scalar")
        value = float(results[0]["value"][1])
        if value != value or value < 0 or value == float("inf"):
            raise RuntimeError("Prometheus returned invalid scalar")
        return value

    def wazuh_alerts(self, host: str, severity: str, limit: int) -> dict[str, Any]:
        request = urllib.request.Request(self.wazuh_url, headers={"Authorization": f"Bearer {self.wazuh_token}", "Accept": "application/json"})
        with urllib.request.urlopen(request, timeout=5) as response:
            if response.status != HTTPStatus.OK:
                raise RuntimeError("Wazuh relay rejected request")
            body = response.read(64 * 1024 + 1)
        if len(body) > 64 * 1024:
            raise RuntimeError("Wazuh response exceeded limit")
        payload = json.loads(body)
        alerts = payload.get("alerts", [])
        filtered = [item for item in alerts if (not host or str(item.get("host", "")).casefold() == host.casefold()) and (not severity or item.get("severity") == severity)]
        return {"source": "wazuh", "host": host or "all", "severity": severity or "all", "count": len(filtered), "alerts": filtered[:limit]}


RUNTIME: Runtime


TOOLS = [
    {
        "name": "proxmox.vm.list",
        "title": "List JARVIS virtual machines",
        "description": "Returns a reduced inventory limited to the Proxmox JARVIS pool.",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
        "annotations": {"readOnlyHint": True, "destructiveHint": False, "idempotentHint": True},
    },
    {
        "name": "wazuh.security.alerts",
        "title": "Query Wazuh security alerts",
        "description": "Read-only query of normalized Wazuh alerts by endpoint and severity.",
        "inputSchema": {"type": "object", "properties": {"host": {"type": "string", "maxLength": 128}, "severity": {"type": "string", "enum": ["low", "medium", "high", "critical"]}, "limit": {"type": "integer", "minimum": 1, "maximum": 20}}, "additionalProperties": False},
        "annotations": {"readOnlyHint": True, "destructiveHint": False, "idempotentHint": True},
    },
    {
        "name": "proxmox.vm.status",
        "title": "Inspect JARVIS virtual machine status",
        "description": "Returns normalized status for one explicitly allowed JARVIS container.",
        "inputSchema": {
            "type": "object",
            "properties": {"vmid": {"type": "integer", "enum": [124, 125]}},
            "required": ["vmid"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "destructiveHint": False, "idempotentHint": True},
    },
    {
        "name": "prometheus.server_central.telemetry",
        "title": "Read Server Central telemetry",
        "description": "Returns normalized operational telemetry for the Proxmox server-central node. Read-only; no arbitrary PromQL is accepted.",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
        "annotations": {"readOnlyHint": True, "destructiveHint": False, "idempotentHint": True},
    },
]


class Handler(BaseHTTPRequestHandler):
    server_version = "JarvisMCP/1"
    sys_version = ""

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def do_GET(self) -> None:
        if self.path == "/v1/health":
            self._json(HTTPStatus.OK, {"status": "healthy", "version": "v1"})
        else:
            self._json(HTTPStatus.NOT_FOUND, {"code": "NOT_FOUND"})

    def do_POST(self) -> None:
        if self.path != "/mcp":
            self._json(HTTPStatus.NOT_FOUND, {"code": "NOT_FOUND"})
            return
        if not self._authorized():
            self._json(HTTPStatus.UNAUTHORIZED, {"code": "AUTHENTICATION_REQUIRED"})
            return
        body = self._body()
        if body is None:
            return
        request_id: str | int | None = None
        try:
            value = json.loads(body)
            if not isinstance(value, dict) or value.get("jsonrpc") != "2.0":
                raise ValueError
            if set(value) - {"jsonrpc", "id", "method", "params"}:
                raise ValueError
            request_id = value.get("id")
            method = value.get("method")
            params = value.get("params", {})
            if request_id is None and method == "notifications/initialized":
                self.send_response(HTTPStatus.ACCEPTED)
                self.send_header("Content-Length", "0")
                self.send_header("Cache-Control", "no-store")
                self.end_headers()
                return
            result = self._dispatch(method, params)
            self._json(HTTPStatus.OK, {"jsonrpc": "2.0", "id": request_id, "result": result})
        except ValueError:
            safe_params = value.get("params") if isinstance(value, dict) else None
            print(json.dumps({
                "event": "mcp.request.rejected",
                "method": value.get("method") if isinstance(value, dict) else None,
                "parameter_keys": sorted(safe_params.keys()) if isinstance(safe_params, dict) else [],
                "tool": safe_params.get("name") if isinstance(safe_params, dict) else None,
            }), file=sys.stderr, flush=True)
            self._rpc_error(request_id, -32602, "Invalid request parameters")
        except Exception:
            self._rpc_error(request_id, -32603, "Tool execution failed")

    def _dispatch(self, method: object, params: object) -> dict[str, Any]:
        if method == "initialize":
            if not isinstance(params, dict):
                raise ValueError
            requested_version = params.get("protocolVersion", "2025-11-25")
            if requested_version not in PROTOCOL_VERSIONS:
                raise ValueError
            return {
                "protocolVersion": requested_version,
                "capabilities": {"tools": {"listChanged": False}},
                "serverInfo": {"name": "jarvis-mcp-gateway", "version": "0.1.0"},
            }
        if method == "tools/list":
            if params not in ({}, None):
                raise ValueError
            return {"tools": TOOLS}
        if method == "tools/call":
            if (
                not isinstance(params, dict)
                or not {"name", "arguments"}.issubset(params)
                or set(params) - {"name", "arguments", "_meta"}
            ):
                raise ValueError
            return self._call_tool(params["name"], params["arguments"])
        raise ValueError

    def _call_tool(self, name: object, arguments: object) -> dict[str, Any]:
        if name == "proxmox.vm.list" and arguments == {}:
            members = RUNTIME.pve_get("pools/JARVIS").get("members", [])
            data = [{
                "vmid": item.get("vmid"), "name": item.get("name"), "type": item.get("type"),
                "status": item.get("status"), "node": item.get("node"),
            } for item in members if item.get("vmid") in ALLOWED_VMIDS]
            return self._tool_result({"pool": "JARVIS", "members": data})
        if name == "proxmox.vm.status" and isinstance(arguments, dict) and set(arguments) == {"vmid"}:
            vmid = arguments["vmid"]
            if vmid not in ALLOWED_VMIDS:
                raise ValueError
            item = RUNTIME.pve_get(f"nodes/Proxmox/lxc/{vmid}/status/current")
            data = {key: item.get(key) for key in ("vmid", "name", "status", "uptime", "cpu", "mem", "maxmem")}
            return self._tool_result(data)
        if name == "prometheus.server_central.telemetry" and arguments == {}:
            instance = 'instance="192.168.1.5:9100"'
            data = {
                "source": "prometheus",
                "host": "server-central",
                "cpu_usage_percent": round(RUNTIME.prometheus_scalar(f'100 - avg(rate(node_cpu_seconds_total{{{instance},mode="idle"}}[2m])) * 100'), 2),
                "memory_total_bytes": int(RUNTIME.prometheus_scalar(f"node_memory_MemTotal_bytes{{{instance}}}")),
                "memory_available_bytes": int(RUNTIME.prometheus_scalar(f"node_memory_MemAvailable_bytes{{{instance}}}")),
                "load_average": [RUNTIME.prometheus_scalar(f"node_load{x}{{{instance}}}") for x in (1, 5, 15)],
                "filesystem_total_bytes": int(RUNTIME.prometheus_scalar(f'node_filesystem_size_bytes{{{instance},mountpoint="/"}}')),
                "filesystem_available_bytes": int(RUNTIME.prometheus_scalar(f'node_filesystem_avail_bytes{{{instance},mountpoint="/"}}')),
                "network_receive_bytes_per_second": round(RUNTIME.prometheus_scalar(f'sum(rate(node_network_receive_bytes_total{{{instance},device!="lo"}}[2m]))'), 2),
                "network_transmit_bytes_per_second": round(RUNTIME.prometheus_scalar(f'sum(rate(node_network_transmit_bytes_total{{{instance},device!="lo"}}[2m]))'), 2),
                "uptime_seconds": int(RUNTIME.prometheus_scalar(f"time() - node_boot_time_seconds{{{instance}}}")),
            }
            return self._tool_result(data)
        if name == "wazuh.security.alerts" and isinstance(arguments, dict) and set(arguments).issubset({"host", "severity", "limit"}):
            host = arguments.get("host", "")
            severity = arguments.get("severity", "")
            limit = arguments.get("limit", 10)
            if not isinstance(host, str) or not isinstance(severity, str) or not isinstance(limit, int) or not 1 <= limit <= 20:
                raise ValueError
            return self._tool_result(RUNTIME.wazuh_alerts(host, severity, limit))
        raise ValueError

    @staticmethod
    def _tool_result(data: dict[str, Any]) -> dict[str, Any]:
        return {"content": [{"type": "text", "text": json.dumps(data, separators=(",", ":"))}], "structuredContent": data, "isError": False}

    def _authorized(self) -> bool:
        value = self.headers.get("Authorization", "")
        return value.startswith("Bearer ") and hmac.compare_digest(value[7:], RUNTIME.gateway_token)

    def _body(self) -> bytes | None:
        try:
            length = int(self.headers.get("Content-Length", "-1"))
        except ValueError:
            length = -1
        if length < 1 or length > MAX_BODY:
            self._json(HTTPStatus.REQUEST_ENTITY_TOO_LARGE, {"code": "BODY_SIZE_REJECTED"})
            return None
        return self.rfile.read(length)

    def _rpc_error(self, request_id: object, code: int, message: str) -> None:
        self._json(HTTPStatus.OK, {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}})

    def _json(self, status: HTTPStatus, value: dict[str, Any]) -> None:
        body = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(body)


def main() -> None:
    global RUNTIME
    RUNTIME = Runtime()
    host, port = os.environ.get("JARVIS_MCP_BIND", "127.0.0.1:4300").rsplit(":", 1)
    server = ThreadingHTTPServer((host, int(port)), Handler)
    server.daemon_threads = True
    server.serve_forever()


if __name__ == "__main__":
    main()
