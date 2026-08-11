# ADR-008: Model routing and agent routing are separate

Status: accepted

LiteLLM selects configured model aliases. JARVIS Capability Router selects capabilities and agents. The router never embeds provider model names, and LiteLLM does not become the authorization authority for Codex, MCP, security or automation.
