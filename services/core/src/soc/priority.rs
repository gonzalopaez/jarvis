use super::AiVerdict;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SocPriority {
    P1,
    P2,
    P3,
    P4,
}

pub fn is_critical_candidate(risk: u8, confidence: u8) -> bool {
    risk >= 90 && confidence >= 90
}

pub fn calculate_final_priority(
    initial: SocPriority,
    risk: u8,
    confidence: u8,
    verdict: AiVerdict,
) -> SocPriority {
    if is_critical_candidate(risk, confidence)
        && matches!(verdict, AiVerdict::TruePositive | AiVerdict::Suspicious)
    {
        SocPriority::P1
    } else if risk <= 49 && confidence >= 90 && verdict == AiVerdict::BenignPositive {
        SocPriority::P4
    } else {
        initial
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benign_high_confidence_can_reduce_priority() {
        assert_eq!(
            calculate_final_priority(SocPriority::P1, 31, 97, AiVerdict::BenignPositive),
            SocPriority::P4
        );
    }

    #[test]
    fn ninety_ninety_security_finding_is_p1_candidate() {
        assert_eq!(
            calculate_final_priority(SocPriority::P2, 96, 94, AiVerdict::TruePositive),
            SocPriority::P1
        );
        assert!(is_critical_candidate(90, 90));
        assert!(!is_critical_candidate(95, 89));
    }
}
