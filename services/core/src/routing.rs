use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiMode {
    #[default]
    Auto,
    Fast,
    Smart,
    Expert,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CapabilityRoute {
    FastModel,
    ReasoningModel,
    Codex,
    InfrastructureAgent,
    SecurityAgent,
    Automation,
    McpTool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRequest {
    pub message: String,
    pub session_id: String,
    pub source: RequestSource,
    #[serde(default)]
    pub mode: AiMode,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequestSource {
    Voice,
    Text,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RoutingDecision {
    pub route: CapabilityRoute,
    pub intent: &'static str,
    pub complexity: Complexity,
    pub agent: Option<&'static str>,
    pub model_alias: Option<&'static str>,
    pub requires_tools: bool,
    pub requires_authorization: bool,
    pub reason: &'static str,
}

pub trait CapabilityRouter: Send + Sync {
    fn decide(&self, request: &CapabilityRequest) -> RoutingDecision;
}

/// Conservative first-stage router. It is intentionally deterministic and
/// side-effect free so it can later be complemented by a classifier without
/// changing the public contract.
#[derive(Debug, Default, Clone, Copy)]
pub struct DeterministicCapabilityRouter;

impl CapabilityRouter for DeterministicCapabilityRouter {
    fn decide(&self, request: &CapabilityRequest) -> RoutingDecision {
        match request.mode {
            AiMode::Fast => {
                return model(
                    CapabilityRoute::FastModel,
                    "conversation",
                    Complexity::Low,
                    "jarvis-fast",
                    "FAST mode selected",
                )
            }
            AiMode::Smart => {
                return model(
                    CapabilityRoute::ReasoningModel,
                    "general_reasoning",
                    Complexity::High,
                    "jarvis-reasoning",
                    "SMART mode selected",
                )
            }
            AiMode::Expert => {
                return agent(
                    CapabilityRoute::Codex,
                    "technical_expert",
                    Complexity::High,
                    "codex",
                    false,
                    "EXPERT mode selected",
                )
            }
            AiMode::Auto => {}
        }

        let normalized = normalize(&request.message);
        let tokens: Vec<&str> = normalized.split_whitespace().collect();

        // Infrastructure telemetry must never fall through to a generic model:
        // it requires verified Prometheus/MCP evidence.
        if contains_any(&normalized, &["prometheus", "telemetria", "server central"]) {
            return agent(
                CapabilityRoute::InfrastructureAgent,
                "infrastructure_diagnostic",
                Complexity::Medium,
                "infrastructure",
                true,
                "explicit infrastructure telemetry request",
            );
        }

        let likely_security_alert_query = contains_any(&normalized, &["alerta", "alertas"])
            && contains_any(
                &normalized,
                &[
                    "critica",
                    "criticas",
                    "seguridad",
                    "ultimos",
                    "ultimo",
                    "ultima",
                    "minuto",
                    "hora",
                ],
            );
        // "caido"/"caida"/"dominio" are too generic to route on their own
        // (e.g. "se me cayó el café", "el dominio de la función"). Only treat
        // them as security-relevant when a service/host context is present.
        let service_context = contains_any(
            &normalized,
            &[
                "servicio",
                "servidor",
                "server",
                "host",
                "maquina",
                "nodo",
                "instancia",
                "vpn",
                "tunnel",
                "cloudflare",
                "tailscale",
                "adguard",
                "freeipa",
                "proxmox",
                "dominio",
            ],
        );
        let service_down_query = service_context
            && contains_any(
                &normalized,
                &["caido", "caida", "no responde", "inaccesible", "fuera de linea"],
            );
        let domain_controller_query = normalized.contains("controlador de dominio")
            || normalized.contains("servidor de ce")
            || contains_word(&tokens, "dc");
        if contains_any(
            &normalized,
            &[
                "cada manana",
                "todos los dias",
                "programa una tarea",
                "automatiza",
                "workflow",
                "n8n",
            ],
        ) {
            return agent(
                CapabilityRoute::Automation,
                "automation",
                Complexity::Medium,
                "n8n",
                true,
                "scheduled or workflow intent",
            );
        }
        if likely_security_alert_query
            || service_down_query
            || domain_controller_query
            || contains_any(
                &normalized,
                &[
                    "alerta de seguridad",
                    "alertas de seguridad",
                    "alertas criticas",
                    "amenazas de seguridad",
                    "wazuh",
                    "vulnerabilidad",
                    "inicio de sesion fallido",
                    "failed login",
                    "incidente de seguridad",
                    "ioc ",
                ],
            )
        {
            return agent(
                CapabilityRoute::SecurityAgent,
                "security_analysis",
                Complexity::High,
                "security",
                true,
                "security-domain request",
            );
        }
        if contains_any(
            &normalized,
            &[
                "proxmox",
                "prometheus",
                "telemetria",
                "telemetría",
                "maquina virtual",
                "contenedor lxc",
                "servidor central",
                "estado del servicio",
                "estado de bluetooth",
            ],
        ) {
            return agent(
                CapabilityRoute::InfrastructureAgent,
                "infrastructure_diagnostic",
                Complexity::Medium,
                "infrastructure",
                true,
                "infrastructure state or diagnostic request",
            );
        }
        if contains_any(
            &normalized,
            &[
                "codigo ",
                "code ",
                "rust",
                "python",
                "typescript",
                "javascript",
                "stack trace",
                "debug",
                "compila",
                "compiler",
                "refactor",
                "bug ",
                "logs",
                "configuracion",
                "dockerfile",
                "kubernetes",
                "systemd",
            ],
        ) || looks_like_code(&request.message)
        {
            return agent(
                CapabilityRoute::Codex,
                "technical_diagnostic",
                Complexity::High,
                "codex",
                contains_any(&normalized, &["revisa", "logs", "estado", "repositorio"]),
                "technical task benefits from expert agent",
            );
        }
        if tokens.len() > 45
            || contains_any(
                &normalized,
                &[
                    "analiza en profundidad",
                    "compara las alternativas",
                    "plan detallado",
                    "razona paso a paso",
                ],
            )
        {
            return model(
                CapabilityRoute::ReasoningModel,
                "general_reasoning",
                Complexity::High,
                "jarvis-reasoning",
                "higher reasoning complexity",
            );
        }
        model(
            CapabilityRoute::FastModel,
            "conversation",
            Complexity::Low,
            "jarvis-fast",
            "low-complexity conversation",
        )
    }
}

fn model(
    route: CapabilityRoute,
    intent: &'static str,
    complexity: Complexity,
    alias: &'static str,
    reason: &'static str,
) -> RoutingDecision {
    RoutingDecision {
        route,
        intent,
        complexity,
        agent: None,
        model_alias: Some(alias),
        requires_tools: false,
        requires_authorization: false,
        reason,
    }
}

fn agent(
    route: CapabilityRoute,
    intent: &'static str,
    complexity: Complexity,
    name: &'static str,
    requires_tools: bool,
    reason: &'static str,
) -> RoutingDecision {
    RoutingDecision {
        route,
        intent,
        complexity,
        agent: Some(name),
        model_alias: None,
        requires_tools,
        requires_authorization: false,
        reason,
    }
}

fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

fn contains_any(value: &str, signals: &[&str]) -> bool {
    signals.iter().any(|signal| value.contains(signal))
}

/// Whole-token match, so short signals like "dc" do not match by substring
/// inside unrelated words. Tokens are normalized text split on whitespace;
/// surrounding punctuation is trimmed before comparison.
fn contains_word(tokens: &[&str], word: &str) -> bool {
    tokens.iter().any(|token| {
        token.trim_matches(|character: char| !character.is_alphanumeric()) == word
    })
}

fn looks_like_code(value: &str) -> bool {
    value.contains("```")
        || value.contains("::")
        || value.contains("Traceback (")
        || (value.contains('{') && value.contains('}') && value.contains(';'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(message: &str) -> RoutingDecision {
        DeterministicCapabilityRouter.decide(&CapabilityRequest {
            message: message.into(),
            session_id: "session-1".into(),
            source: RequestSource::Text,
            mode: AiMode::Auto,
        })
    }

    #[test]
    fn simple_conversation_uses_fast_model() {
        assert_eq!(route("Hola Jarvis").route, CapabilityRoute::FastModel);
        assert_eq!(
            route("Explicame que es una VPN").route,
            CapabilityRoute::FastModel
        );
    }

    #[test]
    fn complex_reasoning_uses_reasoning_model() {
        assert_eq!(
            route("Analiza en profundidad y compara las alternativas para organizar este proyecto")
                .route,
            CapabilityRoute::ReasoningModel
        );
    }

    #[test]
    fn code_and_technical_diagnostics_use_codex() {
        assert_eq!(
            route("Analiza este codigo Rust y encontra el error").route,
            CapabilityRoute::Codex
        );
        assert_eq!(
            route("Depura este stack trace de Python").route,
            CapabilityRoute::Codex
        );
    }

    #[test]
    fn domain_requests_are_separated() {
        assert_eq!(
            route("Cada manana ejecuta este workflow").route,
            CapabilityRoute::Automation
        );
        assert_eq!(
            route("Mostrame las alertas de seguridad de Wazuh").route,
            CapabilityRoute::SecurityAgent
        );
        assert_eq!(
            route("Hay una alerta critica de Guaso en los ultimos 30 minutos").route,
            CapabilityRoute::SecurityAgent
        );
        assert_eq!(
            route("Revisa el estado de Bluetooth").route,
            CapabilityRoute::InfrastructureAgent
        );
    }

    #[test]
    fn spoken_down_server_query_uses_security_agent() {
        let decision = route("¿Está caído el servidor de ce?");
        assert_eq!(decision.route, CapabilityRoute::SecurityAgent);
    }

    #[test]
    fn generic_down_and_domain_words_do_not_route_to_security() {
        // Without a service/host context these are ordinary conversation, not
        // security queries.
        assert_ne!(
            route("Se me ha caído el café encima del teclado").route,
            CapabilityRoute::SecurityAgent
        );
        assert_ne!(
            route("Cuál es el dominio de la función logaritmo").route,
            CapabilityRoute::SecurityAgent
        );
        // "dc" as a substring inside another word must not match.
        assert_ne!(
            route("Explicame el modelo de Bohr adecuado").route,
            CapabilityRoute::SecurityAgent
        );
    }

    #[test]
    fn service_down_query_with_context_uses_security_agent() {
        assert_eq!(
            route("El servicio de VPN está caído").route,
            CapabilityRoute::SecurityAgent
        );
        assert_eq!(
            route("El controlador de dominio no responde").route,
            CapabilityRoute::SecurityAgent
        );
    }

    #[test]
    fn explicit_mode_wins_without_changing_alias_configuration() {
        let router = DeterministicCapabilityRouter;
        let mut request = CapabilityRequest {
            message: "hola".into(),
            session_id: "s".into(),
            source: RequestSource::Text,
            mode: AiMode::Expert,
        };
        assert_eq!(router.decide(&request).route, CapabilityRoute::Codex);
        request.mode = AiMode::Smart;
        assert_eq!(
            router.decide(&request).model_alias,
            Some("jarvis-reasoning")
        );
    }
}
