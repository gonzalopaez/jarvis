# JARVIS session context

Last updated: **2026-08-13** (`America/Argentina/Buenos_Aires`).

Read this file before continuing work in a future session. It is a concise
handoff of verified production state and current repository state. It contains
no credentials.

## Repository state

- Working repository: `/home/d4rkn0d3/Projects/jarvis`.
- Current branch: `feature/voice-latency-instrumentation`.
- Current pushed commit: `0443f8b` (`gate host alert details behind confirmation`).
- The branch is clean and matches `origin/feature/voice-latency-instrumentation`.
- Changes described below are deployed, but the branch has **not** been
  documented here as merged to `main`. Verify merge ancestry before claiming
  that `main` contains them.
- Earlier consolidated baseline used for documentation was `main@a2f37e0`.
- Infrastructure RAG remains pending; do not describe it as implemented merely
  because it was conceptually approved.

## Production topology

- Proxmox host: `192.168.1.5`.
- Core: CT124, `192.168.1.21:4100`.
- Voice: CT125, `192.168.1.22:4200`.
- MCP: CT126, `192.168.1.23:4300`.
- Prometheus: CT127, `192.168.1.24:9090`.
- Wazuh relay/agent endpoint: `192.168.1.10:5515`.
- Ollama/LiteLLM: CT116, `192.168.1.11`.
- No JARVIS server component should be started locally on the workstation.

## Ollama GPU correction

- Host GPUs: Intel UHD 750 and AMD Radeon RX 5600 XT.
- CT116 is an LXC. It previously saw only Mesa `llvmpipe`; GPU bind mounts had
  failed because the container started before `/dev/kfd`, `/dev/dri/card1` and
  `/dev/dri/renderD129` existed.
- `/dev/kfd` currently uses major `511`; CT116 cgroup config was corrected from
  the obsolete major `509`.
- Proxmox hook `deploy/proxmox/ollama-gpu-passthrough-hook.sh` waits during
  `pre-start` for real character devices before native LXC mounts run.
- After a clean stop/start, CT116 sees `AMD Radeon RX 5600 XT (RADV NAVI10)` as
  a discrete GPU. Ollama offloads 17/17 Llama layers.
- Exact benchmark (`llama3.2:1b`, 78 prompt tokens, 192 generated): total
  3.288 s, cold load 1.957 s, generation 166.07 tokens/s. The existing
  `LLM_DEADLINE=20s` is realistic; it was not changed.
- Prometheus textfile metric:
  `jarvis_gpu_passthrough_ok{vmid="116"} 1`.
- Related pushed commit: `bce1925`.

## Live infrastructure queries

- Core now queries current Prometheus vectors instead of relying on transient
  alert history for availability questions.
- Verified production answers:
  - General down list: `dc` VM106, `freeipa` VM108 and
    `cloudflare-tunnel/cloudflared` VM105.
  - VPN: `tailscale-vpn/tailscaled` VM109 is online.
  - Cloudflare tunnel: CT105 is up but `cloudflared` is down/degraded.
  - Firewall: speech/text aliases `firewall`, `pfSense`, `psfesense` and
    `OPNsense` resolve to VM102 (`opnsense`), currently online.
- Availability responses are deterministic, use `INFRASTRUCTURE_AGENT`, and
  fail closed if Prometheus is unavailable.
- Related pushed commit: `c4e6812`.

## Wazuh queries by equipment

- The relay returns the latest 20 normalized alerts, including a `host` field.
- Core dynamically extracts equipment names after `equipo`, `host` or
  `máquina`; names are not hard-coded.
- Current relay evidence at verification time: all latest 20 alerts belonged
  to host `Romina`.
- Required interaction, scoped to one session and single-use:
  1. `¿El equipo Romina tiene alertas?`
  2. Core replies only with the count and asks:
     `¿Necesitás que te detalle las alertas? Sí o no.`
  3. `sí` returns at most five alert details.
  4. `no` replies `Entendido, no voy a detallar las alertas.`
- The initial count response does not include alert titles. A missing host does
  not fall back to alerts from other hosts.
- Alert-detail confirmation is separate from mitigation authorization.
- Related pushed commits: `995bc56`, `0443f8b`.

## Voice state and open audio issue

- Voice latency instrumentation logs only request ID and numeric stage timings:
  capture upload, STT, routing, LLM, TTS, transfer and total.
- With real GPU acceleration, LLM latency is no longer the primary bottleneck.
- User reports audible crackle/static (“lluvia”) in synthesized speech.
- Direct Piper diagnostic WAV is valid PCM signed 16-bit little-endian, mono,
  22,050 Hz, but reaches exactly 0 dBFS. FFmpeg did not detect sustained clipped
  samples. The artifact is already present or suspected before Core/UI; next
  safe step is an isolated A/B comparison of:
  - current `es_AR-daniela-high` parameters;
  - lower Piper noise scale;
  - another installed Piper voice.
- Do not change production voice parameters without presenting/listening to the
  A/B result first.

## Cloudflare WAF gap and security note

- Tunnel state and WAF state are different. Jarvis can currently verify the
  tunnel service but cannot query Cloudflare WAF/rulesets.
- No read-only Cloudflare API integration or n8n workflow was found.
- A separate least-privilege Cloudflare API token is required for WAF/ruleset
  and optional security analytics queries. Do not reuse the tunnel token.
- During diagnosis, the cloudflared tunnel token was exposed in command output
  because it is embedded in the systemd `ExecStart`. Treat it as compromised:
  rotate it and move it out of the unit file. Never copy its value into Git or
  future session notes.

## Safety invariants

- Production uses `DisabledExecutor`; infrastructure/security agents propose
  actions but cannot execute or self-confirm them.
- Human confirmation remains external and session-scoped.
- MCP gateway remains read-only.
- `jarvis-soc-l1`, `jarvis-soc-l2`, `jarvis-reasoning` and
  `jarvis-technical` were not changed during the GPU/model investigation.
- No model aliases or `LLM_DEADLINE` were changed.

## Verification summary

- Latest Core suite after the alert-consent flow: 85 tests passed across all
  targets; clippy passed with `-D warnings`.
- Deployed Core hash after the final consent change:
  `8cf0c03b8119fd5ee1d7b0aff0b04455d39083952affdf3ca850391d9a2133a0`.
- Latest Core deployment backup:
  `/usr/local/bin/jarvis-core.backup-alert-consent-20260813T020508Z` in CT124.

## Recommended next steps

1. Rotate and securely store the exposed Cloudflare tunnel token.
2. Decide whether to add a separate read-only Cloudflare WAF adapter.
3. Run the isolated Piper A/B quality comparison for the crackling audio.
4. Merge/reconcile the current feature branch before updating final stage-6
   documentation; verify ancestry rather than assuming deployment equals merge.

