use jarvis_core::{
    Authenticator, BearerAuthenticator, CredentialConfigError, CredentialRecord, MIN_BEARER_BYTES,
};
use sha2::{Digest, Sha256};

fn record(
    fixture_value: &str,
    subject: &str,
    roles: &[&str],
) -> Result<CredentialRecord, CredentialConfigError> {
    let digest = Sha256::digest(fixture_value.as_bytes());
    CredentialRecord::from_sha256_hex(
        &hex::encode(digest),
        subject,
        roles.iter().map(|role| (*role).to_string()).collect(),
    )
}

fn header(fixture_value: &str) -> String {
    format!("{} {}", "Bearer", fixture_value)
}

#[test]
fn valid_bearer_maps_to_server_owned_identity() {
    let fixture_value = "desktop-fixture-with-at-least-32-bytes";
    let authenticator =
        BearerAuthenticator::new(vec![
            record(fixture_value, "desktop:test", &["desktop"]).expect("record")
        ])
        .expect("authenticator");

    let auth = authenticator
        .authenticate(Some(&header(fixture_value)))
        .expect("authenticated");
    let principal = auth.principal.expect("principal");

    assert!(auth.authenticated);
    assert_eq!(principal.subject, "desktop:test");
    assert_eq!(principal.roles, vec!["desktop"]);
}

#[test]
fn invalid_or_malformed_bearers_are_rejected() {
    let fixture_value = "desktop-fixture-with-at-least-32-bytes";
    let authenticator =
        BearerAuthenticator::new(vec![
            record(fixture_value, "desktop:test", &["desktop"]).expect("record")
        ])
        .expect("authenticator");

    assert!(authenticator
        .authenticate(Some(&header("different-fixture-with-at-least-32-bytes")))
        .is_err());
    assert!(authenticator.authenticate(Some("Basic ignored")).is_err());
    assert!(authenticator.authenticate(None).is_err());
    assert!(authenticator
        .authenticate(Some(&header(&"x".repeat(MIN_BEARER_BYTES - 1))))
        .is_err());
}

#[test]
fn duplicate_digests_are_configuration_errors() {
    let fixture_value = "shared-fixture-with-at-least-32-bytes";
    let records = vec![
        record(fixture_value, "desktop:one", &["desktop"]).expect("record"),
        record(fixture_value, "desktop:two", &["desktop"]).expect("record"),
    ];

    assert!(matches!(
        BearerAuthenticator::new(records),
        Err(CredentialConfigError::DuplicateDigest)
    ));
}

#[test]
fn malformed_identity_configuration_fails_closed() {
    assert!(matches!(
        CredentialRecord::from_sha256_hex("not-hex", "desktop:test", vec!["desktop".into()]),
        Err(CredentialConfigError::InvalidDigest)
    ));
    assert!(matches!(
        record(
            "desktop-fixture-with-at-least-32-bytes",
            "invalid subject",
            &["desktop"]
        ),
        Err(CredentialConfigError::InvalidSubject)
    ));
    assert!(matches!(
        record(
            "desktop-fixture-with-at-least-32-bytes",
            "desktop:test",
            &[]
        ),
        Err(CredentialConfigError::InvalidRole)
    ));
}
