use crate::{AuthContext, AuthError, Authenticator};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use subtle::ConstantTimeEq;

pub const MIN_BEARER_BYTES: usize = 32;
pub const MAX_BEARER_BYTES: usize = 4096;

pub struct CredentialRecord {
    digest: [u8; 32],
    subject: String,
    roles: Vec<String>,
}

impl CredentialRecord {
    pub fn from_sha256_hex(
        digest_hex: &str,
        subject: impl Into<String>,
        roles: Vec<String>,
    ) -> Result<Self, CredentialConfigError> {
        let digest_bytes =
            hex::decode(digest_hex).map_err(|_| CredentialConfigError::InvalidDigest)?;
        let digest = digest_bytes
            .try_into()
            .map_err(|_| CredentialConfigError::InvalidDigest)?;
        let subject = subject.into();
        if !valid_identity_field(&subject) {
            return Err(CredentialConfigError::InvalidSubject);
        }
        if roles.is_empty() || roles.iter().any(|role| !valid_identity_field(role)) {
            return Err(CredentialConfigError::InvalidRole);
        }
        let mut unique_roles = HashSet::new();
        if !roles.iter().all(|role| unique_roles.insert(role.as_str())) {
            return Err(CredentialConfigError::DuplicateRole);
        }
        Ok(Self {
            digest,
            subject,
            roles,
        })
    }
}

pub struct BearerAuthenticator {
    records: Vec<CredentialRecord>,
}

impl BearerAuthenticator {
    pub fn new(records: Vec<CredentialRecord>) -> Result<Self, CredentialConfigError> {
        if records.is_empty() {
            return Err(CredentialConfigError::NoCredentials);
        }
        for (index, record) in records.iter().enumerate() {
            if records[index + 1..]
                .iter()
                .any(|candidate| bool::from(candidate.digest.ct_eq(&record.digest)))
            {
                return Err(CredentialConfigError::DuplicateDigest);
            }
        }
        Ok(Self { records })
    }
}

impl Authenticator for BearerAuthenticator {
    fn authenticate(&self, credential: Option<&str>) -> Result<AuthContext, AuthError> {
        let bearer = parse_bearer(credential.ok_or(AuthError)?).ok_or(AuthError)?;
        let candidate: [u8; 32] = Sha256::digest(bearer.as_bytes()).into();
        let mut matched = None;
        for (index, record) in self.records.iter().enumerate() {
            if bool::from(candidate.ct_eq(&record.digest)) {
                matched = Some(index);
            }
        }
        let record = matched.map(|index| &self.records[index]).ok_or(AuthError)?;
        Ok(AuthContext::authenticated(
            record.subject.clone(),
            record.roles.clone(),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialConfigError {
    NoCredentials,
    InvalidDigest,
    InvalidSubject,
    InvalidRole,
    DuplicateRole,
    DuplicateDigest,
}

fn parse_bearer(value: &str) -> Option<&str> {
    let (scheme, bearer) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer")
        || bearer.len() < MIN_BEARER_BYTES
        || bearer.len() > MAX_BEARER_BYTES
        || bearer.bytes().any(|byte| !byte.is_ascii_graphic())
    {
        return None;
    }
    Some(bearer)
}

fn valid_identity_field(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
}
