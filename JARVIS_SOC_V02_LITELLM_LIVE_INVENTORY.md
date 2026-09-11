# LiteLLM Live Inventory

Status: **VERIFIED CONFIGURATION / NOT INFERENCE-TESTED**
Observed: 2026-09-04 through Proxmox `pct exec 116`

## Runtime

- CT116 `originalOllama`, running.
- `litellm.service` active/running; Ollama also active.
- Listen port 4000; configuration `/etc/litellm/config.yaml`.
- Environment values and tokens were not read.

| ALIAS | LIVE MODEL | PROVIDER | TEMPERATURE | CONTEXT CONFIG | STATUS |
|---|---|---|---:|---|---|
| `jarvis-fast` | `llama3.2:1b` | Ollama local | default | not declared | EXISTS |
| `jarvis-reasoning` | `qwen2.5` | Ollama local | 0.05 | not declared | EXISTS |
| `jarvis-soc-l1` | `llama3.2` | Ollama local | 0.1 | not declared | EXISTS |
| `jarvis-soc-l2` | `qwen2.5` | Ollama local | 0.05 | not declared | EXISTS |
| `jarvis-embed-multilingual` | `bge-m3` | Ollama local | n/a | embedding mode | EXISTS |

All use loopback Ollama port 11434. `router_settings` is empty; fallbacks and context-window fallbacks are absent; `drop_params=true`. Required aliases have no declared per-alias timeout/max_tokens, so client limits remain mandatory.

Repo/live difference: repo maps `jarvis-fast` to `llama3.2`, while live uses `llama3.2:1b`. Required SOC/reasoning/embedding mappings otherwise align. No inference was sent, so runtime response-format behavior remains a later validation item and does not block deterministic scoring work.
