#!/usr/bin/env python3
"""Minimal authenticated, read-only Wazuh alert relay for JARVIS Core."""
import json, os, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ALERTS = "/var/ossec/logs/alerts/alerts.json"
TOKEN_FILE = os.environ.get("JARVIS_WAZUH_RELAY_TOKEN_FILE", "/etc/jarvis-wazuh-relay/token")
HOST = os.environ.get("JARVIS_WAZUH_RELAY_HOST", "192.168.1.10")
PORT = int(os.environ.get("JARVIS_WAZUH_RELAY_PORT", "5515"))

def token():
    with open(TOKEN_FILE, encoding="utf-8") as f: return f.read().strip()

def normalized(line):
    try: raw = json.loads(line)
    except (ValueError, TypeError): return None
    rule = raw.get("rule") or {}
    level = int(rule.get("level") or 0)
    severity = "critical" if level >= 12 else "high" if level >= 10 else "medium" if level >= 7 else "low"
    stamp = raw.get("timestamp") or ""
    agent = raw.get("agent") or {}
    host = str(agent.get("name") or agent.get("hostname") or raw.get("hostname") or "unknown")[:128]
    return {"id": str(raw.get("id") or raw.get("full_log", "")[:32]), "host": host, "timestamp_ms": int(time.time()*1000),
            "severity": severity, "title": str(rule.get("description") or "Wazuh alert")[:160],
            "description": str(raw.get("full_log") or rule.get("description") or "Security event")[:500]}

def read_alerts():
    try:
        with open(ALERTS, encoding="utf-8", errors="replace") as f: lines = f.readlines()[-20:]
    except OSError: lines = []
    alerts = [a for line in lines if (a := normalized(line))]
    return alerts[-10:]

def metrics(alerts):
    def count(*words):
        return sum(1 for item in alerts if any(word in (item["title"] + " " + item["description"]).lower() for word in words))
    return {"timestamp_ms": int(time.time()*1000), "alert_count": len(alerts),
            "failed_logins": count("authentication failed", "failed login", "invalid user"),
            "sudo_commands": count("sudo", "privilege escalation"),
            "new_processes": count("new process", "process created"),
            "fim_changes": count("file integrity", "modified file", "file changed"),
            "network_connections": count("connection", "network", "port")}

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.headers.get("Authorization", "") != "Bearer " + token(): self.send_error(401); return
        alerts = read_alerts()
        body = json.dumps({"alerts": alerts, "metrics": metrics(alerts)}).encode()
        self.send_response(200); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *_): pass

ThreadingHTTPServer((HOST, PORT), Handler).serve_forever()
