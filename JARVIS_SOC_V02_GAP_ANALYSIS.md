# JARVIS SOC Triage & Investigation v0.2 — GAP Analysis

Fecha de discovery: 2026-09-04
Repositorio inspeccionado: `/home/d4rkn0d3/Projects/jarvis`
Branch: `feature/qdrant-infra-rag` (`98acaa6`, alineada con `origin/feature/qdrant-infra-rag`)
Alcance: inspección local, sin cambios funcionales ni consultas/escrituras a producción.

## Resumen ejecutivo

JARVIS ya posee los límites arquitectónicos que v0.2 debe preservar: Core es el coordinador y límite de autorización; el executor productivo está deshabilitado; existen políticas Tier 1/2/3, grants de un solo uso, autenticación por credenciales hasheadas, sesiones HttpOnly + CSRF, WebSocket autenticado, Voice Engine, Wazuh, Prometheus, Qdrant, LiteLLM, MCP y un frontend único.

La implementación SOC transaccional está iniciada en cambios locales todavía no versionados. La validación read-only de 2026-09-04 confirmó en CT133 PostgreSQL 15.19 las tablas `assets`, `soc_cases` y `case_events`, sin migration history, triggers, views, enums ni funciones públicas. El código crea/actualiza High/Critical, consulta criticidad, deduplica por `alert_id` y agrupa por host en 30 minutos.

Los mayores gaps son: contrato Wazuh incompleto (se pierden MITRE, usuario y entidades), falta de Evidence Package persistente, motores deterministas de Risk/Confidence, assessments L1/L2 coordinados por Core, SLA/state machine, API SOC, UI SOC, protocolo realtime SOC, métricas de aplicación y propuestas Tier 2 ligadas a casos. No se recomienda crear servicios nuevos.

## Evidencia y limitaciones del discovery

- Working tree preexistente: 21 archivos modificados y 3 no rastreados (`PLAN.md`, `scripts/test_rag_index.py`, `services/core/src/soc_cases.rs`), aproximadamente 1.722 adiciones. No fue alterado.
- Commits recientes relevantes: `98acaa6` Qdrant RAG, `f7c1213` consultas Wazuh por host, `8d86c22` latencia de voz, `0926ea9` reconciliación ADR-014. `main` está en `53d41f7`.
- No existe `migrations/`, archivos `.sql` ni schema DDL versionado.
- La pre-fase 0.5 verificó mediante Proxmox CT112, CT116, CT120, CT124 y CT133. No se inspeccionó ningún secreto ni dato SOC.
- El workflow n8n live `SOC 2.0` está activo y difiere del export versionado en normalización/Telegram y flag active.
- Hay dos caminos de normalización Wazuh (`wazuh-agent` y `wazuh-relay`) y un tercero en n8n, con contratos divergentes.

## GAP Analysis

| CAPABILITY | STATUS | CURRENT IMPLEMENTATION | GAP | PROPOSED CHANGE | RISK |
|---|---|---|---|---|---|
| Core como único coordinador | EXISTS | `main.rs`, `gateway.rs`, `policy.rs`; browser y agentes pasan por Core | La triage del Wazuh Agent llama LiteLLM directamente; debe quedar subordinada a una capability coordinada por Core | Reutilizar adapters, mover la orquestación de assessment al Core sin crear microservicio | Medio: cambio de ownership sin cortar alertas actuales |
| Working tree / branch / historial | DO_NOT_MODIFY | Branch y cambios locales descritos arriba | Gran cantidad de trabajo no versionado puede solaparse con v0.2 | Crear checkpoint/commit del trabajo actual antes de Fase 1 y mantener commits pequeños | Alto: pérdida o mezcla de cambios del operador |
| Frontend único | EXISTS | `apps/desktop`, runtime web/Tauri, componentes HUD y CSS existentes | No hay navegación/vistas SOC | Extender la app actual y sus tokens/componentes | Medio |
| Wazuh poller Core | PARTIAL | `WazuhSecurityPoller` consulta relay cada 10 s, timeout 4 s, máximo 20 alertas | No cursor/checkpoint; repite ventana; payload limitado | Mantener poller, añadir cursor/idempotencia observable y contrato enriquecido | Medio |
| Relay Wazuh read-only | PARTIAL | Relay autenticado lee últimas 500 líneas y entrega importantes + recientes | Timestamp se reemplaza por hora de lectura; falta `source_ip`; no MITRE/entidades | Unificar normalizador conservando timestamp Wazuh y nulls explícitos | Alto: orden temporal y dedup incorrectos |
| Wazuh Agent | PARTIAL | Read/triage, límites 12 KiB/8 s, aliases L1/L2, propuestas allow-listed | Escala a L2 sólo por level; verdict legacy; no Evidence Package; acceso directo a LiteLLM | Convertirlo en adapter de dominio llamado por Core; no permitir decisiones/ejecución | Alto |
| Normalización Wazuh | PARTIAL | Extrae id, host, agent_id, level, rule_id, title, user, source IP | Pierde groups/frequency/MITRE/dstip/users/process/parent/cmd/hash/file/decoder/location; usa defaults inventados (`unknown`, UUID, `000`) | Contrato v2 común; campos ausentes `null`, evidencia raw referenciada y acotada | Alto: alucinación/provenance |
| Deduplicación por alert_id | EXISTS | `SocCaseStore` evita reinsertar IDs ya presentes en un caso; frontend deduplica por id | CT133 confirma que no hay constraint global por alert ID y poller reemite alertas | Preservar lógica y agregar identidad idempotente en evidencia nueva | Medio |
| Case manager / persistencia | PARTIAL | `SocCaseStore` crea/actualiza High/Critical y agrega `case_events` | Sólo ingesta; no API, assignment, transitions, assessments, SLA o auditoría de caso | Extender store/repository en módulos pequeños | Alto |
| Prioridad inicial P1-P4 | PARTIAL | Critical→P1, High+asset critical→P1, High→P2 | Sólo P1/P2 en flujo actual; campo `priority` mezcla inicial/final | Preservar algoritmo como `initial_priority`; calcular `final_priority` tras assessment | Medio |
| Correlación baseline 30 min | EXISTS | Caso abierto/investigating del mismo host dentro de 30 min | Hardcoded; sólo host; n8n usa otra ventana de 5 min host+user | Configurar 30 min por defecto y sumar user/IP/MITRE/groups sin reemplazar baseline | Alto: sobre/subagrupación |
| Correlación investigativa | PARTIAL | Respuesta conversacional agrupa host/IP y aplica heurística textual MITRE | No persistente; deduce MITRE por texto; no relaciones/chain confidence | Motor puro y testeable sobre evidencia estructurada Wazuh | Alto |
| MITRE desde Wazuh | MISSING | Normalizadores no preservan `rule.mitre`; conversación infiere técnicas por keywords | Viola prioridad de usar MITRE provisto por Wazuh | Ingerir IDs/tácticas/técnicas; clasificar RELATED/POSSIBLE/HIGH_CONFIDENCE con trazabilidad | Alto |
| PostgreSQL SOC CT133 | PARTIAL | PostgreSQL 15.19 verificado; tablas `assets`, `soc_cases`, `case_events`; schema-only versionado | Sin migration history ni assessment/feedback/SLA estructurados | Migraciones aditivas explícitas; no crear DB ni reemplazar tablas | Alto |
| Credencial DB SOC | EXISTS | Drop-in live `soc-db.conf` carga `soc-db-password`; source root:root 0400; Core activo | Drop-in no está versionado en repo | Añadir template sanitizado en fase de deployment, sin tocar live | Medio |
| Evidence Package | MISSING | Hay eventos JSON y contexto conversacional acotado | No contrato, provenance ni persistencia de componentes | Crear modelos/contracts, refs y límites; separar security evidence de ops context | Alto |
| Timeline | MISSING | `case_events.occurred_at` aporta una base | No API/modelo normalizado ni relación MITRE | Derivar cronológicamente de evidencia persistida, sin timestamps inventados | Medio |
| Risk Engine determinista | MISSING | Severidad/prioridad simple | Sin score, factores, versión ni explicación | Módulo puro/config versionada; LLM nunca fija score | Alto |
| Confidence Engine estructurado | MISSING | Wazuh Agent devuelve ALTA/MEDIA/BAJA del LLM | Sin componentes validados, contradicciones o versión | Módulo Core que normalice evidencia y componentes del modelo | Alto |
| AI Verdict v0.2 | PARTIAL | Schema legacy: FALSO_POSITIVO y AMENAZA_REAL_* | No cinco verdicts requeridos ni historial | Nuevo contrato backward-compatible y tabla de assessments | Alto |
| Analyst Verdict separado | MISSING | No endpoints/campos | Riesgo de sobrescribir AI verdict | Campos/eventos separados, reason/comment obligatorios y auditados | Alto |
| L1 alias | EXISTS | Live CT116 confirma `jarvis-soc-l1` → Ollama llama3.2, temperature 0.1 | Inferencia/response-format no probados; output insuficiente | Validar luego con synthetic fixture y actualizar schema | Medio |
| L2 alias / escalation | PARTIAL | Live confirma `jarvis-soc-l2` → qwen2.5, temperature 0.05; selección actual sólo level >=12 | No criterios de incertidumbre/risk/chain/P1/manual ni evidence loop | Escalation policy Core + segundo Evidence Package acotado | Alto |
| LiteLLM gateway | EXISTS | Core/Voice y Wazuh Agent usan URLs/tokens privados, timeouts y aliases | Config versionada es fragmento desired-state, no evidencia live | Mantener; inventariar config/keys sin secretos y añadir métricas de latencia | Bajo |
| Qdrant knowledge/RAG | EXISTS | `KnowledgeClient`, colección dedicada y embedding alias; fail-soft y bounded | No similar SOC cases/resolution summaries | Agregar retrieval específico de casos con IDs, sin source-of-truth duplicada | Medio |
| Prometheus context | EXISTS | Adapter read-only, consultas acotadas y health de dependencias | Core no exporta métricas SOC; sin contexto per-case formal | Reutilizar para operational context; definir endpoint/scrape compatible existente | Medio |
| Métricas SOC | MISSING | Hay telemetría consumida y logs de timing de voz | No métricas `jarvis_soc_*`; ADR-011 dice Core no expone `/metrics` | Elegir mecanismo existente (textfile o endpoint privado revisado), labels de baja cardinalidad | Medio |
| n8n | PARTIAL | Live: 33 workflows; `SOC 2.0` activo, 4 nodos; históricos SOC/action inactivos | Live normalizer v3/Telegram difieren del repo; active flag divergente | Reconciliar export sin modificar live; mantener sólo enrichment/notifications | Alto |
| WebSocket autenticado | EXISTS | `/ws`, session cookie, exact Origin, payload/input bounded, resync | No eventos SOC del protocolo objetivo | Extender `EventType`/contracts y parser frontend con payload mínimo | Medio |
| Voice Engine | EXISTS | `/api/v1/voice/alert`, TTS server-side, límites, CSRF, dedup/coalescing frontend | Habla toda High/Critical antes de assessment; sin mute/repeat/preference | Cambiar trigger bajo flag a assessment 90/90; reutilizar endpoint/dedup | Alto: alert fatigue o silencio |
| Critical rule 90/90 | MISSING | No score ni evento | No persistencia-before-notify, popup, delivery/ack | Servicio transaccional/outbox: persist→P1/SLA→event; no quarantine automática | Crítico |
| Dashboard / My Cases / Case Detail | MISSING | Sólo panel de alertas y Agent Matrix | No consola operativa | Vistas dentro del frontend actual, lazy data + realtime incremental | Medio |
| API SOC | MISSING | Rutas actuales health/agents/session/requests/voice/ws | No endpoints cases/dashboard/actions | Extender router y patrones auth/CSRF/body/timeouts; versionar DTOs | Alto |
| SLA backend | MISSING | No deadlines/status/worker | Browser no debe ser autoridad | Config versionada, timestamps DB, scheduler idempotente y eventos warning/breach | Alto |
| Case state machine | MISSING | Persistencia reconoce estados string `open`,`investigating` | No transición validada/auditada; nombres difieren del objetivo | Mapear estados reales tras dump y migrar de forma compatible | Alto |
| Assignment | MISSING | Campo conceptual no probado | Sin API/RBAC/timestamps | Operaciones atómicas con assigned_at y auditoría | Medio |
| RBAC SOC | PARTIAL | Roles actuales `desktop/operator/wazuh-agent/proxmox-agent`; backend valida clases amplias | No SOC_L1/L2/MANAGER; policy actual permite cualquier rol catalogado para cualquier capability catalogada | Añadir permisos por operación SOC y tests fail-closed, preservando roles legacy | Crítico |
| Tier policies | EXISTS | Catálogo Tier 1/2/3 embebido; deny unknown | `required_evidence` y `owner_agent` no se aplican en policy | No debilitar; para proposals validar evidencia/case binding antes de grants | Alto |
| One-use grants | EXISTS | Session+subject+capability+target, TTL, consume-once | No vínculo a case/proposal; storage en memoria | Reutilizar y ligar proposal ID/case; mantener expiración y single-use | Alto |
| RestrictedExecutor | DO_NOT_MODIFY | `DisabledExecutor` retorna `EXECUTOR_DISABLED` | Ninguno para v0.2; ejecución queda fuera de alcance | Mantener deshabilitado; tests deben verificar NOT_EXECUTED/SIMULATED | Crítico si se habilita accidentalmente |
| Tier 2 quarantine proposal | PARTIAL | Capability `security.host.isolate` y propuesta Wazuh→Core existen | Core puede llegar al executor tras grant (aunque está disabled); no proposal record/case/UI/impact/rollback | Nueva entidad proposal y simulación explícita, sin llamar executor en este milestone | Crítico |
| Autenticación | EXISTS | SHA-256 constant-time, registry systemd, principal server-owned | Bootstrap bearer sigue siendo transitorio | No cambiar en v0.2; reutilizar principal/session | Bajo |
| Sesiones / CSRF / Origin | EXISTS | HttpOnly Secure SameSite Strict; CSRF en writes; exact HTTPS origin | Nuevas rutas SOC aún no integradas | Aplicar los mismos guards a toda mutación SOC | Alto si se omite |
| Auditoría | PARTIAL | Journal `JARVIS_AUDIT` con request/subject/capability/target/outcome | Sin before/after/source/success/case; sink no transaccional | Audit events SOC estructurados, sin secretos; outbox/DB para operaciones críticas | Alto |
| Logging | PARTIAL | Errores sanitizados y timing voz; evita contenido sensible | Sin request_id consistente en jobs SOC ni latencias L1/L2 | Logging estructurado con IDs y timings, sin evidence raw/secrets | Medio |
| MCP | EXISTS | Gateway privado, read-only tools y tests; arquitectura Core→LiteLLM→MCP | No herramientas SOC cases/investigate; no necesarias para UI/API inicial | Extender sólo si voice intent routing no puede reutilizar API interna | Medio |
| Feature flags | MISSING | No sistema de flags identificado | Releases SOC no aislables | Config existente vía env, defaults off para capacidades nuevas | Medio |
| Tests Core | EXISTS | 100 tests Rust pasan con all-features | `SocCaseStore` no tiene tests; DB integration ausente | Añadir unit + integration DB efímera para cada fase | Alto |
| Tests frontend | EXISTS | 27 tests y build pasan | Sin vistas/eventos/modal SOC | Fixtures anonimizadas y tests UI/realtime/accessibility | Medio |
| Tests agentes Python | EXISTS | 17 tests explícitos pasan; RAG 2 pasa | `unittest discover -s services` ejecuta 0 por layout | Añadir runner/CI explícito o packages; no ocultar el problema | Bajo |
| CI | PARTIAL | `.github/workflows/ci.yml` existe | Verificar que ejecute all-features, frontend y todos los Python | Actualizar CI sólo al agregar tests v0.2 | Medio |
| Documentación / ADR | EXISTS | Arquitectura, seguridad, auth, realtime, voice, MCP, ADR-001..014 | Algunos docs describen desired state y contradicen código/export actual | Actualizar tras cada fase y registrar ADRs v0.2 | Medio |
| Networking/perímetro | DO_NOT_MODIFY | Binding privado, Nginx, nftables, Cloudflare/Tailscale documentados | Ningún cambio requerido para v0.2 | Mantener puertos/orígenes/rutas existentes; documentar antes de cualquier excepción | Crítico |
| Secretos / OpenBao | DO_NOT_MODIFY | LoadCredential actual; OpenBao futuro | OpenBao no reemplaza bootstrap actual | Fuera de alcance; corregir sólo wiring faltante de credential SOC tras verificar live | Alto |

## Baseline de validación

| Suite | Resultado |
|---|---|
| `cargo test --workspace --all-features` | PASS: 100 tests, 0 fallos |
| `npm test -- --run` (`apps/desktop`) | PASS: 27 tests en 8 archivos |
| `npm run build` (`apps/desktop`) | PASS: TypeScript + Vite |
| Wazuh Agent | PASS: 7 tests |
| MCP Gateway | PASS: 4 tests |
| Proxmox Agent | PASS: 4 tests |
| RAG index | PASS: 2 tests |
| `python -m unittest discover -s services` | 0 tests descubiertos; se compensó con ejecución explícita |

## Bloqueos previos a Fase 1

1. Crear un checkpoint seguro del working tree actual; no mezclar v0.2 con cambios no versionados sin acuerdo.
2. Obtener desde CT133 un dump de schema-only/read-only (`pg_dump --schema-only` o consultas a `information_schema`) y guardar una copia sanitizada aprobada.
3. Inventariar migrations aplicadas o confirmar que hoy no existe mecanismo de migración.
4. Exportar metadata/workflows n8n activos para reconciliar el template `active:false` y la correlación 5m.
5. Verificar configuración live de aliases LiteLLM y permisos de sus virtual keys sin extraer tokens.
6. Verificar la unidad systemd live de Core y el wiring real de `soc-db-password`.

Hasta completar estas verificaciones, los campos exactos y archivos de migración de Fase 1 son propuestas, no afirmaciones sobre producción.
