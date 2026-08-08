use crate::{ActionRequest, Principal};

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

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    rules: Vec<Rule>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self {
            rules: vec![
                Rule {
                    capability: "core.health.read",
                    target: "jarvis-core",
                    allowed_roles: &["desktop"],
                    risk: Risk::ReadOnly,
                    requires_authorization: false,
                },
                Rule {
                    capability: "demo.protected_action",
                    target: "demo",
                    allowed_roles: &["operator"],
                    risk: Risk::Modification,
                    requires_authorization: true,
                },
            ],
        }
    }
}

impl PolicyEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    pub fn evaluate(&self, principal: &Principal, action: &ActionRequest) -> Decision {
        let Some(rule) = self
            .rules
            .iter()
            .find(|rule| rule.capability == action.capability && rule.target == action.target)
        else {
            return Decision::Deny {
                reason: "CAPABILITY_DENIED",
            };
        };

        if !principal
            .roles
            .iter()
            .any(|role| rule.allowed_roles.contains(&role.as_str()))
        {
            return Decision::Deny {
                reason: "ROLE_NOT_AUTHORIZED",
            };
        }

        if rule.requires_authorization || rule.risk != Risk::ReadOnly {
            Decision::RequireAuthorization { risk: rule.risk }
        } else {
            Decision::Permit { risk: rule.risk }
        }
    }
}
