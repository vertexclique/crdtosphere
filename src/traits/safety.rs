//! Safety CRDT trait definition
//!
//! This module defines traits for CRDTs that must meet safety standards
//! across multiple domains (automotive, industrial, aerospace).

use crate::error::CRDTResult;
use crate::memory::MemoryConfig;
use crate::traits::CRDT;

/// Trait for CRDTs that provide safety guarantees
///
/// This trait extends the base CRDT trait with safety-specific operations
/// that ensure compliance with various safety standards.
pub trait SafetyCRDT<C: MemoryConfig>: CRDT<C> {
    /// The safety level this CRDT operates at
    type SafetyLevel: PartialOrd + Copy;

    /// Returns the current safety level
    fn safety_level(&self) -> Self::SafetyLevel;

    /// Performs a safety-aware merge operation
    ///
    /// This merge prioritizes higher safety levels and ensures that
    /// safety-critical data is never overwritten by lower safety data.
    fn safety_merge(&mut self, other: &Self) -> CRDTResult<()>;

    /// Validates safety constraints
    ///
    /// This method checks that all safety invariants are maintained
    /// and that the CRDT state is consistent with safety requirements.
    fn validate_safety(&self) -> CRDTResult<()>;

    /// Checks if this CRDT can safely merge with another
    ///
    /// Returns true if the merge would not violate any safety constraints.
    fn can_safely_merge(&self, other: &Self) -> bool;

    /// Returns the minimum safety level required for operations
    fn min_safety_level(&self) -> Self::SafetyLevel;

    /// Sets the safety level for this CRDT
    fn set_safety_level(&mut self, level: Self::SafetyLevel) -> CRDTResult<()>;

    /// Performs a safety check on the current state
    fn safety_check(&self) -> CRDTResult<SafetyStatus>;

    /// Returns safety metadata for this CRDT
    fn safety_metadata(&self) -> SafetyMetadata<Self::SafetyLevel>;
}

/// Safety status information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafetyStatus {
    /// Whether the CRDT is in a safe state
    pub is_safe: bool,
    /// Safety level compliance
    pub compliance_level: u8,
    /// Number of safety violations detected
    pub violation_count: u32,
    /// Last safety check timestamp
    pub last_check_time: u32,
}

impl SafetyStatus {
    /// Creates a new safe status
    pub const fn safe() -> Self {
        Self {
            is_safe: true,
            compliance_level: 100,
            violation_count: 0,
            last_check_time: 0,
        }
    }

    /// Creates a new unsafe status
    pub const fn unsafe_with_violations(violations: u32) -> Self {
        Self {
            is_safe: false,
            compliance_level: 0,
            violation_count: violations,
            last_check_time: 0,
        }
    }

    /// Checks if the status indicates a critical safety issue
    pub const fn is_critical(&self) -> bool {
        !self.is_safe && self.violation_count > 0
    }
}

/// Safety metadata for CRDTs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafetyMetadata<S> {
    /// Current safety level
    pub safety_level: S,
    /// Minimum required safety level
    pub min_safety_level: S,
    /// Safety certification level
    pub certification_level: CertificationLevel,
    /// Last safety validation time
    pub last_validation: u32,
    /// Safety check interval in cycles
    pub check_interval: u32,
}

/// Safety certification levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CertificationLevel {
    /// No certification
    None = 0,
    /// Basic certification
    Basic = 1,
    /// Standard certification
    Standard = 2,
    /// High certification
    High = 3,
    /// Critical certification
    Critical = 4,
}

/// Trait for CRDTs that support fail-safe operations
///
/// This trait provides methods for CRDTs to handle failure scenarios
/// and ensure they can reach a safe state even under adverse conditions.
pub trait FailSafeCRDT<C: MemoryConfig>: SafetyCRDT<C> {
    /// Enters fail-safe mode
    ///
    /// In fail-safe mode, the CRDT operates with reduced functionality
    /// but maintains safety guarantees.
    fn enter_fail_safe_mode(&mut self) -> CRDTResult<()>;

    /// Exits fail-safe mode
    ///
    /// Returns to normal operation if it's safe to do so.
    fn exit_fail_safe_mode(&mut self) -> CRDTResult<()>;

    /// Checks if the CRDT is in fail-safe mode
    fn is_in_fail_safe_mode(&self) -> bool;

    /// Performs a controlled shutdown
    ///
    /// Ensures the CRDT reaches a safe state before shutdown.
    fn safe_shutdown(&mut self) -> CRDTResult<()>;

    /// Recovers from a failure state
    ///
    /// Attempts to restore normal operation after a failure.
    fn recover_from_failure(&mut self) -> CRDTResult<()>;

    /// Returns the current failure state
    fn failure_state(&self) -> FailureState;
}

/// Failure state information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureState {
    /// Normal operation
    Normal,
    /// Degraded operation
    Degraded,
    /// Fail-safe mode
    FailSafe,
    /// Critical failure
    Critical,
    /// Shutdown
    Shutdown,
}

/// Trait for CRDTs that support safety monitoring
///
/// This trait provides methods for continuous safety monitoring
/// and automatic safety responses.
pub trait SafetyMonitor<C: MemoryConfig>: SafetyCRDT<C> {
    /// Starts safety monitoring
    fn start_monitoring(&mut self) -> CRDTResult<()>;

    /// Stops safety monitoring
    fn stop_monitoring(&mut self) -> CRDTResult<()>;

    /// Checks if monitoring is active
    fn is_monitoring_active(&self) -> bool;

    /// Performs a periodic safety check
    fn periodic_safety_check(&mut self) -> CRDTResult<SafetyStatus>;

    /// Sets the safety check interval
    fn set_check_interval(&mut self, interval_cycles: u32);

    /// Returns the current check interval
    fn check_interval(&self) -> u32;

    /// Registers a safety violation
    fn register_violation(&mut self, violation: SafetyViolation);

    /// Returns the violation history
    fn violation_history(&self) -> &[SafetyViolation];

    /// Clears the violation history
    fn clear_violations(&mut self);
}

/// Safety violation information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafetyViolation {
    /// Type of violation
    pub violation_type: ViolationType,
    /// Severity level (0-255)
    pub severity: u8,
    /// Timestamp when violation occurred
    pub timestamp: u32,
    /// Safety level at time of violation
    pub safety_level: u8,
}

/// Types of safety violations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationType {
    /// Safety level downgrade
    SafetyLevelDowngrade,
    /// Integrity check failure
    IntegrityFailure,
    /// Timeout violation
    TimeoutViolation,
    /// Resource constraint violation
    ResourceViolation,
    /// Protocol violation
    ProtocolViolation,
    /// Custom violation
    Custom(u8),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CRDTError;
    use crate::memory::DefaultConfig;

    // Simple safety level for testing
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum TestSafetyLevel {
        Low = 1,
        Medium = 2,
        High = 3,
    }

    // Mock safety CRDT for testing
    struct MockSafetyCRDT {
        value: u32,
        safety_level: TestSafetyLevel,
        fail_safe_mode: bool,
    }

    impl MockSafetyCRDT {
        fn new(safety_level: TestSafetyLevel) -> Self {
            Self {
                value: 0,
                safety_level,
                fail_safe_mode: false,
            }
        }
    }

    impl CRDT<DefaultConfig> for MockSafetyCRDT {
        type Error = CRDTError;

        fn merge(&mut self, other: &Self) -> CRDTResult<()> {
            self.value = self.value.max(other.value);
            Ok(())
        }

        fn eq(&self, other: &Self) -> bool {
            self.value == other.value && self.safety_level == other.safety_level
        }

        fn size_bytes(&self) -> usize {
            core::mem::size_of::<Self>()
        }

        fn validate(&self) -> CRDTResult<()> {
            Ok(())
        }

        fn state_hash(&self) -> u32 {
            self.value
        }

        fn can_merge(&self, _other: &Self) -> bool {
            true
        }
    }

    impl SafetyCRDT<DefaultConfig> for MockSafetyCRDT {
        type SafetyLevel = TestSafetyLevel;

        fn safety_level(&self) -> Self::SafetyLevel {
            self.safety_level
        }

        fn safety_merge(&mut self, other: &Self) -> CRDTResult<()> {
            // Higher safety level wins
            if other.safety_level > self.safety_level {
                self.value = other.value;
                self.safety_level = other.safety_level;
            } else if other.safety_level == self.safety_level {
                self.value = self.value.max(other.value);
            }
            // Lower safety level is ignored
            Ok(())
        }

        fn validate_safety(&self) -> CRDTResult<()> {
            if self.fail_safe_mode && self.safety_level < TestSafetyLevel::Medium {
                return Err(CRDTError::InvalidSafetyLevel);
            }
            Ok(())
        }

        fn can_safely_merge(&self, other: &Self) -> bool {
            !self.fail_safe_mode || other.safety_level >= TestSafetyLevel::Medium
        }

        fn min_safety_level(&self) -> Self::SafetyLevel {
            TestSafetyLevel::Low
        }

        fn set_safety_level(&mut self, level: Self::SafetyLevel) -> CRDTResult<()> {
            if level < self.min_safety_level() {
                return Err(CRDTError::InvalidSafetyLevel);
            }
            self.safety_level = level;
            Ok(())
        }

        fn safety_check(&self) -> CRDTResult<SafetyStatus> {
            if self.validate_safety().is_ok() {
                Ok(SafetyStatus::safe())
            } else {
                Ok(SafetyStatus::unsafe_with_violations(1))
            }
        }

        fn safety_metadata(&self) -> SafetyMetadata<Self::SafetyLevel> {
            SafetyMetadata {
                safety_level: self.safety_level,
                min_safety_level: self.min_safety_level(),
                certification_level: CertificationLevel::Standard,
                last_validation: 0,
                check_interval: 1000,
            }
        }
    }

    impl FailSafeCRDT<DefaultConfig> for MockSafetyCRDT {
        fn enter_fail_safe_mode(&mut self) -> CRDTResult<()> {
            self.fail_safe_mode = true;
            Ok(())
        }

        fn exit_fail_safe_mode(&mut self) -> CRDTResult<()> {
            self.fail_safe_mode = false;
            Ok(())
        }

        fn is_in_fail_safe_mode(&self) -> bool {
            self.fail_safe_mode
        }

        fn safe_shutdown(&mut self) -> CRDTResult<()> {
            self.fail_safe_mode = true;
            Ok(())
        }

        fn recover_from_failure(&mut self) -> CRDTResult<()> {
            if self.safety_check()?.is_safe {
                self.fail_safe_mode = false;
            }
            Ok(())
        }

        fn failure_state(&self) -> FailureState {
            if self.fail_safe_mode {
                FailureState::FailSafe
            } else {
                FailureState::Normal
            }
        }
    }

    #[test]
    fn test_safety_crdt() {
        let mut crdt1 = MockSafetyCRDT::new(TestSafetyLevel::Medium);
        let crdt2 = MockSafetyCRDT::new(TestSafetyLevel::High);

        assert_eq!(crdt1.safety_level(), TestSafetyLevel::Medium);
        assert!(crdt1.can_safely_merge(&crdt2));

        // Higher safety level should win
        assert!(crdt1.safety_merge(&crdt2).is_ok());
        assert_eq!(crdt1.safety_level(), TestSafetyLevel::High);

        let status = crdt1.safety_check().unwrap();
        assert!(status.is_safe);
    }

    #[test]
    fn test_fail_safe_crdt() {
        let mut crdt = MockSafetyCRDT::new(TestSafetyLevel::Low);

        assert!(!crdt.is_in_fail_safe_mode());
        assert_eq!(crdt.failure_state(), FailureState::Normal);

        assert!(crdt.enter_fail_safe_mode().is_ok());
        assert!(crdt.is_in_fail_safe_mode());
        assert_eq!(crdt.failure_state(), FailureState::FailSafe);

        assert!(crdt.exit_fail_safe_mode().is_ok());
        assert!(!crdt.is_in_fail_safe_mode());
    }

    #[test]
    fn test_safety_status() {
        let safe_status = SafetyStatus::safe();
        assert!(safe_status.is_safe);
        assert!(!safe_status.is_critical());

        let unsafe_status = SafetyStatus::unsafe_with_violations(3);
        assert!(!unsafe_status.is_safe);
        assert!(unsafe_status.is_critical());
        assert_eq!(unsafe_status.violation_count, 3);
    }
}
