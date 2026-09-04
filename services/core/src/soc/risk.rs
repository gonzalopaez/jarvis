use serde::{Deserialize, Serialize};

pub const RISK_SCORING_VERSION: &str = "risk-v1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiskInput {
    pub wazuh_level: Option<u8>,
    pub asset_criticality: Option<String>,
    pub privileged_identity: Option<bool>,
    pub correlated_alerts: usize,
    pub mitre_techniques: usize,
    pub mitre_tactics: usize,
    pub temporal_progression: bool,
    pub validated_iocs: usize,
    pub historical_true_positives: usize,
    pub historical_benign_positives: usize,
    pub recurrence_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskFactor {
    pub factor: String,
    pub raw_value: String,
    pub points: i16,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    VeryHigh,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskResult {
    pub score: u8,
    pub level: RiskLevel,
    pub positive_points: u16,
    pub negative_points: u16,
    pub scoring_version: String,
    pub factors: Vec<RiskFactor>,
}

pub fn calculate_risk(input: &RiskInput) -> RiskResult {
    let mut factors = Vec::new();
    let level_points = input
        .wazuh_level
        .map(|level| match level {
            0..=3 => 2,
            4..=6 => 6,
            7..=9 => 12,
            10..=11 => 20,
            12..=13 => 28,
            _ => 35,
        })
        .unwrap_or(0);
    factors.push(factor(
        "wazuh_level",
        input.wazuh_level.map_or("null".into(), |v| v.to_string()),
        level_points,
        "Monotonic contribution from the Wazuh rule level",
    ));
    let criticality = input.asset_criticality.as_deref().unwrap_or("unknown");
    let asset_points = match criticality {
        "critical" => 18,
        "high" => 12,
        "medium" => 6,
        "low" => 2,
        _ => 0,
    };
    factors.push(factor(
        "asset_criticality",
        criticality.into(),
        asset_points,
        "Impact contribution from authoritative asset inventory",
    ));
    factors.push(factor(
        "privileged_identity",
        format_option_bool(input.privileged_identity),
        if input.privileged_identity == Some(true) {
            10
        } else {
            0
        },
        "Privilege contributes only when explicitly evidenced",
    ));
    factors.push(factor(
        "correlated_alerts",
        input.correlated_alerts.to_string(),
        capped(input.correlated_alerts.saturating_sub(1), 4, 16),
        "Additional unique correlated alerts, capped",
    ));
    factors.push(factor(
        "mitre_techniques",
        input.mitre_techniques.to_string(),
        capped(input.mitre_techniques, 3, 12),
        "Wazuh-provided MITRE technique diversity, capped",
    ));
    factors.push(factor(
        "mitre_tactics",
        input.mitre_tactics.to_string(),
        capped(input.mitre_tactics, 2, 8),
        "Wazuh-provided MITRE tactic diversity, capped",
    ));
    factors.push(factor(
        "temporal_progression",
        input.temporal_progression.to_string(),
        if input.temporal_progression { 10 } else { 0 },
        "Ordered related activity contributes only after deterministic correlation",
    ));
    factors.push(factor(
        "validated_iocs",
        input.validated_iocs.to_string(),
        capped(input.validated_iocs, 4, 12),
        "Validated IOC evidence, capped",
    ));
    factors.push(factor(
        "historical_true_positives",
        input.historical_true_positives.to_string(),
        capped(input.historical_true_positives, 3, 9),
        "Comparable prior true positives, capped",
    ));
    factors.push(factor(
        "historical_benign_positives",
        input.historical_benign_positives.to_string(),
        -capped(input.historical_benign_positives, 4, 16),
        "Comparable benign history reduces risk, capped",
    ));
    factors.push(factor(
        "recurrence",
        input.recurrence_count.to_string(),
        capped(input.recurrence_count, 2, 8),
        "Defined-window recurrence, capped",
    ));
    finish(factors)
}

fn factor(
    factor: &'static str,
    raw_value: String,
    points: i16,
    reason: &'static str,
) -> RiskFactor {
    RiskFactor {
        factor: factor.into(),
        raw_value,
        points,
        reason: reason.into(),
    }
}
fn capped(count: usize, per: i16, maximum: i16) -> i16 {
    (count.min(i16::MAX as usize) as i16)
        .saturating_mul(per)
        .min(maximum)
}
fn format_option_bool(value: Option<bool>) -> String {
    value.map_or("null".into(), |v| v.to_string())
}
fn finish(factors: Vec<RiskFactor>) -> RiskResult {
    let positive_points = factors.iter().map(|f| f.points.max(0) as u16).sum::<u16>();
    let negative_points = factors
        .iter()
        .map(|f| (-f.points.min(0)) as u16)
        .sum::<u16>();
    let score = positive_points.saturating_sub(negative_points).min(100) as u8;
    let level = match score {
        0..=29 => RiskLevel::Low,
        30..=49 => RiskLevel::Medium,
        50..=69 => RiskLevel::High,
        70..=89 => RiskLevel::VeryHigh,
        _ => RiskLevel::Critical,
    };
    RiskResult {
        score,
        level,
        positive_points,
        negative_points,
        scoring_version: RISK_SCORING_VERSION.into(),
        factors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_and_explainable() {
        let input = RiskInput {
            wazuh_level: Some(13),
            asset_criticality: Some("critical".into()),
            privileged_identity: Some(true),
            correlated_alerts: 3,
            mitre_techniques: 3,
            mitre_tactics: 2,
            temporal_progression: true,
            validated_iocs: 1,
            historical_true_positives: 1,
            historical_benign_positives: 0,
            recurrence_count: 1,
        };
        let a = calculate_risk(&input);
        for _ in 0..100 {
            assert_eq!(a, calculate_risk(&input));
        }
        assert_eq!(a.score, 96);
        assert_eq!(a.level, RiskLevel::Critical);
        assert!(a.factors.iter().all(|f| !f.reason.is_empty()));
    }
    #[test]
    fn benign_history_reduces_without_underflow() {
        let result = calculate_risk(&RiskInput {
            historical_benign_positives: 100,
            ..Default::default()
        });
        assert_eq!(result.score, 0);
        assert_eq!(result.negative_points, 16);
    }
    #[test]
    fn boundaries_are_stable() {
        assert_eq!(
            finish(vec![factor("x", "29".into(), 29, "test")]).level,
            RiskLevel::Low
        );
        assert_eq!(
            finish(vec![factor("x", "30".into(), 30, "test")]).level,
            RiskLevel::Medium
        );
        assert_eq!(
            finish(vec![factor("x", "90".into(), 90, "test")]).level,
            RiskLevel::Critical
        );
    }
}
