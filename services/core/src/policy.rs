use crate::{auth::OneTimeGrantStore, ActionRequest, Principal};
use serde::Deserialize;
use std::time::{Duration, Instant};

const CAPABILITIES_JSON: &str = include_str!("../../../contracts/data/capabilities.json");
const MAX_ACTIVE_GRANTS: usize = 512;
const POLICY_ROLES: &[&str] = &["desktop", "operator", "wazuh-agent", "proxmox-agent"];
const HUMAN_AUTHORIZATION_ROLES: &[&str] = &["desktop", "operator"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    ReadOnly,
    Modification,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub capability: &'static str,
    pub target: &'static str,
    pub allowed_roles: &'static [&'static str],
    pub risk: Risk,
    pub requires_authorization: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Permit { risk: Risk },
    RequireAuthorization { risk: Risk },
    Deny { reason: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationError {
    CapabilityDenied,
    RoleNotAuthorized,
    AuthorizationNotRequired,
    ResourceIdentifierMismatch,
    RollbackPlanRequired,
    GrantCapacityReached,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityEntry {
    capability: String,
    tier: u8,
    #[allow(dead_code)]
    owner_agent: String,
    #[allow(dead_code)]
    required_evidence: Vec<String>,
    authorization: AuthorizationConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum AuthorizationConfig {
    None,
    SingleGrant {
        expiry_seconds: u64,
    },
    TypedConfirmation {
        expiry_seconds: u64,
        required_field: String,
    },
}

#[derive(Debug, Clone)]
struct CapabilityRule {
    capability: String,
    risk: Risk,
    authorization: AuthorizationConfig,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct GrantKey {
    session_id: String,
    subject: String,
    capability: String,
    target: String,
}

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    rules: Vec<CapabilityRule>,
    grants: OneTimeGrantStore<GrantKey>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        let entries: Vec<CapabilityEntry> = serde_json::from_str(CAPABILITIES_JSON)
            .expect("embedded capabilities.json must match the capability contract");
        let rules = entries
            .into_iter()
            .map(|entry| CapabilityRule {
                capability: entry.capability,
                risk: risk_for_tier(entry.tier),
                authorization: entry.authorization,
            })
            .collect();
        Self {
            rules,
            grants: OneTimeGrantStore::new(MAX_ACTIVE_GRANTS),
        }
    }
}

impl PolicyEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        let rules = rules
            .into_iter()
            .map(|rule| CapabilityRule {
                capability: rule.capability.into(),
                risk: rule.risk,
                authorization: if rule.requires_authorization || rule.risk != Risk::ReadOnly {
                    AuthorizationConfig::SingleGrant {
                        expiry_seconds: 300,
                    }
                } else {
                    AuthorizationConfig::None
                },
            })
            .collect();
        Self {
            rules,
            grants: OneTimeGrantStore::new(MAX_ACTIVE_GRANTS),
        }
    }

    pub fn evaluate(&self, principal: &Principal, action: &ActionRequest) -> Decision {
        self.evaluate_at(principal, action, None, Instant::now())
    }

    pub fn evaluate_with_grant(
        &self,
        principal: &Principal,
        action: &ActionRequest,
        session_id: &str,
    ) -> Decision {
        self.evaluate_at(principal, action, Some(session_id), Instant::now())
    }

    pub fn authorize(
        &self,
        principal: &Principal,
        action: &ActionRequest,
        session_id: &str,
        resource_identifier: Option<&str>,
        rollback_plan: Option<&str>,
    ) -> Result<(), AuthorizationError> {
        self.authorize_at(
            principal,
            action,
            session_id,
            resource_identifier,
            rollback_plan,
            Instant::now(),
        )
    }

    #[doc(hidden)]
    pub fn authorize_at(
        &self,
        principal: &Principal,
        action: &ActionRequest,
        session_id: &str,
        resource_identifier: Option<&str>,
        rollback_plan: Option<&str>,
        now: Instant,
    ) -> Result<(), AuthorizationError> {
        let rule = self.authorized_rule(principal, action)?;
        if !principal
            .roles
            .iter()
            .any(|role| HUMAN_AUTHORIZATION_ROLES.contains(&role.as_str()))
        {
            return Err(AuthorizationError::RoleNotAuthorized);
        }
        match &rule.authorization {
            AuthorizationConfig::None => return Err(AuthorizationError::AuthorizationNotRequired),
            AuthorizationConfig::SingleGrant { .. } => {}
            AuthorizationConfig::TypedConfirmation { required_field, .. } => {
                if resource_identifier != Some(action.target.as_str()) {
                    return Err(AuthorizationError::ResourceIdentifierMismatch);
                }
                if required_field != "rollback_plan"
                    || rollback_plan.is_none_or(|plan| plan.trim().is_empty())
                {
                    return Err(AuthorizationError::RollbackPlanRequired);
                }
            }
        }
        if self
            .grants
            .issue_at(grant_key(principal, action, session_id), now)
        {
            Ok(())
        } else {
            Err(AuthorizationError::GrantCapacityReached)
        }
    }

    #[doc(hidden)]
    pub fn evaluate_at(
        &self,
        principal: &Principal,
        action: &ActionRequest,
        session_id: Option<&str>,
        now: Instant,
    ) -> Decision {
        let rule = match self.authorized_rule(principal, action) {
            Ok(rule) => rule,
            Err(AuthorizationError::CapabilityDenied) => {
                return Decision::Deny {
                    reason: "CAPABILITY_DENIED",
                }
            }
            Err(_) => {
                return Decision::Deny {
                    reason: "ROLE_NOT_AUTHORIZED",
                }
            }
        };
        let ttl = match rule.authorization {
            AuthorizationConfig::None => return Decision::Permit { risk: rule.risk },
            AuthorizationConfig::SingleGrant { expiry_seconds }
            | AuthorizationConfig::TypedConfirmation { expiry_seconds, .. } => {
                Duration::from_secs(expiry_seconds)
            }
        };
        if session_id.is_some_and(|session| {
            self.grants
                .take_at(&grant_key(principal, action, session), ttl, now)
        }) {
            Decision::Permit { risk: rule.risk }
        } else {
            Decision::RequireAuthorization { risk: rule.risk }
        }
    }

    fn authorized_rule<'a>(
        &'a self,
        principal: &Principal,
        action: &ActionRequest,
    ) -> Result<&'a CapabilityRule, AuthorizationError> {
        let rule = self
            .rules
            .iter()
            .find(|rule| rule.capability == action.capability)
            .ok_or(AuthorizationError::CapabilityDenied)?;
        if !principal
            .roles
            .iter()
            .any(|role| POLICY_ROLES.contains(&role.as_str()))
        {
            return Err(AuthorizationError::RoleNotAuthorized);
        }
        Ok(rule)
    }
}

fn risk_for_tier(tier: u8) -> Risk {
    match tier {
        1 => Risk::ReadOnly,
        2 => Risk::Modification,
        3 => Risk::Critical,
        _ => panic!("embedded capability has unsupported tier {tier}"),
    }
}

fn grant_key(principal: &Principal, action: &ActionRequest, session_id: &str) -> GrantKey {
    GrantKey {
        session_id: session_id.into(),
        subject: principal.subject.clone(),
        capability: action.capability.clone(),
        target: action.target.clone(),
    }
}
