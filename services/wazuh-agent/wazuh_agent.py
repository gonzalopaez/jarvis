#!/usr/bin/env python3
"""Wazuh domain agent: read evidence, triage through LiteLLM, propose to Core."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
import uuid
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

MAX_CONTEXT_BYTES = 12 * 1024
LLM_TIMEOUT_SECONDS = 8
HTTP_TIMEOUT_SECONDS = 8
MAX_RESPONSE_BYTES = 128 * 1024
MAX_REQUEST_BYTES = 64 * 1024
ALLOWED_ACTIONS = {
    "security.user.disable",
    "security.ip.block",
    "security.host.isolate",
}


def _read_secret(path: str) -> str:
    value = Path(path).read_text(encoding="utf-8").strip()
    if len(value) < 20:
        raise RuntimeError("credential is missing or too short")
    return value


@dataclass(frozen=True)
class AgentConfig:
    alerts_path: str
    relay_token_file: str
    litellm_url: str
    litellm_token_file: str
    core_url: str
    core_token_file: str
    verdict_schema_path: str
    host: str = "192.168.1.10"
    port: int = 5515

    @classmethod
    def from_env(cls) -> "AgentConfig":
        return cls(
            alerts_path=os.getenv("JARVIS_WAZUH_ALERTS_PATH", "/var/ossec/logs/alerts/alerts.json"),
            relay_token_file=os.getenv("JARVIS_WAZUH_AGENT_TOKEN_FILE", "/etc/jarvis-wazuh-agent/relay-token"),
            litellm_url=os.environ["JARVIS_LITELLM_URL"].rstrip("/"),
            litellm_token_file=os.getenv("JARVIS_LITELLM_TOKEN_FILE", "/run/credentials/litellm-token"),
            core_url=os.environ["JARVIS_CORE_URL"].rstrip("/"),
            core_token_file=os.getenv("JARVIS_CORE_TOKEN_FILE", "/run/credentials/core-token"),
            verdict_schema_path=os.getenv(
                "JARVIS_SECURITY_VERDICT_SCHEMA",
                "/usr/local/share/jarvis/contracts/security-verdict.v1.schema.json",
            ),
            host=os.getenv("JARVIS_WAZUH_AGENT_HOST", "192.168.1.10"),
            port=int(os.getenv("JARVIS_WAZUH_AGENT_PORT", "5515")),
        )


def normalize_alert(raw: dict[str, Any]) -> dict[str, Any]:
    rule = raw.get("rule") if isinstance(raw.get("rule"), dict) else {}
    agent = raw.get("agent") if isinstance(raw.get("agent"), dict) else {}
    level = int(rule.get("level") or 0)
    severity = "critical" if level >= 12 else "high" if level >= 10 else "medium" if level >= 7 else "low"
    host = str(agent.get("name") or agent.get("hostname") or raw.get("hostname") or "unknown")[:128]
    full_log = str(raw.get("full_log") or "")
    data = raw.get("data") if isinstance(raw.get("data"), dict) else {}
    source_ip = str(data.get("srcip") or data.get("src_ip") or "")[:45]
    user = str(data.get("dstuser") or data.get("srcuser") or data.get("user") or "")[:128]
    return {
        "id": str(raw.get("id") or full_log[:32] or uuid.uuid4().hex)[:128],
        "host": host,
        "agent_id": str(agent.get("id") or "000")[:128],
        "timestamp": str(raw.get("timestamp") or "")[:128],
        "timestamp_ms": int(time.time() * 1000),
        "severity": severity,
        "level": level,
        "rule_id": str(rule.get("id") or "")[:128],
        "title": str(rule.get("description") or "Wazuh alert")[:160],
        "description": (full_log or str(rule.get("description") or "Security event"))[:2000],
        "user": user,
        "source_ip": source_ip,
    }


class AlertStore:
    def __init__(self, path: str):
        self.path = path

    def read(self, host: str = "", severity: str = "", limit: int = 20) -> list[dict[str, Any]]:
        try:
            with open(self.path, encoding="utf-8", errors="replace") as stream:
                lines = stream.readlines()[-500:]
        except OSError:
            return []
        alerts: list[dict[str, Any]] = []
        for line in lines:
            try:
                alert = normalize_alert(json.loads(line))
            except (TypeError, ValueError, json.JSONDecodeError):
                continue
            if host and alert["host"] != host:
                continue
            if severity and alert["severity"] != severity:
                continue
            alerts.append(alert)
        return alerts[-max(1, min(limit, 20)) :]

    @staticmethod
    def metrics(alerts: list[dict[str, Any]]) -> dict[str, Any]:
        def count(*words: str) -> int:
            return sum(
                1
                for item in alerts
                if any(word in f'{item["title"]} {item["description"]}'.lower() for word in words)
            )

        return {
            "timestamp_ms": int(time.time() * 1000),
            "alert_count": len(alerts),
            "failed_logins": count("authentication failed", "failed login", "invalid user"),
            "sudo_commands": count("sudo", "privilege escalation"),
            "new_processes": count("new process", "process created"),
            "fim_changes": count("file integrity", "modified file", "file changed"),
            "network_connections": count("connection", "network", "port"),
        }


class JsonHttpClient:
    def post(self, url: str, token: str, payload: dict[str, Any], timeout: int) -> dict[str, Any]:
        body = json.dumps(payload, separators=(",", ":")).encode()
        request = urllib.request.Request(
            url,
            data=body,
            method="POST",
            headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                raw = response.read(MAX_RESPONSE_BYTES + 1)
        except urllib.error.HTTPError as error:
            raw = error.read(MAX_RESPONSE_BYTES + 1)
        if len(raw) > MAX_RESPONSE_BYTES:
            raise RuntimeError("upstream response exceeded limit")
        value = json.loads(raw)
        if not isinstance(value, dict):
            raise RuntimeError("upstream response is not an object")
        return value


class WazuhAgent:
    def __init__(self, config: AgentConfig, http: JsonHttpClient | None = None):
        self.config = config
        self.alerts = AlertStore(config.alerts_path)
        self.http = http or JsonHttpClient()

    def triage(self, alert: dict[str, Any]) -> dict[str, Any]:
        normalized = normalize_alert(alert)
        context = json.dumps(normalized, ensure_ascii=False, separators=(",", ":"))
        if len(context.encode()) > MAX_CONTEXT_BYTES:
            raise ValueError("alert context exceeds 12 KiB")
        alias = "jarvis-soc-l2" if normalized["level"] >= 12 else "jarvis-soc-l1"
        schema = json.loads(Path(self.config.verdict_schema_path).read_text(encoding="utf-8"))
        payload = {
            "model": alias,
            "temperature": 0.05,
            "messages": [
                {
                    "role": "system",
                    "content": (
                        "Sos un agente de dominio Wazuh. Interpretá únicamente la evidencia provista. "
                        "Podés proponer acciones, pero nunca afirmar que fueron ejecutadas. Respondé JSON estricto."
                    ),
                },
                {"role": "user", "content": context},
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {"name": "security_verdict_v1", "strict": True, "schema": schema},
            },
        }
        response = self.http.post(
            f"{self.config.litellm_url}/v1/chat/completions",
            _read_secret(self.config.litellm_token_file),
            payload,
            LLM_TIMEOUT_SECONDS,
        )
        content = response["choices"][0]["message"]["content"]
        verdict = json.loads(content) if isinstance(content, str) else content
        self._validate_verdict(verdict)
        return verdict

    def propose(self, verdict: dict[str, Any], session_id: str) -> list[dict[str, Any]]:
        self._validate_verdict(verdict)
        results = []
        token = _read_secret(self.config.core_token_file)
        for proposed in verdict.get("proposed_actions", []):
            request_id = f"wazuh-{uuid.uuid4().hex}"
            payload = {
                "api_version": "v1",
                "request_id": request_id,
                "session_id": session_id,
                "kind": "action",
                "action": proposed,
            }
            response = self.http.post(
                f"{self.config.core_url}/api/v1/requests",
                token,
                payload,
                HTTP_TIMEOUT_SECONDS,
            )
            results.append({"request_id": request_id, "capability": proposed["capability"], "core_response": response})
        return results

    @staticmethod
    def _validate_verdict(verdict: Any) -> None:
        if not isinstance(verdict, dict):
            raise ValueError("verdict must be an object")
        required = {"verdict", "confidence", "justification"}
        if not required.issubset(verdict):
            raise ValueError("verdict is missing required fields")
        if verdict["verdict"] not in {"FALSO_POSITIVO", "AMENAZA_REAL_BAJA", "AMENAZA_REAL_MEDIA", "AMENAZA_REAL_ALTA"}:
            raise ValueError("invalid verdict")
        if verdict["confidence"] not in {"ALTA", "MEDIA", "BAJA"}:
            raise ValueError("invalid confidence")
        if not isinstance(verdict["justification"], str) or not verdict["justification"].strip():
            raise ValueError("invalid justification")
        for action in verdict.get("proposed_actions", []):
            if not isinstance(action, dict) or set(action) != {"capability", "target", "parameters"}:
                raise ValueError("invalid proposed action")
            if action["capability"] not in ALLOWED_ACTIONS or not isinstance(action["parameters"], dict):
                raise ValueError("proposed capability is not allowed")
            if not isinstance(action["target"], str) or not re.fullmatch(r"[a-z0-9_.:-]{1,128}", action["target"]):
                raise ValueError("proposed target does not match the Core contract")


def mcp_loop(agent: WazuhAgent) -> None:
    tools = [
        {
            "name": "wazuh.alerts.read",
            "description": "Read normalized Wazuh alerts without modifying Wazuh.",
            "inputSchema": {"type": "object", "properties": {"host": {"type": "string"}, "severity": {"type": "string"}, "limit": {"type": "integer"}}},
        },
        {
            "name": "wazuh.alert.triage",
            "description": "Triage one Wazuh alert and submit only proposed actions to Core.",
            "inputSchema": {"type": "object", "required": ["alert", "session_id"], "properties": {"alert": {"type": "object"}, "session_id": {"type": "string"}}},
        },
    ]
    for line in sys.stdin:
        request: Any = None
        try:
            request = json.loads(line)
            method = request.get("method")
            if method == "initialize":
                result = {"protocolVersion": "2025-03-26", "capabilities": {"tools": {}}, "serverInfo": {"name": "jarvis-wazuh-agent", "version": "1.0.0"}}
            elif method == "tools/list":
                result = {"tools": tools}
            elif method == "tools/call":
                params = request.get("params") or {}
                args = params.get("arguments") or {}
                if params.get("name") == "wazuh.alerts.read":
                    value = {"alerts": agent.alerts.read(str(args.get("host") or ""), str(args.get("severity") or ""), int(args.get("limit") or 20))}
                elif params.get("name") == "wazuh.alert.triage":
                    verdict = agent.triage(args["alert"])
                    value = {"verdict": verdict, "proposals": agent.propose(verdict, args["session_id"])}
                else:
                    raise ValueError("tool is not allowlisted")
                result = {"content": [{"type": "text", "text": json.dumps(value, ensure_ascii=False)}]}
            else:
                continue
            print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
        except Exception as error:  # MCP must return a bounded safe error.
            print(json.dumps({"jsonrpc": "2.0", "id": request.get("id") if isinstance(request, dict) else None, "error": {"code": -32000, "message": str(error)[:256]}}), flush=True)


def serve(agent: WazuhAgent) -> None:
    expected_token = _read_secret(agent.config.relay_token_file)

    class Handler(BaseHTTPRequestHandler):
        def _authorized(self) -> bool:
            return self.headers.get("Authorization", "") == f"Bearer {expected_token}"

        def do_GET(self) -> None:
            if not self._authorized():
                self.send_error(401)
                return
            alerts = agent.alerts.read()
            body = json.dumps({"alerts": alerts, "metrics": agent.alerts.metrics(alerts)}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self) -> None:
            if not self._authorized():
                self.send_error(401)
                return
            length = int(self.headers.get("Content-Length", "0"))
            if length <= 0 or length > MAX_REQUEST_BYTES:
                self.send_error(413)
                return
            try:
                request = json.loads(self.rfile.read(length))
                verdict = agent.triage(request["alert"])
                value = {"verdict": verdict, "proposals": agent.propose(verdict, request["session_id"])}
                status = 200
            except (KeyError, TypeError, ValueError, RuntimeError, OSError, urllib.error.URLError):
                value = {"error": "triage_failed"}
                status = 502
            body = json.dumps(value).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_: Any) -> None:
            return

    ThreadingHTTPServer((agent.config.host, agent.config.port), Handler).serve_forever()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mcp", action="store_true")
    args = parser.parse_args()
    agent = WazuhAgent(AgentConfig.from_env())
    mcp_loop(agent) if args.mcp else serve(agent)


if __name__ == "__main__":
    main()
