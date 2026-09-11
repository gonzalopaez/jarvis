use crate::{CommittedTaskOutcomeRecord, SkillMemoryClient, SkillMemoryError, VerifiedTaskOutcome};
use serde_json::Value;
use std::time::Duration;
use tokio_postgres::{Client, Config, NoTls};

const CLAIM_SECONDS: i64 = 30;
const MAX_ATTEMPTS: i32 = 8;
pub(crate) const WAZUH_TASK_TYPE: &str = "wazuh_alerts_read";
pub(crate) const WAZUH_APPROACH: &str =
    "Read normalized alerts through the bounded Wazuh adapter and compare severity counts.";

#[derive(Debug, Clone)]
pub struct DurableAuditEvent {
    pub audit_id: String,
    pub request_id: String,
    pub subject: String,
    pub capability: String,
    pub capability_tier: u8,
    pub outcome: &'static str,
    pub provenance: crate::TaskOutcomeProvenance,
}

#[derive(Debug, Clone)]
pub struct ClaimedOutboxEvent {
    pub event_id: String,
    pub payload: CommittedTaskOutcomeRecord,
    pub attempts: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxError {
    InvalidConfiguration,
    InvalidRecord,
    Unavailable,
    TierWriteDisabled,
    SkillMemory(SkillMemoryError),
}

#[derive(Clone)]
pub struct CoreOutboxStore {
    client: std::sync::Arc<tokio::sync::Mutex<Client>>,
}

impl CoreOutboxStore {
    pub async fn connect(url: &str, password: &str) -> Result<Self, OutboxError> {
        if !url.starts_with("postgresql://")
            || password.len() < 32
            || url.to_ascii_lowercase().contains("/jarvis_soc")
        {
            return Err(OutboxError::InvalidConfiguration);
        }
        let mut config: Config = url.parse().map_err(|_| OutboxError::InvalidConfiguration)?;
        config.password(password);
        config.connect_timeout(Duration::from_secs(4));
        let (client, connection) = config
            .connect(NoTls)
            .await
            .map_err(|_| OutboxError::Unavailable)?;
        tokio::spawn(async move {
            let _ = connection.await;
        });
        Ok(Self {
            client: std::sync::Arc::new(tokio::sync::Mutex::new(client)),
        })
    }

    /// Commits provenance audit and its verified-success event atomically.
    pub async fn commit_verified(
        &self,
        audit: &DurableAuditEvent,
        record: &CommittedTaskOutcomeRecord,
    ) -> Result<(), OutboxError> {
        if audit.outcome != "verified"
            || audit.audit_id != record.source_audit_id
            || audit.request_id != record.source_request_id
            || audit.subject != record.subject
            || audit.capability != record.capability
            || audit.capability_tier != record.capability_tier
            || record.schema_version != "task_outcome.verified.v1"
        {
            return Err(OutboxError::InvalidRecord);
        }
        let payload = serde_json::to_value(record).map_err(|_| OutboxError::InvalidRecord)?;
        let provenance =
            serde_json::to_value(&audit.provenance).map_err(|_| OutboxError::InvalidRecord)?;
        let mut client = self.client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|_| OutboxError::Unavailable)?;
        let audit_rows = tx.execute(
            "INSERT INTO jarvis_core.audit_events(audit_id,request_id,subject,capability,capability_tier,outcome,provenance) VALUES($1,$2,$3,$4,$5,'verified',$6) ON CONFLICT (audit_id) DO UPDATE SET audit_id=excluded.audit_id WHERE jarvis_core.audit_events.request_id=excluded.request_id AND jarvis_core.audit_events.subject=excluded.subject AND jarvis_core.audit_events.capability=excluded.capability AND jarvis_core.audit_events.capability_tier=excluded.capability_tier AND jarvis_core.audit_events.outcome=excluded.outcome AND jarvis_core.audit_events.provenance=excluded.provenance",
            &[&audit.audit_id, &audit.request_id, &audit.subject, &audit.capability, &(audit.capability_tier as i16), &provenance],
        ).await.map_err(|_| OutboxError::Unavailable)?;
        if audit_rows != 1 {
            return Err(OutboxError::InvalidRecord);
        }
        let outbox_rows = tx.execute(
            "INSERT INTO jarvis_core.outbox_events(event_id,event_type,audit_id,payload) VALUES($1,'task_outcome.verified.v1',$2,$3) ON CONFLICT (event_id) DO UPDATE SET event_id=excluded.event_id WHERE jarvis_core.outbox_events.audit_id=excluded.audit_id AND jarvis_core.outbox_events.payload=excluded.payload",
            &[&record.source_event_id, &record.source_audit_id, &payload],
        ).await.map_err(|_| OutboxError::Unavailable)?;
        if outbox_rows != 1 {
            return Err(OutboxError::InvalidRecord);
        }
        tx.commit().await.map_err(|_| OutboxError::Unavailable)
    }

    /// Persists a failed verification for audit, without creating learnable work.
    pub async fn commit_failure(&self, audit: &DurableAuditEvent) -> Result<(), OutboxError> {
        if audit.outcome != "verification_failed"
            || audit.capability_tier == 0
            || audit.capability_tier > 3
        {
            return Err(OutboxError::InvalidRecord);
        }
        let provenance =
            serde_json::to_value(&audit.provenance).map_err(|_| OutboxError::InvalidRecord)?;
        let changed = self.client.lock().await.execute(
            "INSERT INTO jarvis_core.audit_events(audit_id,request_id,subject,capability,capability_tier,outcome,provenance) VALUES($1,$2,$3,$4,$5,'verification_failed',$6) ON CONFLICT (audit_id) DO UPDATE SET audit_id=excluded.audit_id WHERE jarvis_core.audit_events.request_id=excluded.request_id AND jarvis_core.audit_events.subject=excluded.subject AND jarvis_core.audit_events.capability=excluded.capability AND jarvis_core.audit_events.capability_tier=excluded.capability_tier AND jarvis_core.audit_events.outcome=excluded.outcome AND jarvis_core.audit_events.provenance=excluded.provenance",
            &[&audit.audit_id, &audit.request_id, &audit.subject, &audit.capability, &(audit.capability_tier as i16), &provenance],
        ).await.map_err(|_| OutboxError::Unavailable)?;
        if changed == 1 {
            Ok(())
        } else {
            Err(OutboxError::InvalidRecord)
        }
    }

    pub async fn claim_next(&self) -> Result<Option<ClaimedOutboxEvent>, OutboxError> {
        let row = self.client.lock().await.query_opt(
            "WITH candidate AS (SELECT event_id FROM jarvis_core.outbox_events WHERE state IN ('pending','processing') AND next_attempt_at <= now() AND (locked_until IS NULL OR locked_until < now()) ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE jarvis_core.outbox_events o SET state='processing', attempts=o.attempts+1, locked_until=now()+($1::bigint * interval '1 second') FROM candidate WHERE o.event_id=candidate.event_id RETURNING o.event_id,o.payload,o.attempts",
            &[&CLAIM_SECONDS],
        ).await.map_err(|_| OutboxError::Unavailable)?;
        row.map(|row| {
            let payload: Value = row.get(1);
            Ok(ClaimedOutboxEvent {
                event_id: row.get(0),
                payload: serde_json::from_value(payload).map_err(|_| OutboxError::InvalidRecord)?,
                attempts: row.get(2),
            })
        })
        .transpose()
    }

    pub async fn mark_delivered(&self, event_id: &str) -> Result<(), OutboxError> {
        let changed = self.client.lock().await.execute(
            "UPDATE jarvis_core.outbox_events SET state='delivered',processed_at=now(),locked_until=NULL,last_error=NULL WHERE event_id=$1 AND state='processing'",
            &[&event_id],
        ).await.map_err(|_| OutboxError::Unavailable)?;
        if changed == 1 {
            Ok(())
        } else {
            Err(OutboxError::InvalidRecord)
        }
    }

    pub async fn mark_retry(
        &self,
        event: &ClaimedOutboxEvent,
        error: &str,
    ) -> Result<(), OutboxError> {
        let attempts = event.attempts.clamp(1, MAX_ATTEMPTS);
        let delay_seconds = 2_i64.pow(attempts as u32).min(300);
        let state = if event.attempts >= MAX_ATTEMPTS {
            "failed"
        } else {
            "pending"
        };
        self.client.lock().await.execute(
            "UPDATE jarvis_core.outbox_events SET state=$2,next_attempt_at=now()+($3::bigint * interval '1 second'),locked_until=NULL,last_error=$4 WHERE event_id=$1 AND state='processing'",
            &[&event.event_id, &state, &delay_seconds, &error.chars().take(256).collect::<String>()],
        ).await.map_err(|_| OutboxError::Unavailable)?;
        Ok(())
    }

    pub async fn mark_failed(
        &self,
        event: &ClaimedOutboxEvent,
        error: &str,
    ) -> Result<(), OutboxError> {
        self.client.lock().await.execute(
            "UPDATE jarvis_core.outbox_events SET state='failed',locked_until=NULL,last_error=$2 WHERE event_id=$1 AND state='processing'",
            &[&event.event_id, &error.chars().take(256).collect::<String>()],
        ).await.map_err(|_| OutboxError::Unavailable)?;
        Ok(())
    }
}

pub struct TierOneSkillConsumer {
    memory: SkillMemoryClient,
}

impl TierOneSkillConsumer {
    pub fn new(memory: SkillMemoryClient) -> Self {
        Self { memory }
    }

    pub async fn deliver(&self, record: CommittedTaskOutcomeRecord) -> Result<(), OutboxError> {
        Self::admit(&record)?;
        let verified =
            VerifiedTaskOutcome::from_committed_record(record).map_err(OutboxError::SkillMemory)?;
        self.memory
            .write_verified(&verified)
            .await
            .map_err(OutboxError::SkillMemory)
    }

    fn admit(record: &CommittedTaskOutcomeRecord) -> Result<(), OutboxError> {
        if record.capability_tier != 1 {
            eprintln!(
                "JARVIS_SKILL_OUTBOX rejected event={} reason=tier_write_disabled tier={}",
                record.source_event_id, record.capability_tier
            );
            return Err(OutboxError::TierWriteDisabled);
        }
        validate_tier_one_policy(record)?;
        Ok(())
    }
}

fn validate_tier_one_policy(record: &CommittedTaskOutcomeRecord) -> Result<(), OutboxError> {
    if record.capability != "wazuh.alerts.read"
        || record.task_type != WAZUH_TASK_TYPE
        || record.approach != WAZUH_APPROACH
        || record.provenance.kind != "validated_read_adapter"
        || record.provenance.adapter != "wazuh-relay-http"
        || record.provenance.response_schema != "wazuh-normalized.v1"
        || !valid_wazuh_count_summary(&record.context_summary)
    {
        return Err(OutboxError::InvalidRecord);
    }
    Ok(())
}

fn valid_wazuh_count_summary(value: &str) -> bool {
    let Some(values) = value.strip_prefix("Alert counts: total=") else {
        return false;
    };
    let labels = [", critical=", ", high=", ", medium=", ", low="];
    let mut remainder = values;
    for label in labels {
        let Some((number, rest)) = remainder.split_once(label) else {
            return false;
        };
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        remainder = rest;
    }
    !remainder.is_empty() && remainder.bytes().all(|byte| byte.is_ascii_digit())
}

pub async fn run_skill_outbox_until(
    store: CoreOutboxStore,
    consumer: TierOneSkillConsumer,
    shutdown: impl std::future::Future<Output = ()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let Ok(Some(event)) = store.claim_next().await else { continue };
                match consumer.deliver(event.payload.clone()).await {
                    Ok(()) => {
                        if store.mark_delivered(&event.event_id).await.is_err() {
                            eprintln!("JARVIS_SKILL_OUTBOX delivery acknowledgement failed");
                        }
                    }
                    Err(OutboxError::TierWriteDisabled) => {
                        let _ = store.mark_failed(&event, "tier_write_disabled").await;
                    }
                    Err(_) => {
                        let _ = store.mark_retry(&event, "skill_delivery_failed").await;
                    }
                }
            },
            () = &mut shutdown => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SkillMemoryConfig;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    #[test]
    fn migration_has_locking_and_idempotency_guards() {
        let migration = include_str!("../core-migrations/0001_core_audit_outbox.sql");
        let source = include_str!("task_outbox.rs");
        assert!(migration.contains("event_id text PRIMARY KEY"));
        assert!(source.contains("FOR UPDATE SKIP LOCKED"));
    }

    fn record(tier: u8) -> CommittedTaskOutcomeRecord {
        CommittedTaskOutcomeRecord {
            schema_version: "task_outcome.verified.v1".into(),
            source_event_id: "event-1".into(),
            source_audit_id: "audit-1".into(),
            source_request_id: "request-1".into(),
            subject: "operator".into(),
            task_type: WAZUH_TASK_TYPE.into(),
            context_summary: "Alert counts: total=1, critical=1, high=0, medium=0, low=0".into(),
            approach: WAZUH_APPROACH.into(),
            capability: if tier == 1 {
                "wazuh.alerts.read"
            } else if tier == 2 {
                "security.ip.block"
            } else {
                "proxmox.vm.deploy"
            }
            .into(),
            capability_tier: tier,
            executor_verified: true,
            human_authorization_audit_id: (tier > 1).then(|| "authorization-1".into()),
            provenance: crate::TaskOutcomeProvenance {
                kind: "validated_read_adapter".into(),
                adapter: "wazuh-relay-http".into(),
                response_schema: "wazuh-normalized.v1".into(),
            },
            created_at: "2026-09-11T12:00:00Z".into(),
            committed_at_epoch_seconds: 1_789_128_000,
        }
    }

    #[test]
    fn tier_one_policy_accepts_only_curated_wazuh_records() {
        assert_eq!(validate_tier_one_policy(&record(1)), Ok(()));
        let mut arbitrary = record(1);
        arbitrary.approach = "A model said this worked".into();
        assert_eq!(
            validate_tier_one_policy(&arbitrary),
            Err(OutboxError::InvalidRecord)
        );
    }

    #[test]
    fn tier_two_and_three_are_explicitly_disabled_before_writing() {
        for tier in [2, 3] {
            let item = record(tier);
            assert_eq!(item.capability_tier, tier);
            assert_eq!(
                TierOneSkillConsumer::admit(&item),
                Err(OutboxError::TierWriteDisabled)
            );
        }
    }

    #[tokio::test]
    async fn tier_one_write_is_idempotent_and_retrievable() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let writes = Arc::new(Mutex::new(Vec::new()));
        let captured = writes.clone();
        let server = std::thread::spawn(move || {
            for _ in 0..6 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0_u8; 16 * 1024];
                let length = stream.read(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..length]);
                let body = if request.starts_with("POST /v1/embeddings ") {
                    r#"{"data":[{"embedding":[0.1,0.2]}]}"#.to_owned()
                } else if request.starts_with("PUT /collections/") {
                    captured.lock().unwrap().push(request.to_string());
                    r#"{"result":{"status":"completed"}}"#.to_owned()
                } else if request.starts_with("POST /collections/") {
                    r#"{"result":[{"score":0.99,"payload":{"schema_version":"jarvis.skill.v1","skill_id":"skill-1","task_type":"wazuh_alerts_read","context_summary":"Alert counts: total=1, critical=1, high=0, medium=0, low=0","approach":"Read normalized alerts through the bounded Wazuh adapter and compare severity counts.","outcome":"success","capability":"wazuh.alerts.read","capability_tier":1,"human_confirmed":false,"source_event_id":"event-1","source_audit_id":"audit-1","created_at":"2026-09-11T12:00:00Z","expires_at_epoch_seconds":2000000000,"revoked":false}}]}"#.to_owned()
                } else {
                    panic!("unexpected request: {request}");
                };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            }
        });
        let base: reqwest::Url = format!("http://{address}/").parse().unwrap();
        let memory = SkillMemoryClient::new(SkillMemoryConfig {
            litellm_base_url: base.clone(),
            litellm_token: "x".repeat(32),
            embedding_model: "test-embed".into(),
            qdrant_base_url: base,
            collection: "jarvis_skill_memory_v1".into(),
            score_threshold: 0.6,
        })
        .unwrap();
        let consumer = TierOneSkillConsumer::new(memory.clone());
        consumer.deliver(record(1)).await.unwrap();
        consumer.deliver(record(1)).await.unwrap();
        let context = memory
            .retrieve(WAZUH_TASK_TYPE, "recent alerts")
            .await
            .unwrap()
            .unwrap();
        server.join().unwrap();
        assert!(context.contains("Alert counts: total=1"));
        let writes = writes.lock().unwrap();
        assert_eq!(writes.len(), 2);
        let id_marker = "\"id\":";
        let first_id = writes[0]
            .split(id_marker)
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        let second_id = writes[1]
            .split(id_marker)
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        assert_eq!(first_id, second_id);
    }
}
