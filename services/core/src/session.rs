use crate::{AuthContext, Principal};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

pub const SESSION_COOKIE_NAME: &str = "jarvis_session";
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(8 * 60 * 60);
pub const DEFAULT_MAX_SESSIONS: usize = 1_024;
pub const MAX_SESSIONS: usize = 16_384;

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<SessionStoreInner>,
}

struct SessionStoreInner {
    sessions: Mutex<HashMap<[u8; 32], SessionRecord>>,
    ttl: Duration,
    max_sessions: usize,
}

struct SessionRecord {
    principal: Principal,
    expires_at: Instant,
    csrf_token: String,
}

pub struct IssuedSession {
    value: String,
    pub expires_at_ms: u64,
    max_age_seconds: u64,
}

impl IssuedSession {
    pub fn cookie_header(&self) -> String {
        format!(
            "{SESSION_COOKIE_NAME}={}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={}",
            self.value, self.max_age_seconds
        )
    }

    #[cfg(test)]
    pub(crate) fn value(&self) -> &str {
        &self.value
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new(DEFAULT_SESSION_TTL, DEFAULT_MAX_SESSIONS)
            .expect("default session configuration is valid")
    }
}

impl SessionStore {
    pub fn new(ttl: Duration, max_sessions: usize) -> Result<Self, SessionConfigError> {
        if ttl.is_zero()
            || ttl > Duration::from_secs(24 * 60 * 60)
            || max_sessions == 0
            || max_sessions > MAX_SESSIONS
        {
            return Err(SessionConfigError);
        }
        Ok(Self {
            inner: Arc::new(SessionStoreInner {
                sessions: Mutex::new(HashMap::new()),
                ttl,
                max_sessions,
            }),
        })
    }

    pub fn issue(&self, principal: Principal) -> Result<IssuedSession, SessionIssueError> {
        let mut random = [0_u8; 64];
        getrandom::fill(&mut random).map_err(|_| SessionIssueError)?;
        let encoded = hex::encode(&random[..32]);
        let csrf_token = hex::encode(&random[32..]);
        let digest: [u8; 32] = Sha256::digest(encoded.as_bytes()).into();
        let now = Instant::now();
        let mut sessions = self.inner.sessions.lock().map_err(|_| SessionIssueError)?;
        sessions.retain(|_, record| record.expires_at > now);
        if sessions.len() >= self.inner.max_sessions {
            return Err(SessionIssueError);
        }
        sessions.insert(
            digest,
            SessionRecord {
                principal,
                expires_at: now + self.inner.ttl,
                csrf_token,
            },
        );
        let expires_at_ms = SystemTime::now()
            .checked_add(self.inner.ttl)
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
            .unwrap_or(0);
        Ok(IssuedSession {
            value: encoded,
            expires_at_ms,
            max_age_seconds: self.inner.ttl.as_secs(),
        })
    }

    pub fn authenticate_cookie(&self, cookie_header: &str) -> Option<AuthContext> {
        let value = cookie_value(cookie_header)?;
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let digest: [u8; 32] = Sha256::digest(value.as_bytes()).into();
        let now = Instant::now();
        let mut sessions = self.inner.sessions.lock().ok()?;
        let record = sessions.get(&digest)?;
        if record.expires_at <= now {
            sessions.remove(&digest);
            return None;
        }
        Some(AuthContext {
            authenticated: true,
            principal: Some(record.principal.clone()),
        })
    }

    pub fn revoke_cookie(&self, cookie_header: &str) -> bool {
        let Some(value) = cookie_value(cookie_header) else {
            return false;
        };
        let digest: [u8; 32] = Sha256::digest(value.as_bytes()).into();
        self.inner
            .sessions
            .lock()
            .ok()
            .is_some_and(|mut sessions| sessions.remove(&digest).is_some())
    }

    pub fn csrf_token(&self, cookie_header: &str) -> Option<String> {
        let value = cookie_value(cookie_header)?;
        let digest: [u8; 32] = Sha256::digest(value.as_bytes()).into();
        let sessions = self.inner.sessions.lock().ok()?;
        let record = sessions.get(&digest)?;
        (record.expires_at > Instant::now()).then(|| record.csrf_token.clone())
    }

    pub fn validate_csrf(&self, cookie_header: &str, candidate: &str) -> bool {
        use subtle::ConstantTimeEq;
        let Some(expected) = self.csrf_token(cookie_header) else {
            return false;
        };
        candidate.len() == expected.len()
            && bool::from(candidate.as_bytes().ct_eq(expected.as_bytes()))
    }
}

fn cookie_value(header: &str) -> Option<&str> {
    header.split(';').map(str::trim).find_map(|entry| {
        let (name, value) = entry.split_once('=')?;
        (name == SESSION_COOKIE_NAME).then_some(value)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionConfigError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIssueError;

#[cfg(test)]
mod tests {
    use super::*;

    fn principal() -> Principal {
        Principal {
            subject: "operator:test".into(),
            roles: vec!["operator".into()],
        }
    }

    #[test]
    fn session_cookie_is_opaque_and_hardened() {
        let store = SessionStore::default();
        let session = store.issue(principal()).expect("session");
        assert_eq!(session.value().len(), 64);
        let cookie = session.cookie_header();
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert_eq!(
            store
                .authenticate_cookie(&cookie)
                .expect("auth")
                .principal
                .unwrap()
                .subject,
            "operator:test"
        );
        let csrf = store.csrf_token(&cookie).expect("csrf");
        assert_eq!(csrf.len(), 64);
        assert!(store.validate_csrf(&cookie, &csrf));
        assert!(!store.validate_csrf(&cookie, "invalid"));
    }

    #[test]
    fn revoked_or_malformed_sessions_fail_closed() {
        let store = SessionStore::default();
        let session = store.issue(principal()).expect("session");
        let cookie = format!("{SESSION_COOKIE_NAME}={}", session.value());
        assert!(store.revoke_cookie(&cookie));
        assert!(store.authenticate_cookie(&cookie).is_none());
        assert!(store.authenticate_cookie("jarvis_session=weak").is_none());
    }

    #[test]
    fn session_configuration_is_bounded() {
        assert!(SessionStore::new(Duration::ZERO, 1).is_err());
        assert!(SessionStore::new(Duration::from_secs(1), MAX_SESSIONS + 1).is_err());
    }
}
