use jarvis_core::{ActionRequest, Decision, PolicyEngine, Principal, Risk, Rule};
use serde_json::Map;

fn action(capability: &str, target: &str) -> ActionRequest {
    ActionRequest {
        capability: capability.into(),
        target: target.into(),
        parameters: Map::new(),
    }
}

#[test]
fn role_mismatch_is_denied() {
    let principal = Principal {
        subject: "desktop:test".into(),
        roles: vec!["desktop".into()],
    };

    let decision =
        PolicyEngine::default().evaluate(&principal, &action("demo.protected_action", "demo"));

    assert_eq!(
        decision,
        Decision::Deny {
            reason: "ROLE_NOT_AUTHORIZED"
        }
    );
}

#[test]
fn critical_rules_can_never_bypass_configured_authorization() {
    let policy = PolicyEngine::new(vec![Rule {
        capability: "infrastructure.restart",
        target: "test-target",
        allowed_roles: &["operator"],
        risk: Risk::Critical,
        requires_authorization: false,
    }]);
    let principal = Principal {
        subject: "operator:test".into(),
        roles: vec!["operator".into()],
    };

    let decision = policy.evaluate(&principal, &action("infrastructure.restart", "test-target"));

    assert_eq!(
        decision,
        Decision::RequireAuthorization {
            risk: Risk::Critical
        }
    );
}
