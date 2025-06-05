//! Bounded CRDT trait definition
//!
//! This module defines traits for CRDTs that have bounded memory usage
//! and can verify their resource constraints at compile time.

use crate::error::CRDTResult;
use crate::memory::MemoryConfig;
use crate::traits::CRDT;

/// Trait for CRDTs with bounded memory usage
///
/// This trait ensures that CRDTs have predictable memory usage that can be
/// verified at compile time, which is essential for embedded systems.
pub trait BoundedCRDT<C: MemoryConfig>: CRDT<C> {
    /// Maximum size in bytes this CRDT can occupy
    const MAX_SIZE_BYTES: usize;

    /// Memory alignment requirement in bytes
    const ALIGNMENT: usize = C::MEMORY_ALIGNMENT;

    /// Maximum number of elements this CRDT can contain
    const MAX_ELEMENTS: usize;

    /// Returns the current memory usage in bytes
    fn memory_usage(&self) -> usize;

    /// Returns the remaining memory capacity in bytes
    fn remaining_capacity(&self) -> usize {
        Self::MAX_SIZE_BYTES.saturating_sub(self.memory_usage())
    }

    /// Checks if the CRDT is at its memory limit
    fn is_at_capacity(&self) -> bool {
        self.memory_usage() >= Self::MAX_SIZE_BYTES
    }

    /// Returns the memory utilization as a percentage (0-100)
    fn utilization_percent(&self) -> u8 {
        if Self::MAX_SIZE_BYTES == 0 {
            return 100;
        }

        let utilization = (self.memory_usage() * 100) / Self::MAX_SIZE_BYTES;
        utilization.min(100) as u8
    }

    /// Checks if adding an element would exceed memory bounds
    fn can_add_element(&self) -> bool {
        self.element_count() < Self::MAX_ELEMENTS && !self.is_at_capacity()
    }

    /// Returns the current number of elements
    fn element_count(&self) -> usize;

    /// Returns the maximum number of elements that can be stored
    fn max_elements(&self) -> usize {
        Self::MAX_ELEMENTS
    }

    /// Validates that the CRDT is within its memory bounds
    fn validate_bounds(&self) -> CRDTResult<()> {
        if self.memory_usage() > Self::MAX_SIZE_BYTES {
            return Err(crate::error::CRDTError::BufferOverflow);
        }

        if self.element_count() > Self::MAX_ELEMENTS {
            return Err(crate::error::CRDTError::ConfigurationExceeded);
        }

        Ok(())
    }

    /// Compacts the CRDT to reduce memory usage if possible
    fn compact(&mut self) -> CRDTResult<usize>;

    /// Returns memory statistics for this CRDT
    fn memory_stats(&self) -> MemoryStats {
        MemoryStats {
            current_usage: self.memory_usage(),
            max_capacity: Self::MAX_SIZE_BYTES,
            element_count: self.element_count(),
            max_elements: Self::MAX_ELEMENTS,
            utilization_percent: self.utilization_percent(),
        }
    }
}

/// Memory statistics for bounded CRDTs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStats {
    /// Current memory usage in bytes
    pub current_usage: usize,
    /// Maximum memory capacity in bytes
    pub max_capacity: usize,
    /// Current number of elements
    pub element_count: usize,
    /// Maximum number of elements
    pub max_elements: usize,
    /// Memory utilization percentage (0-100)
    pub utilization_percent: u8,
}

impl MemoryStats {
    /// Returns the remaining memory capacity
    pub fn remaining_capacity(&self) -> usize {
        self.max_capacity.saturating_sub(self.current_usage)
    }

    /// Returns the remaining element capacity
    pub fn remaining_elements(&self) -> usize {
        self.max_elements.saturating_sub(self.element_count)
    }

    /// Checks if the CRDT is at capacity
    pub fn is_at_capacity(&self) -> bool {
        self.current_usage >= self.max_capacity || self.element_count >= self.max_elements
    }

    /// Checks if the CRDT is nearly full (>90% utilization)
    pub fn is_nearly_full(&self) -> bool {
        self.utilization_percent >= 90
    }
}

/// Trait for CRDTs that support memory pressure handling
///
/// This trait provides methods for CRDTs to handle memory pressure situations
/// and implement strategies for memory management.
pub trait MemoryPressureHandler<C: MemoryConfig>: BoundedCRDT<C> {
    /// Handles memory pressure by freeing up space
    ///
    /// Returns the number of bytes freed
    fn handle_memory_pressure(&mut self) -> CRDTResult<usize>;

    /// Returns the memory pressure level (0-100)
    fn memory_pressure_level(&self) -> u8 {
        self.utilization_percent()
    }

    /// Checks if the CRDT is under memory pressure
    fn is_under_pressure(&self) -> bool {
        self.memory_pressure_level() >= 80
    }

    /// Sets the memory pressure threshold (0-100)
    fn set_pressure_threshold(&mut self, threshold: u8);

    /// Returns the current memory pressure threshold
    fn pressure_threshold(&self) -> u8;
}

/// Trait for CRDTs that support garbage collection
///
/// This trait provides methods for CRDTs to perform garbage collection
/// to reclaim unused memory.
pub trait GarbageCollectable<C: MemoryConfig>: BoundedCRDT<C> {
    /// Performs garbage collection
    ///
    /// Returns the number of bytes freed
    fn garbage_collect(&mut self) -> CRDTResult<usize>;

    /// Returns the amount of garbage (reclaimable memory) in bytes
    fn garbage_size(&self) -> usize;

    /// Checks if garbage collection is needed
    fn needs_gc(&self) -> bool {
        self.garbage_size() > 0 || self.utilization_percent() >= 80
    }

    /// Sets the garbage collection threshold
    fn set_gc_threshold(&mut self, threshold: usize);

    /// Returns the current garbage collection threshold
    fn gc_threshold(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CRDTError;
    use crate::memory::DefaultConfig;

    // Mock bounded CRDT for testing
    struct MockBoundedCRDT {
        elements: [Option<u32>; 10],
        count: usize,
    }

    impl MockBoundedCRDT {
        fn new() -> Self {
            Self {
                elements: [None; 10],
                count: 0,
            }
        }

        fn add(&mut self, value: u32) -> bool {
            if self.count < 10 {
                self.elements[self.count] = Some(value);
                self.count += 1;
                true
            } else {
                false
            }
        }
    }

    impl CRDT<DefaultConfig> for MockBoundedCRDT {
        type Error = CRDTError;

        fn merge(&mut self, other: &Self) -> CRDTResult<()> {
            for &element in other.elements.iter().flatten() {
                if !self.add(element) {
                    return Err(CRDTError::BufferOverflow);
                }
            }
            Ok(())
        }

        fn eq(&self, other: &Self) -> bool {
            self.elements == other.elements
        }

        fn size_bytes(&self) -> usize {
            core::mem::size_of::<Self>()
        }

        fn validate(&self) -> CRDTResult<()> {
            Ok(())
        }

        fn state_hash(&self) -> u32 {
            self.count as u32
        }

        fn can_merge(&self, other: &Self) -> bool {
            self.count + other.count <= 10
        }
    }

    impl BoundedCRDT<DefaultConfig> for MockBoundedCRDT {
        const MAX_SIZE_BYTES: usize = 64;
        const MAX_ELEMENTS: usize = 10;

        fn memory_usage(&self) -> usize {
            // XXX: Return memory usage based on actual elements stored
            let base_size = core::mem::size_of::<usize>(); // for count field
            let element_size = self.count * core::mem::size_of::<Option<u32>>();
            base_size + element_size
        }

        fn element_count(&self) -> usize {
            self.count
        }

        fn compact(&mut self) -> CRDTResult<usize> {
            // No compaction needed for this simple example
            Ok(0)
        }
    }

    #[test]
    fn test_bounded_crdt() {
        let mut crdt = MockBoundedCRDT::new();

        assert_eq!(crdt.element_count(), 0);
        assert_eq!(crdt.max_elements(), 10);
        assert!(crdt.can_add_element());
        assert!(!crdt.is_at_capacity());

        // Add some elements
        for i in 0..5 {
            assert!(crdt.add(i));
        }

        assert_eq!(crdt.element_count(), 5);
        // With 5 elements, memory usage should be reasonable (not 100%)
        assert!(crdt.utilization_percent() < 100);
        assert!(crdt.can_add_element()); // Should still be able to add more elements

        let stats = crdt.memory_stats();
        assert_eq!(stats.element_count, 5);
        assert_eq!(stats.max_elements, 10);
        assert_eq!(stats.remaining_elements(), 5);
    }

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats {
            current_usage: 32,
            max_capacity: 64,
            element_count: 5,
            max_elements: 10,
            utilization_percent: 50,
        };

        assert_eq!(stats.remaining_capacity(), 32);
        assert_eq!(stats.remaining_elements(), 5);
        assert!(!stats.is_at_capacity());
        assert!(!stats.is_nearly_full());
    }
}
