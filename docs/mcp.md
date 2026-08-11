# MCP integration

The production path is `JARVIS Core -> LiteLLM MCP Gateway -> jarvis-mcp -> Proxmox API`. The browser never receives MCP, LiteLLM, or Proxmox credentials.

The private `jarvis-mcp` service implements Streamable HTTP-compatible JSON-RPC for protocol revisions `2025-06-18` and `2025-11-25`. LiteLLM prefixes upstream tools with the registered server alias.

Currently enabled tools are read-only:

- `proxmox.vm.list`: reduced inventory for pool `JARVIS`.
- `proxmox.vm.status`: normalized status for allow-listed VMIDs 124 and 125.

LiteLLM server ID `jarvis_proxmox` is not public and is assigned only to team/key `jarvis-core`. Tool permissions are explicit. The Core maps model-facing aliases to namespaced MCP tools, validates arguments again, permits a single read tool per turn, validates the structured result, then makes a second model call to formulate the response.

Proxmox uses separate privilege-separated tokens for READ and MODIFY. The MODIFY token has only `VM.Audit` and `VM.PowerMgmt`, but no tool or execution path consumes it until the Authorization Service issues concrete, expiring, one-time grants. DESTRUCTIVE operations remain unavailable.
