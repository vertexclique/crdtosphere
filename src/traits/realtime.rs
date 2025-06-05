//! Real-time CRDT trait definition
//!
//! This module defines traits for CRDTs that must meet real-time constraints.

use crate::error::CRDTResult;
use crate::memory::MemoryConfig;
use crate::traits::CRDT;

/// Trait for CRDTs that provide real-time guarantees
///
/// This trait extends the base CRDT trait with real-time specific operations
/// that ensure bounded execution time and deterministic behavior.
pub trait RealTimeCRDT<C: MemoryConfig>: CRDT<C> {
    /// Maximum number of CPU cycles for merge operation
    const MAX_MERGE_CYCLES: u32;

    /// Maximum number of CPU cycles for validation
    const MAX_VALIDATE_CYCLES: u32;

    /// Maximum number of CPU cycles for serialization
    const MAX_SERIALIZE_CYCLES: u32;

    /// Performs a bounded merge operation
    ///
    /// This operation is guaranteed to complete within MAX_MERGE_CYCLES
    /// or return a timeout error.
    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()>;

    /// Performs bounded validation
    ///
    /// This operation is guaranteed to complete within MAX_VALIDATE_CYCLES
    /// or return a timeout error.
    fn validate_bounded(&self) -> CRDTResult<()>;

    /// Returns the worst-case execution time for merge in CPU cycles
    fn merge_wcet(&self) -> u32 {
        Self::MAX_MERGE_CYCLES
    }

    /// Returns the worst-case execution time for validation in CPU cycles
    fn validate_wcet(&self) -> u32 {
        Self::MAX_VALIDATE_CYCLES
    }

    /// Checks if the operation can complete within the given deadline
    fn can_meet_deadline(&self, operation: RTOperation, deadline_cycles: u32) -> bool {
        let wcet = match operation {
            RTOperation::Merge => self.merge_wcet(),
            RTOperation::Validate => self.validate_wcet(),
            RTOperation::Serialize => Self::MAX_SERIALIZE_CYCLES,
        };
        wcet <= deadline_cycles
    }

    /// Returns the current execution time budget remaining
    fn remaining_budget(&self) -> Option<u32>;

    /// Sets the execution time budget for operations
    fn set_budget(&mut self, cycles: u32);
}

/// Real-time operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RTOperation {
    /// Merge operation
    Merge,
    /// Validation operation
    Validate,
    /// Serialization operation
    Serialize,
}

/// Trait for CRDTs that support interrupt-safe operations
///
/// This trait provides methods for CRDTs that can be safely accessed
/// from interrupt contexts without causing priority inversion.
pub trait InterruptSafeCRDT<C: MemoryConfig>: RealTimeCRDT<C> {
    /// Performs an atomic merge operation that is interrupt-safe
    fn atomic_merge(&mut self, other: &Self) -> CRDTResult<()>;

    /// Performs an atomic read operation that is interrupt-safe
    fn atomic_read<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Self) -> R;

    /// Performs an atomic write operation that is interrupt-safe
    fn atomic_write<F>(&mut self, f: F) -> CRDTResult<()>
    where
        F: FnOnce(&mut Self) -> CRDTResult<()>;

    /// Returns true if the CRDT is currently locked
    fn is_locked(&self) -> bool;

    /// Returns the maximum interrupt latency this CRDT can cause
    fn max_interrupt_latency(&self) -> u32;
}

/// Trait for CRDTs that support priority-based operations
///
/// This trait allows CRDTs to handle operations with different priority levels,
/// ensuring that high-priority operations can preempt lower-priority ones.
pub trait PrioritizedCRDT<C: MemoryConfig>: RealTimeCRDT<C> {
    /// Priority level for operations
    type Priority: PartialOrd + Copy;

    /// Performs a merge with the given priority
    fn merge_with_priority(&mut self, other: &Self, priority: Self::Priority) -> CRDTResult<()>;

    /// Checks if an operation with the given priority can preempt current operations
    fn can_preempt(&self, priority: Self::Priority) -> bool;

    /// Returns the current operation priority
    fn current_priority(&self) -> Option<Self::Priority>;

    /// Sets the priority for subsequent operations
    fn set_priority(&mut self, priority: Self::Priority);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CRDTError;
    use crate::memory::DefaultConfig;

    // Mock real-time CRDT for testing
    struct MockRTCRDT {
        value: u32,
        budget: Option<u32>,
    }

    impl CRDT<DefaultConfig> for MockRTCRDT {
        type Error = CRDTError;

        fn merge(&mut self, other: &Self) -> CRDTResult<()> {
            self.value = self.value.max(other.value);
            Ok(())
        }

        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }

        fn size_bytes(&self) -> usize {
            core::mem::size_of::<u32>()
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

    impl RealTimeCRDT<DefaultConfig> for MockRTCRDT {
        const MAX_MERGE_CYCLES: u32 = 100;
        const MAX_VALIDATE_CYCLES: u32 = 50;
        const MAX_SERIALIZE_CYCLES: u32 = 75;

        fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
            // Simulate bounded merge
            self.merge(other)
        }

        fn validate_bounded(&self) -> CRDTResult<()> {
            // Simulate bounded validation
            self.validate()
        }

        fn remaining_budget(&self) -> Option<u32> {
            self.budget
        }

        fn set_budget(&mut self, cycles: u32) {
            self.budget = Some(cycles);
        }
    }

    #[test]
    fn test_realtime_crdt() {
        let mut crdt = MockRTCRDT {
            value: 10,
            budget: None,
        };

        assert_eq!(crdt.merge_wcet(), 100);
        assert_eq!(crdt.validate_wcet(), 50);

        assert!(crdt.can_meet_deadline(RTOperation::Merge, 150));
        assert!(!crdt.can_meet_deadline(RTOperation::Merge, 50));

        crdt.set_budget(1000);
        assert_eq!(crdt.remaining_budget(), Some(1000));
    }
}
