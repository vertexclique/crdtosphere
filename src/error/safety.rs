//! Safety-specific error types
//!
//! This module defines error types related to safety standards compliance
//! across multiple domains (automotive, industrial, aerospace).

/// Safety error types for multi-domain compliance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyError {
    // Automotive (ISO 26262) errors
    /// ASIL level violation
    ASILViolation {
        /// Required ASIL level
        required: ASILLevel,
        /// Actual ASIL level
        actual: ASILLevel,
    },
    /// Automotive safety function failure
    SafetyFunctionFailure,
    /// Diagnostic coverage insufficient
    DiagnosticCoverageInsufficient,

    // Industrial (IEC 61508) errors
    /// SIL level violation
    SILViolation {
        /// Required SIL level
        required: SILLevel,
        /// Actual SIL level
        actual: SILLevel,
    },
    /// Safety instrumented function failure
    SIFFailure,
    /// Proof test coverage insufficient
    ProofTestCoverageInsufficient,

    // Aerospace (DO-178C) errors
    /// DAL level violation
    DALViolation {
        /// Required DAL level
        required: DALLevel,
        /// Actual DAL level
        actual: DALLevel,
    },
    /// Software level failure
    SoftwareLevelFailure,
    /// Verification coverage insufficient
    VerificationCoverageInsufficient,

    // Generic safety errors
    /// Safety monitor timeout
    SafetyMonitorTimeout,
    /// Redundancy failure
    RedundancyFailure,
    /// Safety barrier breach
    SafetyBarrierBreach,
    /// Fail-safe state not reachable
    FailSafeStateUnreachable,
    /// Safety critical data corruption
    SafetyCriticalDataCorruption,
}

/// Automotive Safety Integrity Level (ISO 26262)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ASILLevel {
    /// Quality Management (non-safety)
    QM = 0,
    /// ASIL A (lowest safety level)
    A = 1,
    /// ASIL B
    B = 2,
    /// ASIL C
    C = 3,
    /// ASIL D (highest safety level)
    D = 4,
}

/// Safety Integrity Level (IEC 61508)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SILLevel {
    /// SIL 1 (lowest safety level)
    SIL1 = 1,
    /// SIL 2
    SIL2 = 2,
    /// SIL 3
    SIL3 = 3,
    /// SIL 4 (highest safety level)
    SIL4 = 4,
}

/// Design Assurance Level (DO-178C)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DALLevel {
    /// DAL E (no safety effect)
    E = 0,
    /// DAL D (minor safety effect)
    D = 1,
    /// DAL C (major safety effect)
    C = 2,
    /// DAL B (hazardous safety effect)
    B = 3,
    /// DAL A (catastrophic safety effect)
    A = 4,
}

/// Universal safety level that can represent any domain
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    /// Automotive ASIL level
    ASIL(ASILLevel),
    /// Industrial SIL level
    SIL(SILLevel),
    /// Aerospace DAL level
    DAL(DALLevel),
    /// Custom safety level
    Custom(u8),
}

impl SafetyLevel {
    /// Returns the numeric safety level (higher = more critical)
    pub const fn numeric_level(&self) -> u8 {
        match self {
            Self::ASIL(level) => *level as u8,
            Self::SIL(level) => *level as u8,
            Self::DAL(level) => *level as u8,
            Self::Custom(level) => *level,
        }
    }

    /// Checks if this safety level is compatible with another
    pub const fn is_compatible_with(&self, other: &Self) -> bool {
        match (self, other) {
            // Same domain comparisons
            (Self::ASIL(_), Self::ASIL(_)) => true,
            (Self::SIL(_), Self::SIL(_)) => true,
            (Self::DAL(_), Self::DAL(_)) => true,
            (Self::Custom(_), Self::Custom(_)) => true,
            // Cross-domain comparisons not directly compatible
            _ => false,
        }
    }

    /// Returns true if this safety level is higher than or equal to the other
    pub const fn satisfies(&self, required: &Self) -> bool {
        if !self.is_compatible_with(required) {
            return false;
        }

        self.numeric_level() >= required.numeric_level()
    }
}

impl PartialOrd for SafetyLevel {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self.is_compatible_with(other) {
            Some(self.numeric_level().cmp(&other.numeric_level()))
        } else {
            None
        }
    }
}

impl SafetyError {
    /// Returns true if this is a critical safety error
    pub const fn is_critical(&self) -> bool {
        match self {
            Self::ASILViolation { required, .. } => matches!(required, ASILLevel::C | ASILLevel::D),
            Self::SILViolation { required, .. } => {
                matches!(required, SILLevel::SIL3 | SILLevel::SIL4)
            }
            Self::DALViolation { required, .. } => matches!(required, DALLevel::A | DALLevel::B),
            Self::SafetyFunctionFailure
            | Self::SIFFailure
            | Self::SoftwareLevelFailure
            | Self::RedundancyFailure
            | Self::SafetyBarrierBreach
            | Self::FailSafeStateUnreachable
            | Self::SafetyCriticalDataCorruption => true,
            _ => false,
        }
    }

    /// Returns the safety domain this error belongs to
    pub const fn domain(&self) -> &'static str {
        match self {
            Self::ASILViolation { .. }
            | Self::SafetyFunctionFailure
            | Self::DiagnosticCoverageInsufficient => "Automotive",

            Self::SILViolation { .. } | Self::SIFFailure | Self::ProofTestCoverageInsufficient => {
                "Industrial"
            }

            Self::DALViolation { .. }
            | Self::SoftwareLevelFailure
            | Self::VerificationCoverageInsufficient => "Aerospace",

            _ => "Generic",
        }
    }
}

impl From<ASILLevel> for SafetyLevel {
    fn from(level: ASILLevel) -> Self {
        Self::ASIL(level)
    }
}

impl From<SILLevel> for SafetyLevel {
    fn from(level: SILLevel) -> Self {
        Self::SIL(level)
    }
}

impl From<DALLevel> for SafetyLevel {
    fn from(level: DALLevel) -> Self {
        Self::DAL(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asil_level_ordering() {
        assert!(ASILLevel::D > ASILLevel::C);
        assert!(ASILLevel::C > ASILLevel::B);
        assert!(ASILLevel::B > ASILLevel::A);
        assert!(ASILLevel::A > ASILLevel::QM);
    }

    #[test]
    fn test_safety_level_compatibility() {
        let asil_d = SafetyLevel::ASIL(ASILLevel::D);
        let asil_c = SafetyLevel::ASIL(ASILLevel::C);
        let sil_4 = SafetyLevel::SIL(SILLevel::SIL4);

        assert!(asil_d.is_compatible_with(&asil_c));
        assert!(!asil_d.is_compatible_with(&sil_4));

        assert!(asil_d.satisfies(&asil_c));
        assert!(!asil_c.satisfies(&asil_d));
    }

    #[test]
    fn test_safety_error_criticality() {
        let critical_error = SafetyError::ASILViolation {
            required: ASILLevel::D,
            actual: ASILLevel::B,
        };

        let non_critical_error = SafetyError::ASILViolation {
            required: ASILLevel::A,
            actual: ASILLevel::QM,
        };

        assert!(critical_error.is_critical());
        assert!(!non_critical_error.is_critical());
    }

    #[test]
    fn test_safety_error_domain() {
        let automotive_error = SafetyError::SafetyFunctionFailure;
        let industrial_error = SafetyError::SIFFailure;
        let aerospace_error = SafetyError::SoftwareLevelFailure;
        let generic_error = SafetyError::SafetyMonitorTimeout;

        assert_eq!(automotive_error.domain(), "Automotive");
        assert_eq!(industrial_error.domain(), "Industrial");
        assert_eq!(aerospace_error.domain(), "Aerospace");
        assert_eq!(generic_error.domain(), "Generic");
    }
}
