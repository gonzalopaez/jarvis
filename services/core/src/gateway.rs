use crate::{
    validate_request, ApiError, AuditEvent, AuditSink, AuthContext, AuthorizationError,
    CoreRequest, CoreResponse, Decision, PolicyEngine, ResponseStatus, RestrictedExecutor,
    API_VERSION,
};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

static AUDIT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub struct CoreGateway<E, A> {
    policy: PolicyEngine,
    executor: E,
    audit: A,
}

impl<E, A> CoreGateway<E, A>
where
    E: RestrictedExecutor,
    A: AuditSink,
{
    pub fn new(policy: PolicyEngine, executor: E, audit: A) -> Self {
        Self {
            policy,
            executor,
            audit,
        }
    }

    pub fn handle(&self, auth: &AuthContext, request: CoreRequest) -> CoreResponse {
        let audit_id = next_audit_id();
        if validate_request(&request).is_err() {
            self.audit(&audit_id, &request, "anonymous", "rejected");
            return response(
                request,
                audit_id,
                ResponseStatus::Rejected,
                None,
                Some(ApiError {
                    code: "INVALID_REQUEST",
                    message: "Request validation failed",
                }),
            );
        }

        let Some(principal) = auth.principal.as_ref().filter(|_| auth.authenticated) else {
            self.audit(&audit_id, &request, "anonymous", "denied");
            return response(
                request,
                audit_id,
                ResponseStatus::Denied,
                None,
                Some(ApiError {
                    code: "AUTHENTICATION_REQUIRED",
                    message: "Authentication is required",
                }),
            );
        };

        if request.kind == "conversation" {
            self.audit(&audit_id, &request, &principal.subject, "mock_completed");
            return response(
                request,
                audit_id,
                ResponseStatus::Completed,
                Some(json!({
                    "mode": "mock",
                    "message": "Core contract accepted; no model or external service was contacted"
                })),
                None,
            );
        }

        let action = request
            .action
            .as_ref()
            .expect("validated action request must contain action");
        let decision = if let Some(authorization) = &request.authorization {
            if let Err(error) = self.policy.authorize(
                principal,
                action,
                &request.session_id,
                Some(&authorization.confirmation),
                authorization.rollback_plan.as_deref(),
            ) {
                self.audit(
                    &audit_id,
                    &request,
                    &principal.subject,
                    "authorization_denied",
                );
                return response(
                    request,
                    audit_id,
                    ResponseStatus::Denied,
                    None,
                    Some(authorization_error(error)),
                );
            }
            self.policy
                .evaluate_with_grant(principal, action, &request.session_id)
        } else {
            self.policy.evaluate(principal, action)
        };
        match decision {
            Decision::Deny { reason } => {
                self.audit(&audit_id, &request, &principal.subject, "denied");
                response(
                    request,
                    audit_id,
                    ResponseStatus::Denied,
                    None,
                    Some(ApiError {
                        code: reason,
                        message: "Action is not permitted",
                    }),
                )
            }
            Decision::RequireAuthorization { .. } => {
                self.audit(
                    &audit_id,
                    &request,
                    &principal.subject,
                    "authorization_required",
                );
                response(
                    request,
                    audit_id,
                    ResponseStatus::AuthorizationRequired,
                    None,
                    Some(ApiError {
                        code: "AUTHORIZATION_REQUIRED",
                        message: "Explicit authorization is required",
                    }),
                )
            }
            Decision::Permit { .. } => match self.executor.execute(principal, action) {
                Ok(result) if result.verified => {
                    self.audit(&audit_id, &request, &principal.subject, "verified");
                    response(
                        request,
                        audit_id,
                        ResponseStatus::Completed,
                        Some(result.data),
                        None,
                    )
                }
                Ok(_) | Err(_) => {
                    self.audit(
                        &audit_id,
                        &request,
                        &principal.subject,
                        "verification_failed",
                    );
                    response(
                        request,
                        audit_id,
                        ResponseStatus::Denied,
                        None,
                        Some(ApiError {
                            code: "EXECUTION_NOT_VERIFIED",
                            message: "Action result could not be verified",
                        }),
                    )
                }
            },
        }
    }

    fn audit(&self, audit_id: &str, request: &CoreRequest, subject: &str, outcome: &'static str) {
        self.audit.record(AuditEvent {
            audit_id: audit_id.to_string(),
            request_id: request.request_id.clone(),
            subject: subject.to_string(),
            capability: request
                .action
                .as_ref()
                .map(|action| action.capability.clone()),
            target: request.action.as_ref().map(|action| action.target.clone()),
            outcome,
        });
    }
}

fn authorization_error(error: AuthorizationError) -> ApiError {
    match error {
        AuthorizationError::CapabilityDenied => ApiError {
            code: "CAPABILITY_DENIED",
            message: "Action is not permitted",
        },
        AuthorizationError::RoleNotAuthorized => ApiError {
            code: "ROLE_NOT_AUTHORIZED",
            message: "Only an authorized human may grant this action",
        },
        AuthorizationError::AuthorizationNotRequired => ApiError {
            code: "AUTHORIZATION_NOT_REQUIRED",
            message: "This action does not accept an authorization grant",
        },
        AuthorizationError::ConfirmationRequired => ApiError {
            code: "CONFIRMATION_REQUIRED",
            message: "The exact confirmation value is required",
        },
        AuthorizationError::ResourceIdentifierMismatch => ApiError {
            code: "RESOURCE_IDENTIFIER_MISMATCH",
            message: "Confirmation does not match the exact resource identifier",
        },
        AuthorizationError::RollbackPlanRequired => ApiError {
            code: "ROLLBACK_PLAN_REQUIRED",
            message: "A non-empty rollback plan is required",
        },
        AuthorizationError::GrantCapacityReached => ApiError {
            code: "AUTHORIZATION_UNAVAILABLE",
            message: "Authorization capacity is unavailable",
        },
    }
}

fn response(
    request: CoreRequest,
    audit_id: String,
    status: ResponseStatus,
    data: Option<serde_json::Value>,
    error: Option<ApiError>,
) -> CoreResponse {
    CoreResponse {
        api_version: API_VERSION,
        request_id: request.request_id,
        session_id: request.session_id,
        status,
        audit_id,
        data,
        error,
    }
}

fn next_audit_id() -> String {
    format!(
        "audit-{:016x}",
        AUDIT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
