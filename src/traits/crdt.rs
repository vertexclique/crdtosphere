//! Base CRDT trait definition
//!
//! This module defines the fundamental CRDT trait that all Conflict-free
//! Replicated Data Types must implement.

use crate::error::CRDTResult;
use crate::memory::MemoryConfig;

/// Base trait for all Conflict-free Replicated Data Types
///
/// This trait defines the fundamental operations that all CRDTs must support.
/// The trait is parameterized by a memory configuration to enable compile-time
/// resource management.
pub trait CRDT<C: MemoryConfig> {
    /// The error type for CRDT operations
    type Error;

    /// Merges another CRDT instance into this one
    ///
    /// This operation must be:
    /// - Commutative: merge(a, b) = merge(b, a)
    /// - Associative: merge(merge(a, b), c) = merge(a, merge(b, c))
    /// - Idempotent: merge(a, a) = a
    ///
    /// # Arguments
    ///
    /// * `other` - The other CRDT instance to merge
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the merge was successful, or an error if the merge failed.
    fn merge(&mut self, other: &Self) -> CRDTResult<()>;

    /// Checks if this CRDT is equal to another
    ///
    /// Two CRDTs are considered equal if they represent the same logical state,
    /// regardless of their internal representation or history.
    fn eq(&self, other: &Self) -> bool;

    /// Returns the current size of the CRDT in bytes
    ///
    /// This includes all internal state and metadata required for the CRDT
    /// to function correctly.
    fn size_bytes(&self) -> usize;

    /// Validates the internal consistency of the CRDT
    ///
    /// This method checks that the CRDT's internal state is consistent and
    /// that all invariants are maintained.
    fn validate(&self) -> CRDTResult<()>;

    /// Returns a hash of the CRDT's logical state
    ///
    /// This hash should be the same for CRDTs that represent the same logical
    /// state, regardless of their internal representation.
    fn state_hash(&self) -> u32;

    /// Checks if the CRDT can be merged with another without exceeding limits
    ///
    /// This method allows checking merge compatibility before attempting the
    /// actual merge operation.
    fn can_merge(&self, other: &Self) -> bool;
}

/// Trait for CRDTs that support partial ordering
///
/// Some CRDTs have a natural partial ordering based on their logical state.
/// This trait provides methods to compare CRDT instances.
pub trait PartiallyOrdered<C: MemoryConfig>: CRDT<C> {
    /// Checks if this CRDT is less than or equal to another
    ///
    /// Returns `Some(true)` if this CRDT is less than or equal to the other,
    /// `Some(false)` if it is greater, or `None` if they are incomparable.
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering>;

    /// Checks if this CRDT is causally before another
    /// In a sense, this is causality check.
    fn happens_before(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(core::cmp::Ordering::Less))
    }

    /// Checks if this CRDT is concurrent with another
    fn is_concurrent(&self, other: &Self) -> bool {
        self.partial_cmp(other).is_none()
    }
}

/// Trait for CRDTs that support delta operations
///
/// Delta CRDTs can generate and apply deltas (incremental changes) rather
/// than merging entire states, which can be more efficient for network
/// transmission.
pub trait DeltaCRDT<C: MemoryConfig>: CRDT<C> {
    /// The type representing a delta (incremental change)
    type Delta;

    /// Generates a delta representing changes since the given state
    fn delta_since(&self, other: &Self) -> Option<Self::Delta>;

    /// Applies a delta to this CRDT
    fn apply_delta(&mut self, delta: &Self::Delta) -> CRDTResult<()>;

    /// Merges two deltas into a single delta
    fn merge_deltas(delta1: &Self::Delta, delta2: &Self::Delta) -> Self::Delta;

    /// Returns the size of a delta in bytes
    fn delta_size(delta: &Self::Delta) -> usize;
}

/// Trait for CRDTs that support causal consistency
///
/// Causal CRDTs maintain causal relationships between operations and can
/// detect causality violations.
pub trait CausalCRDT<C: MemoryConfig>: CRDT<C> {
    /// The type representing a causal context (e.g., vector clock)
    type CausalContext;

    /// Returns the current causal context
    fn causal_context(&self) -> &Self::CausalContext;

    /// Checks if an operation is causally ready to be applied
    fn is_causally_ready(&self, context: &Self::CausalContext) -> bool;

    /// Updates the causal context after an operation
    fn update_causal_context(&mut self, context: Self::CausalContext);

    /// Checks for causal violations
    fn check_causality(&self, other: &Self) -> CRDTResult<()>;
}

/// Trait for CRDTs that support serialization
///
/// This trait provides methods for serializing and deserializing CRDTs
/// for network transmission or persistent storage.
pub trait SerializableCRDT<C: MemoryConfig>: CRDT<C> {
    /// Serializes the CRDT to a byte buffer
    ///
    /// The buffer must be pre-allocated with sufficient capacity.
    fn serialize(&self, buffer: &mut [u8]) -> CRDTResult<usize>;

    /// Deserializes a CRDT from a byte buffer
    fn deserialize(buffer: &[u8]) -> CRDTResult<Self>
    where
        Self: Sized;

    /// Returns the maximum serialized size in bytes
    fn max_serialized_size() -> usize;

    /// Returns the actual serialized size for this instance
    fn serialized_size(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CRDTError;
    use crate::memory::DefaultConfig;

    // Mock CRDT implementation for testing
    struct MockCRDT {
        value: u32,
    }

    impl CRDT<DefaultConfig> for MockCRDT {
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

    #[test]
    fn test_crdt_merge() {
        let mut crdt1 = MockCRDT { value: 10 };
        let crdt2 = MockCRDT { value: 20 };

        assert!(crdt1.merge(&crdt2).is_ok());
        assert_eq!(crdt1.value, 20);
    }

    #[test]
    fn test_crdt_equality() {
        let crdt1 = MockCRDT { value: 10 };
        let crdt2 = MockCRDT { value: 10 };
        let crdt3 = MockCRDT { value: 20 };

        assert!(crdt1.eq(&crdt2));
        assert!(!crdt1.eq(&crdt3));
    }

    #[test]
    fn test_crdt_properties() {
        let crdt = MockCRDT { value: 42 };

        assert_eq!(crdt.size_bytes(), 4);
        assert_eq!(crdt.state_hash(), 42);
        assert!(crdt.validate().is_ok());
        assert!(crdt.can_merge(&crdt));
    }
}
