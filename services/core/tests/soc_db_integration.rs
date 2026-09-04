#![cfg(feature = "integration-tests")]

use jarvis_core::{
    AiVerdict, AnalysisLevel, AnalystVerdict, EventEnvelope, RiskLevel, SocAssessment, SocCaseStore,
};
use serde_json::json;
use std::sync::OnceLock;
use tokio_postgres::{Client, NoTls};

static TEST_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn test_url() -> String {
    let url = std::env::var("JARVIS_SOC_TEST_DB_URL")
        .expect("JARVIS_SOC_TEST_DB_URL is required; refusing implicit database selection");
    let lower = url.to_ascii_lowercase();
    assert!(
        !lower.contains("192.168.1.26"),
        "production CT133 address is forbidden"
    );
    assert!(
        !lower.contains("jarvis-soc-db"),
        "production hostname is forbidden"
    );
    let db = url
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    assert!(
        db.contains("test") || db.contains("rehearsal") || db.contains("nonprod"),
        "database must be explicitly nonprod"
    );
    assert_ne!(db, "jarvis_soc", "production database is forbidden");
    url
}

async fn probe(url: &str) -> (Client, tokio::task::JoinHandle<()>) {
    let (client, connection) = tokio_postgres::connect(url, NoTls)
        .await
        .expect("test DB connection");
    let handle = tokio::spawn(async move {
        let _ = connection.await;
    });
    let db: String = client
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    assert!(db.contains("test") || db.contains("rehearsal") || db.contains("nonprod"));
    let _: String = client
        .query_one("SELECT version()", &[])
        .await
        .unwrap()
        .get(0);
    (client, handle)
}

fn alert(id: &str, ts: u64, mitre: bool) -> EventEnvelope {
    EventEnvelope {
        event_version: "1",
        event_id: format!("synthetic-{id}"),
        event_type: "security.alert",
        timestamp_ms: ts,
        correlation_id: None,
        payload: json!({
            "id": id, "host": "SYN-INTEGRATION-01", "timestamp_ms": ts,
            "severity": "critical", "title": "Synthetic Wazuh alert", "source_ip": "192.0.2.10",
            "wazuh": if mitre { json!({"mitre":[{"id":"T1078"},{"id":"T1059.001"},{"id":"T1105"}]}) } else { json!({}) }
        }),
    }
}

const BASE_TS: u64 = 1_788_523_200_000;

fn assessment(
    case_id: i64,
    level: AnalysisLevel,
    risk: u8,
    confidence: u8,
    verdict: AiVerdict,
    supersedes: Option<i64>,
) -> SocAssessment {
    SocAssessment {
        assessment_version: "integration-v1".into(),
        case_id,
        model_alias: "jarvis-soc-l1".into(),
        analysis_level: level,
        ai_verdict: verdict,
        confidence_score: confidence,
        risk_score: risk,
        risk_level: if risk >= 90 {
            RiskLevel::Critical
        } else {
            RiskLevel::VeryHigh
        },
        summary: "synthetic".into(),
        hypothesis: "synthetic".into(),
        supporting_evidence: vec!["event-1".into()],
        contradicting_evidence: vec![],
        missing_information: vec![],
        recommendations: vec![],
        risk_factors: vec![],
        positive_points: risk as u16,
        negative_points: 0,
        mitre_correlation: vec![json!({"id":"T1078"})],
        evidence_package_version: "evidence-v1".into(),
        evidence_snapshot: json!({"synthetic":true}),
        supersedes_assessment_id: supersedes,
        scoring_version: "risk-v1".into(),
        confidence_version: "confidence-v1".into(),
    }
}

#[tokio::test]
async fn nonprod_guard_and_canonical_alert_persist() {
    let _guard = TEST_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let url = test_url();
    let (db, _connection) = probe(&url).await;
    let store = SocCaseStore::connect_test(&url)
        .await
        .expect("guarded test connection");
    db.execute("DELETE FROM soc_feedback", &[]).await.unwrap();
    db.execute("DELETE FROM soc_assessments", &[])
        .await
        .unwrap();
    db.execute("DELETE FROM case_events WHERE case_id IN (SELECT id FROM soc_cases WHERE host='SYN-INTEGRATION-01')", &[]).await.unwrap();
    db.execute("DELETE FROM soc_cases WHERE host='SYN-INTEGRATION-01'", &[])
        .await
        .unwrap();
    let event = alert("INT-MITRE-1", BASE_TS, true);
    let case_id = store.ingest(&event).await.unwrap().expect("case created");
    let row = db.query_one("SELECT to_char(e.occurred_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), c.alert_ids, e.evidence->'wazuh'->'mitre' FROM case_events e JOIN soc_cases c ON c.id=e.case_id WHERE e.case_id=$1", &[&case_id]).await.unwrap();
    let occurred: String = row.get(0);
    assert_eq!(occurred, "2026-09-04T12:00:00Z");
    let ids: Vec<String> = row.get(1);
    assert_eq!(ids, vec!["INT-MITRE-1"]);
    let mitre = row.get::<_, serde_json::Value>(2);
    assert_eq!(
        mitre
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["T1078", "T1059.001", "T1105"]
    );
}

#[tokio::test]
async fn legacy_dedup_and_thirty_minute_grouping_work_on_new_schema() {
    let _guard = TEST_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let url = test_url();
    let (db, _connection) = probe(&url).await;
    let store = SocCaseStore::connect_test(&url).await.unwrap();
    db.execute("DELETE FROM soc_feedback", &[]).await.unwrap();
    db.execute("DELETE FROM soc_assessments", &[])
        .await
        .unwrap();
    db.execute("DELETE FROM case_events WHERE case_id IN (SELECT id FROM soc_cases WHERE host='SYN-INTEGRATION-01')", &[]).await.unwrap();
    db.execute("DELETE FROM soc_cases WHERE host='SYN-INTEGRATION-01'", &[])
        .await
        .unwrap();
    let first = alert("INT-GROUP-1", BASE_TS, false);
    let first_id = store.ingest(&first).await.unwrap().unwrap();
    assert_eq!(store.ingest(&first).await.unwrap(), Some(first_id));
    assert_eq!(
        store
            .ingest(&alert("INT-GROUP-2", BASE_TS + 5 * 60_000, false))
            .await
            .unwrap(),
        Some(first_id)
    );
    let outside = store
        .ingest(&alert("INT-GROUP-3", BASE_TS + 36 * 60_000, false))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(outside, first_id);
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM case_events WHERE case_id=$1",
            &[&first_id]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        2
    );
}

#[tokio::test]
async fn assessments_are_append_only_and_projection_is_latest() {
    let _guard = TEST_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let url = test_url();
    let (db, _connection) = probe(&url).await;
    let store = SocCaseStore::connect_test(&url).await.unwrap();
    db.execute("DELETE FROM soc_feedback", &[]).await.unwrap();
    db.execute("DELETE FROM soc_assessments", &[])
        .await
        .unwrap();
    db.execute("DELETE FROM case_events WHERE case_id IN (SELECT id FROM soc_cases WHERE case_key='INT-ASSESS')", &[]).await.unwrap();
    db.execute("DELETE FROM soc_cases WHERE case_key='INT-ASSESS'", &[])
        .await
        .unwrap();
    let case_id: i64 = db.query_one("INSERT INTO soc_cases(case_key,severity,priority,title,host,first_seen,last_seen) VALUES ('INT-ASSESS','critical','p2','synthetic','SYN-ASSESS',now(),now()) RETURNING id", &[]).await.unwrap().get(0);
    let l1 = store
        .persist_assessment(&assessment(
            case_id,
            AnalysisLevel::L1,
            81,
            63,
            AiVerdict::Suspicious,
            None,
        ))
        .await
        .unwrap();
    let _l2 = store
        .persist_assessment(&assessment(
            case_id,
            AnalysisLevel::L2,
            94,
            93,
            AiVerdict::TruePositive,
            Some(l1),
        ))
        .await
        .unwrap();
    store
        .record_analyst_verdict(
            case_id,
            Some(_l2),
            Some(AnalystVerdict::Pending),
            AnalystVerdict::FalsePositive,
            "known exception",
            "synthetic analyst review",
            "analyst-synthetic",
            "req-synthetic",
        )
        .await
        .unwrap();
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM soc_assessments WHERE case_id=$1",
            &[&case_id]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        2
    );
    let row = db
        .query_one(
            "SELECT risk_score,ai_confidence,ai_verdict FROM soc_cases WHERE id=$1",
            &[&case_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i16>(0), 94);
    assert_eq!(row.get::<_, i16>(1), 93);
    assert_eq!(row.get::<_, String>(2), "TRUE_POSITIVE");
    assert_eq!(
        db.query_one(
            "SELECT analyst_verdict FROM soc_cases WHERE id=$1",
            &[&case_id]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        "FALSE_POSITIVE"
    );
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM soc_feedback WHERE case_id=$1",
            &[&case_id]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    let mitre: serde_json::Value = db
        .query_one(
            "SELECT mitre_correlation FROM soc_assessments WHERE assessment_id=$1",
            &[&_l2],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(mitre[0]["id"], "T1078");
}

#[tokio::test]
async fn assessment_insert_failure_rolls_back_without_projection() {
    let _guard = TEST_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let url = test_url();
    let (db, _connection) = probe(&url).await;
    let store = SocCaseStore::connect_test(&url).await.unwrap();
    db.execute("DELETE FROM soc_feedback", &[]).await.unwrap();
    db.execute("DELETE FROM soc_assessments", &[])
        .await
        .unwrap();
    db.execute("DELETE FROM case_events WHERE case_id IN (SELECT id FROM soc_cases WHERE case_key='INT-ROLLBACK')", &[]).await.unwrap();
    db.execute("DELETE FROM soc_cases WHERE case_key='INT-ROLLBACK'", &[])
        .await
        .unwrap();
    let case_id: i64 = db.query_one("INSERT INTO soc_cases(case_key,severity,priority,title,host,first_seen,last_seen) VALUES ('INT-ROLLBACK','high','p2','synthetic','SYN-ROLLBACK',now(),now()) RETURNING id", &[]).await.unwrap().get(0);
    assert!(store
        .persist_assessment_for_test(
            &assessment(
                case_id,
                AnalysisLevel::L1,
                81,
                63,
                AiVerdict::Suspicious,
                None
            ),
            true
        )
        .await
        .is_err());
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM soc_assessments WHERE case_id=$1",
            &[&case_id]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    assert_eq!(
        db.query_one("SELECT risk_score FROM soc_cases WHERE id=$1", &[&case_id])
            .await
            .unwrap()
            .get::<_, Option<i16>>(0),
        None
    );
}

#[tokio::test]
async fn malformed_alert_identifiers_remain_absent() {
    let _guard = TEST_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let url = test_url();
    let (db, _connection) = probe(&url).await;
    let store = SocCaseStore::connect_test(&url).await.unwrap();
    let event = EventEnvelope {
        event_version: "1",
        event_id: "synthetic-null".into(),
        event_type: "security.alert",
        timestamp_ms: BASE_TS,
        correlation_id: None,
        payload: json!({"host":"SYN-NULL","severity":"high","title":"Synthetic incomplete","timestamp_ms":BASE_TS}),
    };
    assert_eq!(store.ingest(&event).await.unwrap(), None);
    assert_eq!(
        db.query_one("SELECT count(*) FROM soc_cases WHERE host='SYN-NULL'", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
async fn assessment_insert_constraint_failure_does_not_change_projection() {
    let _guard = TEST_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let url = test_url();
    let (db, _connection) = probe(&url).await;
    let store = SocCaseStore::connect_test(&url).await.unwrap();
    assert!(store
        .persist_assessment(&assessment(
            9_999_999,
            AnalysisLevel::L1,
            50,
            50,
            AiVerdict::Suspicious,
            None
        ))
        .await
        .is_err());
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM soc_assessments WHERE case_id=9999999",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
}
