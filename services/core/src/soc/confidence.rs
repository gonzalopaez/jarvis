use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CONFIDENCE_VERSION: &str = "confidence-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSource {
    pub source: String,
    pub source_id: String,
}

pub fn count_independent_sources(evidence: &[EvidenceSource]) -> usize {
    evidence
        .iter()
        .map(|item| {
            let source = if item.source.eq_ignore_ascii_case("wazuh-agent")
                || item.source.eq_ignore_ascii_case("n8n-wazuh")
                || item.source.eq_ignore_ascii_case("wazuh")
            {
                "wazuh".to_owned()
            } else {
                item.source.to_lowercase()
            };
            (source, item.source_id.clone())
        })
        .collect::<BTreeSet<_>>()
        .len()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfidenceInput {
    pub authoritative_source: bool,
    pub unique_supporting_evidence: usize,
    pub temporal_correlation: bool,
    pub independent_sources: usize,
    pub validated_historical_similarity: bool,
    pub contradictions: usize,
    pub missing_information: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfidenceFactor {
    pub factor: String,
    pub raw_value: String,
    pub points: i16,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfidenceResult {
    pub score: u8,
    pub positive_points: u16,
    pub negative_points: u16,
    pub confidence_version: String,
    pub factors: Vec<ConfidenceFactor>,
}

pub fn calculate_confidence(input: &ConfidenceInput) -> ConfidenceResult {
    let factors = vec![
        item(
            "authoritative_source",
            input.authoritative_source.to_string(),
            if input.authoritative_source { 35 } else { 10 },
            "Source quality baseline",
        ),
        item(
            "supporting_evidence",
            input.unique_supporting_evidence.to_string(),
            capped(input.unique_supporting_evidence, 8, 32),
            "Unique resolvable supporting evidence, capped",
        ),
        item(
            "temporal_correlation",
            input.temporal_correlation.to_string(),
            if input.temporal_correlation { 12 } else { 0 },
            "Deterministic temporal consistency",
        ),
        item(
            "independent_sources",
            input.independent_sources.to_string(),
            capped(input.independent_sources.saturating_sub(1), 8, 16),
            "Independent sources beyond the first, capped",
        ),
        item(
            "historical_similarity",
            input.validated_historical_similarity.to_string(),
            if input.validated_historical_similarity {
                8
            } else {
                0
            },
            "Validated historical similarity",
        ),
        item(
            "contradictions",
            input.contradictions.to_string(),
            -capped(input.contradictions, 15, 45),
            "Contradicting evidence reduces confidence",
        ),
        item(
            "missing_information",
            input.missing_information.to_string(),
            -capped(input.missing_information, 6, 30),
            "Hypothesis-relevant missing information reduces confidence",
        ),
    ];
    let positive_points = factors.iter().map(|f| f.points.max(0) as u16).sum::<u16>();
    let negative_points = factors
        .iter()
        .map(|f| (-f.points.min(0)) as u16)
        .sum::<u16>();
    ConfidenceResult {
        score: positive_points.saturating_sub(negative_points).min(100) as u8,
        positive_points,
        negative_points,
        confidence_version: CONFIDENCE_VERSION.into(),
        factors,
    }
}
fn item(
    factor: &'static str,
    raw_value: String,
    points: i16,
    reason: &'static str,
) -> ConfidenceFactor {
    ConfidenceFactor {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reproducible_high_confidence_result() {
        let input = ConfidenceInput {
            authoritative_source: true,
            unique_supporting_evidence: 3,
            temporal_correlation: true,
            independent_sources: 3,
            validated_historical_similarity: true,
            contradictions: 0,
            missing_information: 0,
        };
        let result = calculate_confidence(&input);
        assert_eq!(result, calculate_confidence(&input));
        assert_eq!(result.score, 95);
    }
    #[test]
    fn contradictions_are_not_risk() {
        let result = calculate_confidence(&ConfidenceInput {
            authoritative_source: true,
            unique_supporting_evidence: 2,
            contradictions: 2,
            missing_information: 1,
            ..Default::default()
        });
        assert_eq!(result.score, 15);
        assert_eq!(result.negative_points, 36);
    }
    #[test]
    fn duplicate_adapters_are_not_independent_sources() {
        let sources = vec![
            EvidenceSource {
                source: "wazuh-agent".into(),
                source_id: "A-42".into(),
            },
            EvidenceSource {
                source: "n8n-wazuh".into(),
                source_id: "A-42".into(),
            },
            EvidenceSource {
                source: "prometheus".into(),
                source_id: "M-1".into(),
            },
        ];
        assert_eq!(count_independent_sources(&sources), 2);
    }

    #[test]
    fn unique_evidence_changes_score_once() {
        let one = calculate_confidence(&ConfidenceInput {
            authoritative_source: true,
            unique_supporting_evidence: 1,
            ..Default::default()
        });
        let two = calculate_confidence(&ConfidenceInput {
            authoritative_source: true,
            unique_supporting_evidence: 2,
            ..Default::default()
        });
        assert_eq!(two.score - one.score, 8);
    }
}
