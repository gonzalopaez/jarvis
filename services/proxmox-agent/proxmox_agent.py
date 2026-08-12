#!/usr/bin/env python3
"""Proposal-only Proxmox domain agent for ADR-014."""
from __future__ import annotations
import json, os, re, sys, urllib.error, urllib.request, uuid
from pathlib import Path
from typing import Any

CORE_TIMEOUT_SECONDS = 8
MAX_RESPONSE_BYTES = 128 * 1024
MAX_PARAMETERS_BYTES = 12 * 1024
CAPABILITIES = {"proxmox.vm.deploy", "proxmox.vm.destroy", "proxmox.ct.destroy"}

class ProxmoxAgent:
    def __init__(self, core_url: str, token_file: str):
        self.core_url, self.token_file = core_url.rstrip("/"), token_file

    @classmethod
    def from_env(cls) -> "ProxmoxAgent":
        return cls(os.environ["JARVIS_CORE_URL"], os.getenv("JARVIS_CORE_TOKEN_FILE", "/run/credentials/core-token"))

    def propose(self, capability: str, target: str, parameters: dict[str, Any], session_id: str) -> dict[str, Any]:
        if capability not in CAPABILITIES: raise ValueError("capability is not allowlisted")
        if not re.fullmatch(r"[a-z0-9_.:-]{1,128}", target): raise ValueError("target does not match the Core contract")
        if not re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", session_id): raise ValueError("session_id does not match the Core contract")
        if not isinstance(parameters, dict) or len(json.dumps(parameters).encode()) > MAX_PARAMETERS_BYTES: raise ValueError("parameters exceed the explicit 12 KiB limit")
        token = Path(self.token_file).read_text(encoding="utf-8").strip()
        if len(token) < 20: raise RuntimeError("Core credential is invalid")
        request_id = f"proxmox-{uuid.uuid4().hex}"
        payload = {"api_version":"v1","request_id":request_id,"session_id":session_id,"kind":"action","action":{"capability":capability,"target":target,"parameters":parameters}}
        request = urllib.request.Request(f"{self.core_url}/api/v1/requests", data=json.dumps(payload,separators=(",",":")).encode(), method="POST", headers={"Authorization":f"Bearer {token}","Content-Type":"application/json"})
        try:
            with urllib.request.urlopen(request, timeout=CORE_TIMEOUT_SECONDS) as response: raw = response.read(MAX_RESPONSE_BYTES + 1)
        except urllib.error.HTTPError as error: raw = error.read(MAX_RESPONSE_BYTES + 1)
        if len(raw) > MAX_RESPONSE_BYTES: raise RuntimeError("Core response exceeded limit")
        return {"request_id":request_id,"capability":capability,"core_response":json.loads(raw)}

def mcp_loop(agent: ProxmoxAgent) -> None:
    tools = [{"name":capability,"description":"Submit a tier-3 proposal to Core. Never executes Proxmox operations.","inputSchema":{"type":"object","additionalProperties":False,"required":["target","parameters","session_id"],"properties":{"target":{"type":"string","pattern":"^[a-z0-9_.:-]{1,128}$"},"parameters":{"type":"object"},"session_id":{"type":"string"}}}} for capability in sorted(CAPABILITIES)]
    for line in sys.stdin:
        request: Any = None
        try:
            request=json.loads(line); method=request.get("method")
            if method=="initialize": result={"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"jarvis-proxmox-agent","version":"1.0.0"}}
            elif method=="tools/list": result={"tools":tools}
            elif method=="tools/call":
                params=request.get("params") or {}; args=params.get("arguments") or {}; value=agent.propose(params.get("name",""),args["target"],args["parameters"],args["session_id"]); result={"content":[{"type":"text","text":json.dumps(value)}]}
            else: continue
            print(json.dumps({"jsonrpc":"2.0","id":request.get("id"),"result":result}),flush=True)
        except Exception as error: print(json.dumps({"jsonrpc":"2.0","id":request.get("id") if isinstance(request,dict) else None,"error":{"code":-32000,"message":str(error)[:256]}}),flush=True)

if __name__ == "__main__": mcp_loop(ProxmoxAgent.from_env())
