# PLAN — Jarvis hacia un agente SOC operativo

Última actualización: 2026-09-02, contra `origin/main@1092e01`.

Este documento es la ÚNICA fuente de verdad sobre qué falta y en qué orden.
Actualizalo vos mismo al cerrar cada etapa (marcar hecho, mover al historial
en STATUS.md, no borrar el registro). Si algo de este plan contradice lo que
encontrás en el código real, el código real gana — avisá la discrepancia
antes de seguir, no la resuelvas en silencio.

## Norte: qué significa "terminado"

Un agente SOC operativo significa que Jarvis puede:
1. Saber en qué estado está toda la infraestructura (autoobservabilidad).
2. Recordar conversaciones y decisiones pasadas (persistencia).
3. Responder preguntas sobre un host o alerta cruzando conocimiento
   estático de la infra con evidencia en vivo (RAG + Wazuh + Proxmox).
4. Agrupar alertas relacionadas en casos, no mostrarlas sueltas (correlación).
5. Tomar al menos una acción real autorizada, no solo proponerla (al final,
   a propósito — es el paso de mayor riesgo).

Todo esto sin romper NINGUNO de los guardrails de la sección siguiente.

## Guardrails no negociables (ADR-014 y sucesivos)

Estos principios ya están implementados y verificados. Ninguna tarea de
este plan puede violarlos, aunque parezca más simple hacerlo:

- **Un solo cerebro de razonamiento.** Solo `ConversationService` en Core
  decide y forma veredictos, usando los alias de LiteLLM
  (`jarvis-fast`, `jarvis-reasoning`, `jarvis-soc-l1`, `jarvis-soc-l2`).
  Ningún agente nuevo (System Monitor, Memory, o el que sea) puede tener su
  propio cliente LiteLLM independiente. Si necesita "razonar", la pregunta
  se resuelve en Core con la evidencia que el agente entrega.
- **Agentes proponen, nunca autorizan.** Todo agente de dominio nuevo sigue
  la forma de Wazuh Agent/Proxmox Agent: MCP tools de lectura + un set
  declarado de capacidades proponibles en `contracts/data/capabilities.json`.
  Verificado en código por `domain_agent_cannot_issue_its_own_grant` y
  `domain_agent_cannot_submit_human_confirmation` — cualquier agente nuevo
  necesita el mismo tipo de test.
- **Capacidades por tier, no autorización uniforme.** Tier 1 sin gate, tier
  2 con grant de un solo uso (5 min), tier 3 con confirmación tipeada +
  rollback_plan (2 min). Ver `docs/architecture.md`.
- **Fan-out paralelo, nunca cadena secuencial**, cuando se necesita
  evidencia de más de una fuente. Cada llamada a un modelo tiene timeout y
  límite de contexto explícitos (hoy: 20s LiteLLM, 8s retrieval RAG).
- **Documentación es evidencia, no aspiración.** Nada se documenta como
  "hecho" sin un test o verificación de producción citada. Ver `STATUS.md`
  para el formato exacto (4 categorías: verificado en producción / validado
  solo por tests / desplegado y deshabilitado a propósito / pendiente).
- **`RestrictedExecutor` permanece deshabilitado** hasta que el punto 7 del
  backlog lo habilite explícitamente, y solo para una capacidad chica.

## Disciplina de proceso (por qué existe, no la saltees)

- Cada branch se audita con evidencia real (grep/tests corridos, no
  confianza en el resumen de otra sesión) antes de mergear a `main`.
- `git push` y cualquier acción visible/compartida esperan confirmación
  explícita del operador — comandos de solo lectura (`git grep`,
  `git ls-tree`, `git show`) no la necesitan.
- Antes de escribir "está en producción" en cualquier doc, verificalo con
  un comando real contra el servicio real — no lo infieras de una
  conversación anterior.
- Un merge a `main` sin pasar por revisión ya generó una cita de test
  inventada una vez en este proyecto (corregido en `ceaa509`). No repetir.

## Backlog, en orden

### 0 — Cerrar lo que ya está aprobado y pausado (empezar por acá)

- [x] Terminar y mergear `feature/hud-real-agent-roster` (2026-09-02).
  El trabajo pendiente de la sesión anterior vivía sin commitear en un
  worktree (`git worktree list` lo mostraba apuntando a un directorio
  temporal de un job viejo). Al revisarlo se encontró código huérfano: al
  sacar el tile placeholder "SYSTEM MONITOR" del roster, quedaron 3 handlers
  muertos en `state.ts` y un mapeo `prometheus → monitor` sin nadie
  escuchando en `client.ts` (la telemetría de Prometheus se publicaba a un
  id que ya no existía en el modelo). Se limpió, se corrió la suite
  completa (`cargo test/clippy/fmt` en core, `npm test`/`npm run build` en
  desktop) y se commiteó como `45674d1`. Mergeado a `main` en `43a9809`.
- [ ] Desplegar el HUD nuevo a producción (build + Nginx), verificar con
  curl, avisar al operador para que lo confirme visualmente en el navegador.
  **Bloqueado**: no hay script de deploy en el repo (el commit `1739764`
  documenta que el redeploy anterior fue un paso operativo manual, "no en
  este diff"). El build local (`npm run build` en `apps/desktop`) genera
  `dist/` sin errores y está listo. SSH como usuario normal a `192.168.1.5`
  fue rechazado (publickey). `STATUS.md` documenta que sesiones anteriores
  de Claude Code usaron una key SSH root de Proxmox sin restricciones para
  cambios de producción — pero no hay host/IP de ese Proxmox documentado en
  el repo, y adivinar credenciales de root contra infraestructura de
  producción (que además corre Wazuh) no es un paso que tomar sin
  confirmación. **Necesito que el operador indique el mecanismo de deploy
  real** (¿qué host sirve `/usr/share/jarvis/web`? ¿qué credencial se usa?).
- [x] Mergear `feature/gpu-passthrough-hook-v2` (2026-09-02, merge `66575cc`).
  Auditado: solo config/telemetría de solo lectura (regla Prometheus, hook
  script de Proxmox, textfile exporter); nada ejecuta contra guests salvo
  `pct status`/`pct exec ... systemctl is-active`. YAML validado.
- [x] Mergear `fix/prometheus-disk-capacity` (2026-09-02, merge `1092e01`).
  Hubo conflicto real con el merge anterior: ambas branches agregaban un
  bloque `rule_files:` distinto en `deploy/prometheus/prometheus.yml`.
  Resuelto conservando ambas reglas (`jarvis-gpu.rules.yml` +
  `jarvis-storage.rules.yml`). YAML validado, `STATUS.md` fusionado sin
  pérdida de contenido de ninguna de las dos branches.
- [ ] Confirmar con el operador si el token de Cloudflare mencionado en
  `SESSION_CONTEXT.md` (branch `feature/voice-latency-instrumentation`, no
  mergeada) ya fue rotado. **Encontrado y confirmado como bloqueante real**:
  el token del túnel `cloudflared` quedó expuesto en output de comando
  porque estaba embebido en el `ExecStart` del unit de systemd; el propio
  documento dice tratarlo como comprometido. No se tocó nada de
  infraestructura de Cloudflare — requiere respuesta del operador antes de
  seguir.

Los tres merges de código de este punto ya están en `origin/main`
(`1092e01`, pusheado). Faltan las dos tareas bloqueadas por decisión/acceso
del operador antes de dar el punto 0 por cerrado y pasar al punto 1.

### 1 — Autoobservabilidad → System Monitor Agent

- [ ] Inventario de qué componentes propios de Jarvis NO tienen chequeo de
  salud hoy (evidencia real, no supuesto).
- [ ] Proponer reglas de Prometheus para cerrar los huecos (disco, servicio
  caído, restart loop, métrica ausente) — mismo patrón que
  `jarvis_gpu_passthrough_ok`.
- [ ] Proponer canal de notificación real (Telegram vía n8n u otro) para
  que las alertas lleguen a un humano, no solo vivan en `/alerts`.
- [ ] Con aprobación: `services/system-monitor-agent/` — MCP tools de solo
  lectura sobre esas métricas, capability tier 1 nueva en
  `capabilities.json`. Sin cliente LiteLLM propio.
- [ ] Actualizar HUD: System Monitor Agent con estado real.

### 2 — Persistencia → Memory Agent

- [ ] ADR proponiendo: tecnología (evaluar si un solo Postgres alcanza),
  esquema para conversaciones y para historial de decisiones de seguridad
  (veredicto + evidencia + decisión humana), control de acceso, retención,
  credenciales (OpenBao o `LoadCredential`), y postura explícita sobre si
  Codex puede leer este historial para razonar (cuidado: no reabrir el
  problema de múltiples cerebros).
- [ ] Con aprobación del ADR: `services/memory-agent/` — MCP tools de solo
  lectura sobre la base nueva, mismo patrón sin LiteLLM propio.
- [ ] Actualizar HUD: Memory Agent con estado real.

### 3 — Conocimiento de infraestructura (`qdrant-infra-rag` Paso 2)

Paso 1 (rebase, descarte del ADR duplicado, guardrails reconfirmados) ya
está aprobado. Falta:

- [ ] Indexar en Qdrant: ADRs, READMEs, `STATUS.md`, inventario derivado de
  `deploy/` (qué CT/VM es cada cosa). Indexar desde el `main` más reciente
  al momento de implementar esto, no desde una foto vieja.
- [ ] Cruce en paralelo (fan-out) entre evidencia estática (Qdrant) y
  evidencia en vivo (Wazuh Agent, Proxmox Agent, System Monitor Agent)
  cuando la pregunta lo amerite. Test nuevo estilo
  `cross_domain_evidence_is_requested_concurrently` pero con 3 fuentes.
- [ ] Mergear con la misma auditoría de siempre.

### 4 — Correlación de alertas

- [ ] Diseñar agrupación de eventos relacionados (mismo host/usuario,
  ventana de tiempo) en un caso único, en vez de alertas sueltas. Puede
  vivir en el Wazuh Agent (ya hace triage) o en Core — decidir con ADR
  corto antes de implementar.
- [ ] HUD: mostrar casos agrupados, no solo el stream de eventos crudo.

### 5 — Primera acción real autorizada (al final, a propósito)

- [ ] Elegir UNA capacidad tier 2 de bajo riesgo (ejemplo:
  `security.ip.block`) para ser la primera en tener un
  `RestrictedExecutor` real, no el `DisabledExecutor`.
- [ ] Implementar ese executor específico, con su propio backup/rollback
  y auditoría reforzada.
- [ ] Probar en un entorno controlado antes de dejarlo activo en
  producción — coordinar con el operador una ventana de prueba.
- [ ] Recién después de que esto funcione de forma confiable, evaluar
  sumar más capacidades.

## Cómo trabajar este plan

Una tarea del backlog a la vez, en el orden numerado, salvo que el
operador pida explícitamente reordenar. Al cerrar una tarea: actualizar
este archivo (tachar o mover a "hecho") y actualizar `STATUS.md` con la
evidencia real. Cualquier decisión de diseño que este plan deja abierta
(marcada como "proponer" o "decidir con ADR") necesita aprobación del
operador antes de implementarse — no asumas la respuesta.
