use crate::{EventBus, EventEnvelope, SocAssessment};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_postgres::{Client, Config, NoTls};

#[derive(Clone)]
pub struct SocCaseStore {
    client: Arc<Mutex<Client>>,
}

impl SocCaseStore {
    pub async fn connect(url: &str, password: &str) -> Result<Self, &'static str> {
        if !url.starts_with("postgresql://jarvis_soc@192.168.1.26:5432/jarvis_soc")
            || password.len() < 32
        {
            return Err("invalid SOC database configuration");
        }
        let mut config: Config = url.parse().map_err(|_| "invalid SOC database URL")?;
        config.password(password);
        config.connect_timeout(Duration::from_secs(4));
        let (client, connection) = config
            .connect(NoTls)
            .await
            .map_err(|_| "SOC database unavailable")?;
        tokio::spawn(async move {
            let _ = connection.await;
        });
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    pub async fn run_until(
        self,
        events: EventBus,
        shutdown: impl std::future::Future<Output = ()>,
    ) {
        let mut subscription = events.subscribe();
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                result = subscription.recv() => if let Ok(event) = result {
                    if let Err(error) = self.ingest(&event).await {
                        eprintln!("jarvis-core SOC case persistence failed: {error}");
                    }
                },
                () = &mut shutdown => break,
            }
        }
    }

    pub async fn ingest(&self, event: &EventEnvelope) -> Result<Option<i64>, &'static str> {
        let Some(alert) = ParsedAlert::new(event) else {
            return Ok(None);
        };
        if !matches!(alert.severity.as_str(), "critical" | "high") {
            return Ok(None);
        }
        let mut client = self.client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|_| "SOC transaction failed")?;
        let asset = tx
            .query_opt(
                "SELECT criticality FROM assets WHERE lower(host)=lower($1)",
                &[&alert.host],
            )
            .await
            .map_err(|_| "asset lookup failed")?;
        let criticality = asset
            .map(|row| row.get::<_, String>(0))
            .unwrap_or_else(|| "unknown".into());
        let priority = case_priority(&alert.severity, &criticality);
        let existing = tx.query_opt("SELECT id FROM soc_cases WHERE status IN ('open','investigating') AND lower(host)=lower($1) AND last_seen >= to_timestamp(($2::bigint)::double precision/1000)-interval '30 minutes' ORDER BY last_seen DESC LIMIT 1 FOR UPDATE", &[&alert.host, &alert.timestamp_ms]).await.map_err(|_| "active case lookup failed")?;
        let case_id: i64 = if let Some(row) = existing {
            let id = row.get(0);
            let known_alert_ids: Vec<String> = tx
                .query_one("SELECT alert_ids FROM soc_cases WHERE id=$1", &[&id])
                .await
                .map_err(|_| "case alert lookup failed")?
                .get(0);
            if known_alert_ids.iter().any(|known| known == &alert.id) {
                tx.commit()
                    .await
                    .map_err(|_| "SOC transaction commit failed")?;
                return Ok(Some(id));
            }
            tx.execute("UPDATE soc_cases SET severity=CASE WHEN severity='critical' OR $2<>'critical' THEN severity ELSE 'critical' END,priority=$3,title=$4,last_seen=to_timestamp(($5::bigint)::double precision/1000),source_ips=CASE WHEN $6='' OR $6=ANY(source_ips) THEN source_ips ELSE array_append(source_ips,$6) END,alert_ids=CASE WHEN $7=ANY(alert_ids) THEN alert_ids ELSE array_append(alert_ids,$7) END,updated_at=now() WHERE id=$1", &[&id, &alert.severity, &priority, &alert.title, &alert.timestamp_ms, &alert.source_ip, &alert.id]).await.map_err(|_| "case update failed")?;
            id
        } else {
            let key = format!(
                "{}:{}",
                alert.host.to_lowercase(),
                alert.timestamp_ms / 1_800_000
            );
            tx.query_one("INSERT INTO soc_cases(case_key,severity,priority,title,host,first_seen,last_seen,source_ips,alert_ids) VALUES($1,$2,$3,$4,$5,to_timestamp(($6::bigint)::double precision/1000),to_timestamp(($6::bigint)::double precision/1000),CASE WHEN $7='' THEN '{}' ELSE ARRAY[$7] END,ARRAY[$8]) RETURNING id", &[&key, &alert.severity, &priority, &alert.title, &alert.host, &alert.timestamp_ms, &alert.source_ip, &alert.id]).await.map_err(|_| "case creation failed")?.get(0)
        };
        tx.execute("INSERT INTO case_events(case_id,occurred_at,event_type,severity,title,evidence) VALUES($1,to_timestamp(($2::bigint)::double precision/1000),'security.alert',$3,$4,$5)", &[&case_id, &alert.timestamp_ms, &alert.severity, &alert.title, &event.payload]).await.map_err(|_| "case evidence insert failed")?;
        tx.commit()
            .await
            .map_err(|_| "SOC transaction commit failed")?;
        Ok(Some(case_id))
    }

    /// Persists immutable assessment history and updates the current case projection atomically.
    pub async fn persist_assessment(
        &self,
        assessment: &SocAssessment,
    ) -> Result<i64, &'static str> {
        let mut client = self.client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|_| "SOC assessment transaction failed")?;
        let ai_verdict = enum_text(&assessment.ai_verdict)?;
        let analysis_level = enum_text(&assessment.analysis_level)?;
        let risk_level = enum_text(&assessment.risk_level)?;
        let risk_factors =
            serde_json::to_value(&assessment.risk_factors).map_err(|_| "invalid risk factors")?;
        let positive_points = i16::try_from(assessment.positive_points)
            .map_err(|_| "positive points out of range")?;
        let negative_points = i16::try_from(assessment.negative_points)
            .map_err(|_| "negative points out of range")?;
        let row = tx.query_one(
            "INSERT INTO soc_assessments(case_id,model_alias,analysis_level,ai_verdict,confidence_score,risk_score,risk_level,summary,hypothesis,risk_factors,positive_points,negative_points,supporting_evidence,contradicting_evidence,missing_information,mitre_correlation,recommendations,assessment_version,scoring_version,confidence_version,evidence_package_version,evidence_snapshot,supersedes_assessment_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23) RETURNING assessment_id",
            &[&assessment.case_id, &assessment.model_alias, &analysis_level, &ai_verdict,
              &(assessment.confidence_score as i16), &(assessment.risk_score as i16), &risk_level,
              &assessment.summary, &assessment.hypothesis, &risk_factors,
              &positive_points, &negative_points,
              &serde_json::to_value(&assessment.supporting_evidence).map_err(|_| "invalid supporting evidence")?,
              &serde_json::to_value(&assessment.contradicting_evidence).map_err(|_| "invalid contradicting evidence")?,
              &serde_json::to_value(&assessment.missing_information).map_err(|_| "invalid missing information")?,
              &serde_json::to_value(&assessment.mitre_correlation).map_err(|_| "invalid MITRE correlation")?,
              &serde_json::to_value(&assessment.recommendations).map_err(|_| "invalid recommendations")?,
              &assessment.assessment_version, &assessment.scoring_version, &assessment.confidence_version,
              &assessment.evidence_package_version, &assessment.evidence_snapshot,
              &assessment.supersedes_assessment_id],
        ).await.map_err(|_| "assessment insert failed")?;
        let assessment_id: i64 = row.get(0);
        let updated = tx.execute(
            "UPDATE soc_cases SET risk_score=$2,risk_level=$3,ai_confidence=$4,ai_verdict=$5,assessment_version=$6,scoring_version=$7,updated_at=now() WHERE id=$1",
            &[&assessment.case_id, &(assessment.risk_score as i16), &risk_level,
              &(assessment.confidence_score as i16), &ai_verdict,
              &assessment.assessment_version, &assessment.scoring_version],
        ).await.map_err(|_| "case assessment projection failed")?;
        if updated != 1 {
            return Err("assessment case does not exist");
        }
        tx.commit()
            .await
            .map_err(|_| "SOC assessment commit failed")?;
        Ok(assessment_id)
    }
}

fn enum_text<T: serde::Serialize>(value: &T) -> Result<String, &'static str> {
    let encoded = serde_json::to_string(value).map_err(|_| "invalid assessment enum")?;
    Ok(encoded.trim_matches('"').to_owned())
}

struct ParsedAlert {
    id: String,
    host: String,
    severity: String,
    title: String,
    source_ip: String,
    timestamp_ms: i64,
}
impl ParsedAlert {
    fn new(event: &EventEnvelope) -> Option<Self> {
        if event.event_type != "security.alert" {
            return None;
        }
        let field = |name| {
            event
                .payload
                .get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
        };
        Some(Self {
            id: field("id")?.chars().take(160).collect(),
            host: field("host")?.chars().take(128).collect(),
            severity: field("severity")?.to_lowercase(),
            title: field("title")?.chars().take(200).collect(),
            source_ip: field("source_ip").unwrap_or("").chars().take(45).collect(),
            timestamp_ms: event
                .payload
                .get("timestamp_ms")
                .and_then(Value::as_u64)?
                .min(i64::MAX as u64) as i64,
        })
    }
}
fn case_priority(severity: &str, criticality: &str) -> String {
    match (severity, criticality) {
        ("critical", _) | ("high", "critical") => "p1",
        ("high", _) => "p2",
        _ => "p3",
    }
    .into()
}
