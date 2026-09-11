use super::{RiskFactor, RiskLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiVerdict {
    FalsePositive,
    BenignPositive,
    Suspicious,
    TruePositive,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnalystVerdict {
    Pending,
    FalsePositive,
    BenignPositive,
    Suspicious,
    TruePositive,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisLevel {
    L1,
    L2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocAssessment {
    pub assessment_version: String,
    pub case_id: i64,
    pub model_alias: String,
    pub analysis_level: AnalysisLevel,
    pub ai_verdict: AiVerdict,
    pub confidence_score: u8,
    pub risk_score: u8,
    pub risk_level: RiskLevel,
    pub summary: String,
    pub hypothesis: String,
    pub supporting_evidence: Vec<String>,
    pub contradicting_evidence: Vec<String>,
    pub missing_information: Vec<String>,
    pub recommendations: Vec<Value>,
    pub risk_factors: Vec<RiskFactor>,
    pub positive_points: u16,
    pub negative_points: u16,
    pub mitre_correlation: Vec<Value>,
    pub evidence_package_version: String,
    pub evidence_snapshot: Value,
    pub supersedes_assessment_id: Option<i64>,
    pub scoring_version: String,
    pub confidence_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_and_analyst_verdicts_are_distinct_domains() {
        assert_eq!(
            serde_json::to_string(&AiVerdict::TruePositive).unwrap(),
            "\"TRUE_POSITIVE\""
        );
        assert_eq!(
            serde_json::to_string(&AnalystVerdict::Pending).unwrap(),
            "\"PENDING\""
        );
    }
}
