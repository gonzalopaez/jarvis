# JARVIS SOC Triage & Investigation v0.2 — Implementation Plan

Fecha: 2026-09-04
Estado: propuesta posterior a discovery; requiere aprobación explícita antes de Fase 1.
Principio: extender componentes existentes, cambios aditivos, flags por defecto desactivados y ningún cambio perimetral.

## Precondiciones

- Preservar/checkpoint del working tree actual.
- Auditar schema-only de CT133 y workflows activos n8n con acceso read-only.
- Confirmar aliases/keys LiteLLM y unidad systemd Core live sin revelar secretos.
- Elegir herramienta de migración acorde al mecanismo real. Si no existe, introducir una única herramienta mínima dentro de Core/deploy; no ejecutar DDL ad hoc.
- Fijar baseline de tests indicado en el GAP Analysis como gate de cada commit.

## Fase 1 — Data model

Objetivo: versionar el schema real y añadir sólo lo faltante.

Cambios previstos:

- Nuevo directorio tentativo `services/core/migrations/` (nombre final depende del discovery live).
- Migraciones aditivas para scores, verdicts separados, prioridades, timestamps SLA/case, versiones y assessment history.
- Preferir tablas normalizadas `case_assessments`, `case_evidence`, `case_activity` y/o `action_proposals` sólo si el schema real no posee equivalentes.
- Índices idempotentes para alert ID, estado/prioridad/SLA y timestamps; sin índices de texto indiscriminados.
- `services/core/src/soc_cases.rs`: separar repository de lógica, conservar ingest existente.
- `deploy/systemd/jarvis-core.service`: sólo si el live audit confirma el `LoadCredential` faltante.

Migraciones:

1. Snapshot/baseline no ejecutable del schema live sanitizado.
2. Add columns/enums/check constraints compatibles; evitar reemplazar columnas actuales.
3. Backfill explícito y reversible de `initial_priority` desde `priority`; mantener `priority` durante compatibilidad.
4. Crear assessment history y evidencia con foreign keys no destructivas.
5. Down migration sólo donde sea segura; rollback operativo preferente = flag off + binario anterior, dejando columnas aditivas.

Tests:

- Migración sobre schema snapshot y re-ejecución idempotente.
- Compatibilidad de lecturas/escrituras antiguas.
- AI verdict y Analyst verdict nunca se sobrescriben.
- Constraints score 0..100, verdicts y timestamps.
- Duplicate alert concurrente.

Riesgos: locks y supuestos incorrectos de schema. Mitigación: `lock_timeout`, revisión de query plan, backup, ventana controlada y migraciones pequeñas.

## Fase 2 — Investigation engine

Objetivo: construir assessment trazable dentro de Core.

Archivos previstos:

- `contracts/soc/evidence-package.v1.schema.json`
- `contracts/soc/assessment.v1.schema.json`
- `services/core/src/soc/evidence.rs`
- `services/core/src/soc/mitre.rs`
- `services/core/src/soc/risk.rs`
- `services/core/src/soc/confidence.rs`
- `services/core/src/soc/investigation.rs`
- `services/core/src/soc/models.rs`
- extensión mínima de `security.rs`, `rag.rs`, `telemetry.rs`, `voice.rs`/LiteLLM adapter existente.
- normalizador común reutilizado por `wazuh-agent` y `wazuh-relay` o consolidación segura de uno como fuente.

Diseño:

- Evidence Package acotado con `source`, `source_id`, timestamp original, raw reference y campos normalizados; null para faltantes.
- MITRE se toma de Wazuh cuando exista. Heurística textual actual queda sólo como legacy fallback desactivable y nunca se presenta como dato Wazuh.
- Risk y Confidence son funciones deterministas/versionadas separadas; el LLM aporta componentes/verdict, no scores finales.
- L1 siempre devuelve JSON validado; L2 se activa por reglas Core configurables. Toda solicitud de más evidencia vuelve al Core.
- Persistir assessment antes de publicar eventos.

Tests:

- Fixtures A/B/C/D anonimizadas.
- Risk boundaries, factores positivos/negativos y versionado.
- Confidence con contradicciones/missing data.
- MITRE related/possible/high-confidence chain y prohibición de inventar.
- L1 malformed/timeout/fail-closed; L2 escalation/manual; context/response limits.
- Evidence IDs requeridos en conclusiones y `INCONCLUSIVE` ante insuficiencia.

Dependencias: Fase 1, contrato Wazuh enriquecido, adapters LiteLLM/Qdrant/Prometheus actuales.

Rollback: flags `SOC_INVESTIGATION_ENABLED=false` y `SOC_L2_ENABLED=false`; ingesta/case manager legacy continúa.

## Fase 3 — Case management y SLA

Objetivo: operaciones de analista y deadlines autoritativos.

Archivos previstos:

- `services/core/src/soc/cases.rs`, `state_machine.rs`, `sla.rs`, `feedback.rs`, `rbac.rs`.
- Extensiones backward-compatible en `transport.rs`, contracts API y audit.
- Worker Core idempotente para warning/breach; no timers del navegador.

Diseño:

- Mapear primero estados reales. Transiciones allow-listed y auditadas.
- Assignment/ack/investigate/verdict atómicos con optimistic concurrency/version.
- SLA configurable P1-P4, WARNING <=20%, timestamps reales, MTTA/time-to-investigation/MTTR.
- Analyst verdict requiere reason/comment según tipo; conserva AI verdict.
- RBAC server-side: roles SOC nuevos mapeados sin confiar en UI; roles legacy se mantienen durante transición explícita.

Tests: transiciones válidas/inválidas, concurrencia, SLA con reloj inyectado, recalculo de prioridad, agreement/disagreement, RBAC y CSRF/fail-closed.

Rollback: `SOC_SLA_ENABLED=false`; endpoints de mutación SOC deshabilitados sin afectar lectura/ingesta.

## Fase 4 — SOC Web

Objetivo: dashboard, My Cases y Case Detail en la app actual.

Archivos previstos:

- Nuevos componentes bajo `apps/desktop/src/ui/components/soc/`.
- Stores/types/client bajo `apps/desktop/src/core/` reutilizando `runtime/web-client.ts`.
- Cambios pequeños en `template.ts`, `view.ts`, `styles.css`, navegación y tokens actuales.

Entregables:

- Dashboard con agregados de baja densidad visual.
- Bandeja paginada/filtrable con orden autoritativo default.
- Detalle: Wazuh, assessment, evidencia referenciable, timeline, MITRE, entidades, historial, similares, recomendaciones y actividad.
- Acciones de Analyst Verdict con confirmaciones y comentarios obligatorios.

Tests: parseo DTO, filtros/orden, estados loading/error, permisos, accesibilidad básica, evidencia/missing/contradictions, visual smoke test.

Rollback: flag de navegación/vistas; UI anterior intacta.

## Fase 5 — Realtime y condición 90/90

Objetivo: eventos SOC por WebSocket existente y popup crítico.

Archivos previstos:

- `services/core/src/events.rs`, `contracts/events/realtime-envelope...`, `transport.rs`.
- `apps/desktop/src/realtime/client.ts`, state y modal SOC.
- Outbox/event-delivery persistente si el schema real no posee equivalente.

Orden obligatorio 90/90:

1. Persistir assessment.
2. Fijar final priority P1.
3. Crear/actualizar SLA.
4. Confirmar transacción/outbox.
5. Publicar `SOC_CRITICAL_ALERT` mínimo.
6. Registrar delivery; UI obtiene detalle por API.

Tests: ambos thresholds inclusive, riesgo alto/confianza baja sin popup, payload mínimo, reconnect/resync/dedup, acknowledge/open/dismiss semantics.

Rollback: `SOC_CRITICAL_POPUP_ENABLED=false`; eventos pueden permanecer compatibles e ignorados por clientes antiguos.

## Fase 6 — Voice

Objetivo: reutilizar TTS actual sólo para critical assessment 90/90.

Cambios previstos: trigger en state/event handling, formato de anuncio sanitizado, preferencias compatibles y métricas de latencia. No optimización grande de TTS.

Tests: dedup/coalescing, no hashes/JSON/IDs largos, mute/repeat, autoplay fallback, evento no 90/90 no habla.

Rollback: `SOC_CRITICAL_VOICE_ENABLED=false`; conservar endpoint actual y permitir temporalmente el trigger legacy bajo flag separado durante rollout.

## Fase 7 — Feedback y métricas

Objetivo: feedback durable y observabilidad sin cardinalidad alta.

Cambios previstos:

- Queries agregadas de dashboard y agreement.
- Métricas `jarvis_soc_*` mediante el mecanismo Prometheus acordado tras verificar ADR-011/live deployment.
- Labels permitidos: priority/verdict/status/scoring_version; prohibidos case_id/host/user/IP.

Tests: agreement exacto, counters no duplicados por retry, MTTA/MTTR, ausencia de labels sensibles.

Rollback: desactivar export de métricas; datos transaccionales permanecen.

## Fase 8 — Tier 2 preparation (simulation only)

Objetivo: propuesta de cuarentena vinculada a caso, sin ejecución.

Archivos previstos:

- `services/core/src/soc/actions.rs`, contracts proposal/approval y UI modal.
- Reutilización de `PolicyEngine`/one-use grants con binding proposal+case.
- No cambios a `DisabledExecutor`; agregar una ruta de simulación que finalice `NOT_EXECUTED`/`SIMULATED` antes del executor.

Tests: proposal con target/reason/impact/rollback/requester/case; confirmación; grant único/expirado/cross-session; RBAC; audit; assertion fuerte de cero llamadas al executor.

Rollback: `SOC_TIER2_PROPOSALS_ENABLED=false`; ninguna capacidad de contención se ejecuta.

## Dependencias y orden técnico

```text
schema audit -> F1 data model -> Wazuh contract -> F2 investigation
                                      |                 |
                                      v                 v
                                F3 cases/SLA ------> F5 realtime
                                      |                 |
                                      v                 v
                                   F4 web ----------> F6 voice
                                      |
                                      v
                                F7 feedback/metrics -> F8 proposals
```

F4 puede comenzar después de estabilizar DTOs de F3, pero no debe inventar contratos en frontend. F5 requiere transacción/outbox decidida en F1/F3. F8 es siempre último.

## Feature flags

Usar configuración Core existente por environment, validada al startup, con defaults seguros:

- `SOC_INVESTIGATION_ENABLED=false`
- `SOC_L2_ENABLED=false`
- `SOC_SLA_ENABLED=false`
- `SOC_CRITICAL_POPUP_ENABLED=false`
- `SOC_CRITICAL_VOICE_ENABLED=false`
- `SOC_TIER2_PROPOSALS_ENABLED=false`

Los flags no omiten auth/RBAC/audit; sólo deshabilitan capabilities completas.

## Estrategia de commits

Orden recomendado, cada commit con tests verdes y sin secretos:

1. `chore(discovery): record SOC v0.2 gap analysis and plan`
2. `chore(db): version current SOC schema baseline`
3. `feat(db): add SOC assessment and SLA fields`
4. `feat(wazuh): preserve structured entities and MITRE evidence`
5. `feat(soc): add evidence package and MITRE correlation`
6. `feat(soc): add deterministic risk and confidence engines`
7. `feat(soc): orchestrate L1 and conditional L2 assessments`
8. `feat(soc): add case state machine assignment and SLA`
9. `feat(api): expose authenticated SOC case operations`
10. `feat(web): add SOC dashboard and case workspace`
11. `feat(realtime): add SOC events and transactional 90/90 alert`
12. `feat(voice): announce deduplicated 90/90 assessments`
13. `feat(soc): persist analyst feedback and aggregate metrics`
14. `feat(soc): add simulated Tier 2 quarantine proposals`
15. `docs(soc): finalize architecture operations rollback and troubleshooting`

No mezclar migración, backend, frontend y despliegue en un commit único. Cada commit debe listar archivos, migraciones, tests, resultado, riesgo y compatibilidad.

## Gate de salida por fase

Para aprobar cada fase:

1. Diff y archivos modificados revisados.
2. Migraciones mostradas y ensayadas sobre snapshot/restauración.
3. Tests nuevos enumerados.
4. Baseline completo ejecutado sin deshabilitar tests.
5. Errores y deuda explícitos.
6. Security review de auth/CSRF/RBAC/audit/limits/timeouts.
7. Compatibilidad con clientes y datos anteriores demostrada.
8. Procedimiento de rollback probado o ensayado.

## Riesgos transversales

- **Schema drift CT133:** no ejecutar F1 hasta auditarlo.
- **Cambios locales superpuestos:** checkpoint antes de editar módulos tocados.
- **Tres normalizadores Wazuh:** consolidar contrato gradualmente y medir shadow output.
- **Entrega realtime no durable:** usar outbox si la alerta 90/90 debe sobrevivir reinicios.
- **LLM local lento/no disponible:** timeouts, circuit breaker, INCONCLUSIVE y reintento idempotente; ingesta no se detiene.
- **Cardinalidad Prometheus:** allow-list estricta de labels.
- **RBAC legacy amplio:** añadir permisos SOC fail-closed antes de endpoints mutables.
- **Alert fatigue:** sólo assessment 90/90, dedup y delivery tracking.
- **Tier 2 accidental:** tests de cero ejecución y executor productivo deshabilitado.

## Rollback global

- Desactivar flags SOC nuevos.
- Revertir binario/frontend al artefacto anterior.
- No eliminar columnas/tablas aditivas durante incidente; detener writers nuevos y conservar evidencia.
- Reprocesar outbox sólo después de reconciliar versión de eventos.
- Mantener poller/case ingestion legacy mientras su contrato siga compatible.
- Ningún rollback requiere cambios de firewall, exposición, OpenBao o RestrictedExecutor.

## Documentación final prevista

Después de implementar, actualizar `docs/architecture.md`, `docs/api.md`, `docs/realtime.md`, `docs/voice.md`, `docs/security.md`, `docs/authentication.md`, integrations Wazuh/n8n/LiteLLM/Qdrant/Prometheus y crear documentos de data model, scoring, L1/L2, MITRE, SLA, UI, RBAC, Tier 2 simulation, tests, rollback/troubleshooting y ADRs para scoring/versionado, outbox 90/90 y state machine.
