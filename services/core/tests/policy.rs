use jarvis_core::{ActionRequest, AuthorizationError, Decision, PolicyEngine, Principal, Risk};
use serde_json::Map;
use std::time::{Duration, Instant};

fn action(capability: &str, target: &str) -> ActionRequest {
    ActionRequest {
        capability: capability.into(),
        target: target.into(),
        parameters: Map::new(),
    }
}

fn operator() -> Principal {
    Principal {
        subject: "operator:test".into(),
        roles: vec!["operator".into()],
    }
}

#[test]
fn tier_1_is_allowed_immediately() {
    assert_eq!(
        PolicyEngine::default().evaluate(&operator(), &action("core.health.read", "jarvis-core")),
        Decision::Permit {
            risk: Risk::ReadOnly
        }
    );
}

#[test]
fn tier_2_requires_single_use_authorization() {
    let policy = PolicyEngine::default();
    let action = action("security.ip.block", "203.0.113.10");
    assert_eq!(
        policy.evaluate(&operator(), &action),
        Decision::RequireAuthorization {
            risk: Risk::Modification
        }
    );
    policy
        .authorize(&operator(), &action, "session-a", None, None)
        .expect("tier 2 grant");
    assert_eq!(
        policy.evaluate_with_grant(&operator(), &action, "session-a"),
        Decision::Permit {
            risk: Risk::Modification
        }
    );
}

#[test]
fn tier_3_without_rollback_plan_is_rejected() {
    let policy = PolicyEngine::default();
    let action = action("proxmox.vm.destroy", "vm-104");
    assert_eq!(
        policy.authorize(&operator(), &action, "session-a", Some("vm-104"), None,),
        Err(AuthorizationError::RollbackPlanRequired)
    );
    assert_eq!(
        policy.evaluate_with_grant(&operator(), &action, "session-a"),
        Decision::RequireAuthorization {
            risk: Risk::Critical
        }
    );
}

#[test]
fn tier_3_wrong_resource_identifier_is_rejected() {
    let policy = PolicyEngine::default();
    let action = action("proxmox.vm.destroy", "vm-104");
    assert_eq!(
        policy.authorize(
            &operator(),
            &action,
            "session-a",
            Some("vm-105"),
            Some("restore vm-104 from backup"),
        ),
        Err(AuthorizationError::ResourceIdentifierMismatch)
    );
}

#[test]
fn tier_3_grant_expired_at_121_seconds_is_rejected() {
    let policy = PolicyEngine::default();
    let action = action("proxmox.vm.destroy", "vm-104");
    let issued = Instant::now();
    policy
        .authorize_at(
            &operator(),
            &action,
            "session-a",
            Some("vm-104"),
            Some("restore vm-104 from backup"),
            issued,
        )
        .expect("tier 3 grant");
    assert_eq!(
        policy.evaluate_at(
            &operator(),
            &action,
            Some("session-a"),
            issued + Duration::from_secs(121),
        ),
        Decision::RequireAuthorization {
            risk: Risk::Critical
        }
    );
}

#[test]
fn tier_3_grant_reuse_is_rejected() {
    let policy = PolicyEngine::default();
    let action = action("proxmox.vm.destroy", "vm-104");
    policy
        .authorize(
            &operator(),
            &action,
            "session-a",
            Some("vm-104"),
            Some("restore vm-104 from backup"),
        )
        .expect("tier 3 grant");
    assert_eq!(
        policy.evaluate_with_grant(&operator(), &action, "session-a"),
        Decision::Permit {
            risk: Risk::Critical
        }
    );
    assert_eq!(
        policy.evaluate_with_grant(&operator(), &action, "session-a"),
        Decision::RequireAuthorization {
            risk: Risk::Critical
        }
    );
}

#[test]
fn grants_are_session_scoped() {
    let policy = PolicyEngine::default();
    let action = action("security.user.disable", "user-alice");
    policy
        .authorize(&operator(), &action, "session-a", None, None)
        .expect("tier 2 grant");
    assert_eq!(
        policy.evaluate_with_grant(&operator(), &action, "session-b"),
        Decision::RequireAuthorization {
            risk: Risk::Modification
        }
    );
}

#[test]
fn domain_agent_cannot_issue_its_own_grant() {
    let policy = PolicyEngine::default();
    let principal = Principal {
        subject: "wazuh-agent:prod".into(),
        roles: vec!["wazuh-agent".into()],
    };
    let action = action("security.host.isolate", "host-01");

    assert_eq!(
        policy.authorize(&principal, &action, "session-a", None, None),
        Err(AuthorizationError::RoleNotAuthorized)
    );
}

#[test]
fn unknown_capability_is_denied() {
    assert_eq!(
        PolicyEngine::default().evaluate(&operator(), &action("shell.execute", "workstation")),
        Decision::Deny {
            reason: "CAPABILITY_DENIED"
        }
    );
}

#[test]
fn role_mismatch_is_denied() {
    let principal = Principal {
        subject: "unknown:test".into(),
        roles: vec!["unknown".into()],
    };
    assert_eq!(
        PolicyEngine::default().evaluate(&principal, &action("security.host.isolate", "host-01")),
        Decision::Deny {
            reason: "ROLE_NOT_AUTHORIZED"
        }
    );
}
