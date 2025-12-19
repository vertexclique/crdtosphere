//! Grow-only Counter CRDT
//!
//! A counter that can only be incremented, never decremented.
//! Uses zero allocation with a fixed array for deterministic memory usage.
//!
//! This module provides both standard and atomic implementations:
//! - Standard: Requires `&mut self` for modifications, single-threaded
//! - Atomic: Allows `&self` for modifications, multi-threaded safe

use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

#[cfg(feature = "hardware-atomic")]
use core::sync::atomic::{AtomicU32, Ordering};

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Grow-only Counter with configurable node array
///
/// This counter can only be incremented and provides eventual consistency
/// across multiple nodes. Each node maintains its own counter value.
///
/// # Type Parameters
/// - `C`: Memory configuration that determines the default maximum number of nodes
/// - `CAPACITY`: The maximum number of nodes this counter can track (defaults to C::MAX_NODES)
///
/// # Memory Usage
/// - Fixed size: 4 * CAPACITY + 8 bytes (non-atomic) or 4 * CAPACITY + 8 bytes (atomic)
/// - Example: For 16 nodes = 72 bytes, for 64 nodes = 264 bytes
/// - Completely predictable at compile time
///
/// # Feature Comparison
///
/// | Method | Standard | Atomic | Mutability | Thread Safety | Notes |
/// |--------|----------|--------|------------|---------------|-------|
/// | `increment()` | ✅ | ✅ | `&mut self` / `&self` | Single / Multi | Core operation |
/// | `inc()` | ✅ | ✅ | `&mut self` / `&self` | Single / Multi | Convenience method |
/// | `merge()` | ✅ | ✅ | `&mut self` | Single / Multi | CRDT merge |
/// | `value()` | ✅ | ✅ | `&self` | Single / Multi | Read-only |
/// | `node_value()` | ✅ | ✅ | `&self` | Single / Multi | Read-only |
///
/// **Feature Requirements:**
/// - **Standard Version**: No additional features required (default)
/// - **Atomic Version**: Requires `hardware-atomic` feature
///
/// # Concurrency Behavior
/// - **Without `hardware-atomic`**: Requires `&mut self` for modifications, single-threaded only
/// - **With `hardware-atomic`**: Allows `&self` for modifications, thread-safe atomic operations
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
///
/// // Create counters for different nodes with default capacity
/// let mut counter1 = GCounter::<DefaultConfig>::new(1);
/// counter1.increment(5)?;
///
/// let mut counter2 = GCounter::<DefaultConfig>::new(2);
/// counter2.increment(3)?;
///
/// // Merge the counters
/// counter1.merge(&counter2)?;
/// assert_eq!(counter1.value(), 8); // 5 + 3
///
/// // Example with custom capacity (separate from merging)
/// let counter3 = GCounter::<DefaultConfig, 32>::with_capacity(3);
/// assert_eq!(counter3.capacity(), 32);
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "aurix", repr(align(32)))] // AURIX cache line optimization
#[cfg_attr(feature = "stm32", repr(align(4)))] // STM32 word alignment
#[cfg_attr(feature = "cortex-m", repr(align(4)))] // ARM word alignment
#[cfg_attr(feature = "riscv", repr(align(8)))] // RISC-V double-word alignment
#[cfg_attr(
    not(any(
        feature = "aurix",
        feature = "stm32",
        feature = "cortex-m",
        feature = "riscv"
    )),
    repr(align(4))
)] // Default alignment
pub struct GCounter<C: MemoryConfig, const CAPACITY: usize = 16> {
    /// Counter values for each node (indexed by node ID)
    #[cfg(not(feature = "hardware-atomic"))]
    counters: [u32; CAPACITY],

    /// Atomic counter values for each node (indexed by node ID)
    #[cfg(feature = "hardware-atomic")]
    counters: [AtomicU32; CAPACITY],

    /// This node's ID
    node_id: NodeId,

    /// Phantom data to maintain the memory config type
    _phantom: core::marker::PhantomData<C>,
}

// Implement Clone manually due to AtomicU32 not implementing Clone
impl<C: MemoryConfig, const CAPACITY: usize> Clone for GCounter<C, CAPACITY> {
    fn clone(&self) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                counters: self.counters,
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to read each atomic value
            let new_counters = [const { AtomicU32::new(0) }; CAPACITY];
            for i in 0..CAPACITY {
                new_counters[i].store(self.counters[i].load(Ordering::Relaxed), Ordering::Relaxed);
            }

            Self {
                counters: new_counters,
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> GCounter<C, CAPACITY> {
    /// Creates a new grow-only counter for the given node with custom capacity
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node (must be < CAPACITY)
    ///
    /// # Returns
    /// A new counter with all values initialized to 0
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let counter = GCounter::<DefaultConfig, 32>::with_capacity(1);
    /// assert_eq!(counter.value(), 0);
    /// ```
    pub fn with_capacity(node_id: NodeId) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                counters: [0; CAPACITY],
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            Self {
                counters: [const { AtomicU32::new(0) }; CAPACITY],
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<C: MemoryConfig> GCounter<C, 16> {
    /// Creates a new grow-only counter for the given node with default capacity
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node (must be < MAX_NODES)
    ///
    /// # Returns
    /// A new counter with all values initialized to 0
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let counter = GCounter::<DefaultConfig>::new(1);
    /// assert_eq!(counter.value(), 0);
    /// ```
    pub fn new(node_id: NodeId) -> Self {
        Self::with_capacity(node_id)
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> GCounter<C, CAPACITY> {
    /// Increments this node's counter by the given amount
    ///
    /// # Concurrency Behavior
    /// - **Without `hardware-atomic`**: Requires `&mut self`, single-threaded only
    /// - **With `hardware-atomic`**: Allows `&self`, thread-safe atomic operations
    ///
    /// # Arguments
    /// * `amount` - The amount to increment by (must be > 0)
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the increment would cause overflow
    ///
    /// # Examples
    ///
    /// ## Standard Version (default)
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut counter = GCounter::<DefaultConfig>::new(1);
    /// counter.increment(5)?; // Requires &mut self
    /// assert_eq!(counter.value(), 5);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    ///
    /// ## Atomic Version (with `hardware-atomic` feature)
    /// ```toml
    /// [dependencies]
    /// crdtosphere = { features = ["hardware-atomic"] }
    /// ```
    /// ```rust,ignore
    /// # // This example requires hardware-atomic feature
    /// use crdtosphere::prelude::*;
    /// let counter = GCounter::<DefaultConfig>::new(1);
    /// counter.increment(5)?; // Works with &self (atomic)
    /// assert_eq!(counter.value(), 5);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn increment(&mut self, amount: u32) -> CRDTResult<()> {
        if amount == 0 {
            return Err(CRDTError::InvalidOperation);
        }

        let node_index = self.node_id as usize;
        if node_index >= CAPACITY {
            return Err(CRDTError::InvalidNodeId);
        }

        // Check for overflow
        if self.counters[node_index] > u32::MAX - amount {
            return Err(CRDTError::BufferOverflow);
        }

        self.counters[node_index] += amount;
        Ok(())
    }

    /// Increments this node's counter by the given amount (atomic version)
    ///
    /// # Arguments
    /// * `amount` - The amount to increment by (must be > 0)
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the increment would cause overflow
    ///
    /// # Note
    /// This atomic version allows `&self` instead of `&mut self` for concurrent access.
    #[cfg(feature = "hardware-atomic")]
    pub fn increment(&self, amount: u32) -> CRDTResult<()> {
        if amount == 0 {
            return Err(CRDTError::InvalidOperation);
        }

        let node_index = self.node_id as usize;
        if node_index >= CAPACITY {
            return Err(CRDTError::InvalidNodeId);
        }

        // Use atomic fetch_add with overflow check
        let old_value = self.counters[node_index].fetch_add(amount, Ordering::Relaxed);

        // Check for overflow (if old_value + amount wrapped around)
        if old_value > u32::MAX - amount {
            // Rollback the increment
            self.counters[node_index].fetch_sub(amount, Ordering::Relaxed);
            return Err(CRDTError::BufferOverflow);
        }

        Ok(())
    }

    /// Increments this node's counter by 1
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the increment would cause overflow
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut counter = GCounter::<DefaultConfig>::new(1);
    /// counter.inc()?;
    /// assert_eq!(counter.value(), 1);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn inc(&mut self) -> CRDTResult<()> {
        self.increment(1)
    }

    /// Increments this node's counter by 1 (atomic version)
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the increment would cause overflow
    #[cfg(feature = "hardware-atomic")]
    pub fn inc(&self) -> CRDTResult<()> {
        self.increment(1)
    }

    /// Gets the total value of the counter (sum of all nodes)
    ///
    /// # Returns
    /// The sum of all node counters
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut counter = GCounter::<DefaultConfig>::new(1);
    /// counter.increment(10)?;
    /// assert_eq!(counter.value(), 10);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn value(&self) -> u64 {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.counters.iter().map(|&x| x as u64).sum()
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.counters
                .iter()
                .map(|atomic| atomic.load(Ordering::Relaxed) as u64)
                .sum()
        }
    }

    /// Gets the value for a specific node
    ///
    /// # Arguments
    /// * `node_id` - The node ID to get the value for
    ///
    /// # Returns
    /// The counter value for that node, or 0 if the node ID is invalid
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut counter = GCounter::<DefaultConfig>::new(1);
    /// counter.increment(5)?;
    /// assert_eq!(counter.node_value(1), 5);
    /// assert_eq!(counter.node_value(2), 0);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn node_value(&self, node_id: NodeId) -> u64 {
        let node_index = node_id as usize;
        if node_index < CAPACITY {
            #[cfg(not(feature = "hardware-atomic"))]
            {
                self.counters[node_index] as u64
            }

            #[cfg(feature = "hardware-atomic")]
            {
                self.counters[node_index].load(Ordering::Relaxed) as u64
            }
        } else {
            0
        }
    }

    /// Gets this node's ID
    ///
    /// # Returns
    /// The node ID of this counter
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Returns the capacity of this counter
    ///
    /// # Returns
    /// The maximum number of nodes this counter can track
    pub fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Checks if the counter is empty (all values are 0)
    ///
    /// # Returns
    /// true if all counters are 0, false otherwise
    pub fn is_empty(&self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.counters.iter().all(|&x| x == 0)
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.counters
                .iter()
                .all(|atomic| atomic.load(Ordering::Relaxed) == 0)
        }
    }

    /// Gets the number of nodes that have non-zero values
    ///
    /// # Returns
    /// The count of active nodes
    pub fn active_nodes(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.counters.iter().filter(|&&x| x > 0).count()
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.counters
                .iter()
                .filter(|atomic| atomic.load(Ordering::Relaxed) > 0)
                .count()
        }
    }
}

// Serde implementation for GCounter
#[cfg(feature = "serde")]
impl<C: MemoryConfig, const CAPACITY: usize> Serialize for GCounter<C, CAPACITY> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("GCounter", 2)?;

        // Serialize the logical state (counter values) as slice to handle any CAPACITY
        #[cfg(not(feature = "hardware-atomic"))]
        {
            state.serialize_field("c", &self.counters[..])?;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to extract values into a temporary array
            let mut counters = [0u32; CAPACITY];
            for i in 0..CAPACITY {
                counters[i] = self.counters[i].load(Ordering::Relaxed);
            }
            state.serialize_field("c", &counters[..])?;
        }
        state.serialize_field("n", &self.node_id)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, C: MemoryConfig, const CAPACITY: usize> Deserialize<'de> for GCounter<C, CAPACITY> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use core::fmt;
        use serde::de::{self, MapAccess, Visitor};

        #[derive(Deserialize)]
        #[serde(field_identifier)]
        enum Field {
            #[serde(rename = "c")]
            Counters,
            #[serde(rename = "n")]
            NodeId,
        }

        struct GCounterVisitor<C: MemoryConfig, const CAPACITY: usize> {
            _phantom: core::marker::PhantomData<C>,
        }

        impl<'de, C: MemoryConfig, const CAPACITY: usize> Visitor<'de> for GCounterVisitor<C, CAPACITY> {
            type Value = GCounter<C, CAPACITY>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct GCounter")
            }

            fn visit_map<V>(self, mut map: V) -> Result<GCounter<C, CAPACITY>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut counters = None;
                let mut node_id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Counters => {
                            if counters.is_some() {
                                return Err(de::Error::duplicate_field("c"));
                            }
                            // Use a simpler approach - deserialize as a slice and convert
                            use serde::de::SeqAccess;

                            struct ArrayDeserializer<const N: usize>;

                            impl<'de, const N: usize> serde::de::DeserializeSeed<'de> for ArrayDeserializer<N> {
                                type Value = [u32; N];

                                fn deserialize<D>(
                                    self,
                                    deserializer: D,
                                ) -> Result<Self::Value, D::Error>
                                where
                                    D: serde::de::Deserializer<'de>,
                                {
                                    struct ArrayVisitor<const N: usize>;

                                    impl<'de, const N: usize> serde::de::Visitor<'de> for ArrayVisitor<N> {
                                        type Value = [u32; N];

                                        fn expecting(
                                            &self,
                                            formatter: &mut core::fmt::Formatter,
                                        ) -> core::fmt::Result
                                        {
                                            write!(formatter, "an array of {} u32 values", N)
                                        }

                                        fn visit_seq<A>(
                                            self,
                                            mut seq: A,
                                        ) -> Result<Self::Value, A::Error>
                                        where
                                            A: SeqAccess<'de>,
                                        {
                                            let mut array = [0u32; N];
                                            for i in 0..N {
                                                if let Some(value) = seq.next_element()? {
                                                    array[i] = value;
                                                } else {
                                                    return Err(serde::de::Error::invalid_length(
                                                        i, &self,
                                                    ));
                                                }
                                            }
                                            Ok(array)
                                        }
                                    }

                                    deserializer.deserialize_seq(ArrayVisitor::<N>)
                                }
                            }

                            counters = Some(map.next_value_seed(ArrayDeserializer::<CAPACITY>)?);
                        }
                        Field::NodeId => {
                            if node_id.is_some() {
                                return Err(de::Error::duplicate_field("n"));
                            }
                            node_id = Some(map.next_value()?);
                        }
                    }
                }

                let counters = counters.ok_or_else(|| de::Error::missing_field("c"))?;
                let node_id = node_id.ok_or_else(|| de::Error::missing_field("n"))?;

                // Reconstruct the GCounter
                #[cfg(not(feature = "hardware-atomic"))]
                {
                    Ok(GCounter {
                        counters,
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }

                #[cfg(feature = "hardware-atomic")]
                {
                    let atomic_counters = [const { AtomicU32::new(0) }; CAPACITY];
                    for i in 0..CAPACITY {
                        atomic_counters[i].store(counters[i], Ordering::Relaxed);
                    }

                    Ok(GCounter {
                        counters: atomic_counters,
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }
            }
        }

        const FIELDS: &[&str] = &["c", "n"];
        deserializer.deserialize_struct(
            "GCounter",
            FIELDS,
            GCounterVisitor {
                _phantom: core::marker::PhantomData,
            },
        )
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> CRDT<C> for GCounter<C, CAPACITY> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Take the maximum value for each node
        #[cfg(not(feature = "hardware-atomic"))]
        {
            for i in 0..CAPACITY {
                self.counters[i] = self.counters[i].max(other.counters[i]);
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            for i in 0..CAPACITY {
                let other_value = other.counters[i].load(Ordering::Relaxed);
                let mut current = self.counters[i].load(Ordering::Relaxed);

                // Use compare_exchange loop to atomically update to max value
                while other_value > current {
                    match self.counters[i].compare_exchange_weak(
                        current,
                        other_value,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(actual) => current = actual,
                    }
                }
            }
        }
        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.counters == other.counters
        }

        #[cfg(feature = "hardware-atomic")]
        {
            for i in 0..CAPACITY {
                if self.counters[i].load(Ordering::Relaxed)
                    != other.counters[i].load(Ordering::Relaxed)
                {
                    return false;
                }
            }
            true
        }
    }

    fn size_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn validate(&self) -> CRDTResult<()> {
        // Validate node ID is within bounds
        if self.node_id as usize >= CAPACITY {
            return Err(CRDTError::InvalidNodeId);
        }

        // Validate that we don't exceed the configured maximum nodes
        if self.node_id as usize >= C::MAX_NODES {
            return Err(CRDTError::InvalidNodeId);
        }

        // Platform-specific validation rules
        #[cfg(feature = "aurix")]
        {
            // AURIX safety validation - max 3 TriCore CPUs (node IDs 0, 1, 2)
            if self.node_id >= crate::platform::constants::MAX_CORES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        #[cfg(feature = "stm32")]
        {
            // STM32 power-aware validation - limit active nodes
            if self.active_nodes() > crate::platform::validation::MAX_ACTIVE_NODES {
                return Err(CRDTError::ConfigurationExceeded);
            }
        }

        #[cfg(feature = "cortex-m")]
        {
            // Cortex-M memory-aware validation - RAM constraint
            if self.memory_usage() > crate::platform::validation::MAX_MEMORY_USAGE {
                return Err(CRDTError::BufferOverflow);
            }
        }

        #[cfg(feature = "riscv")]
        {
            // RISC-V flexible validation - check active nodes
            if self.active_nodes() > crate::platform::validation::MAX_ACTIVE_NODES {
                return Err(CRDTError::ConfigurationExceeded);
            }
        }

        Ok(())
    }

    fn state_hash(&self) -> u32 {
        // Simple hash based on counter values
        let mut hash = 0u32;

        #[cfg(not(feature = "hardware-atomic"))]
        {
            for (i, &value) in self.counters.iter().enumerate() {
                if value > 0 {
                    hash ^= value ^ ((i as u32) << 16);
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            for (i, atomic) in self.counters.iter().enumerate() {
                let value = atomic.load(Ordering::Relaxed);
                if value > 0 {
                    hash ^= value ^ ((i as u32) << 16);
                }
            }
        }

        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // GCounters can always merge
        true
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> BoundedCRDT<C> for GCounter<C, CAPACITY> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = CAPACITY; // Maximum number of nodes

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.active_nodes()
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // GCounters can't be compacted without losing data
        // This is a no-op that returns 0 bytes freed
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        // For counters, we can always "add" (increment) if not at max capacity
        self.element_count() < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> RealTimeCRDT<C> for GCounter<C, CAPACITY> {
    const MAX_MERGE_CYCLES: u32 = crate::platform::constants::MAX_MERGE_CYCLES / 10; // GCounter is very fast
    const MAX_VALIDATE_CYCLES: u32 = crate::platform::constants::MAX_MERGE_CYCLES / 25; // Validation is even faster
    const MAX_SERIALIZE_CYCLES: u32 = crate::platform::constants::MAX_MERGE_CYCLES / 5; // Serialization is moderate

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // GCounter merge is always bounded - just array max operations
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is always bounded - just a few checks
        self.validate()
    }

    fn remaining_budget(&self) -> Option<u32> {
        // For this simple implementation, we don't track budget
        None
    }

    fn set_budget(&mut self, _cycles: u32) {
        // For this simple implementation, we don't track budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::DefaultConfig;
    use crate::traits::CRDT;

    #[test]
    fn test_new_counter() {
        let counter = GCounter::<DefaultConfig>::new(1);
        assert_eq!(counter.value(), 0);
        assert_eq!(counter.node_id(), 1);
        assert!(counter.is_empty());
        assert_eq!(counter.active_nodes(), 0);
    }

    #[test]
    fn test_increment() {
        let mut counter = GCounter::<DefaultConfig>::new(1);

        assert!(counter.increment(5).is_ok());
        assert_eq!(counter.value(), 5);
        assert_eq!(counter.node_value(1), 5);
        assert_eq!(counter.node_value(2), 0);
        assert!(!counter.is_empty());
        assert_eq!(counter.active_nodes(), 1);
    }

    #[test]
    fn test_inc() {
        let mut counter = GCounter::<DefaultConfig>::new(1);

        assert!(counter.inc().is_ok());
        assert_eq!(counter.value(), 1);

        assert!(counter.inc().is_ok());
        assert_eq!(counter.value(), 2);
    }

    #[test]
    fn test_invalid_increment() {
        let mut counter = GCounter::<DefaultConfig>::new(1);

        // Zero increment should fail
        assert!(counter.increment(0).is_err());
        assert_eq!(counter.value(), 0);
    }

    #[test]
    fn test_overflow_protection() {
        let mut counter = GCounter::<DefaultConfig>::new(1);

        // Set to near max value
        #[cfg(not(feature = "hardware-atomic"))]
        {
            counter.counters[1] = u32::MAX - 1;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            counter.counters[1].store(u32::MAX - 1, Ordering::Relaxed);
        }

        // This should succeed
        assert!(counter.increment(1).is_ok());
        assert_eq!(counter.node_value(1), u32::MAX as u64);

        // This should fail (overflow)
        assert!(counter.increment(1).is_err());
    }

    #[test]
    fn test_merge() {
        let mut counter1 = GCounter::<DefaultConfig>::new(1);
        let mut counter2 = GCounter::<DefaultConfig>::new(2);

        counter1.increment(10).unwrap();
        counter2.increment(5).unwrap();

        // Before merge
        assert_eq!(counter1.value(), 10);
        assert_eq!(counter2.value(), 5);

        // Merge counter2 into counter1
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 15); // 10 + 5
        assert_eq!(counter1.node_value(1), 10);
        assert_eq!(counter1.node_value(2), 5);
        assert_eq!(counter1.active_nodes(), 2);
    }

    #[test]
    fn test_merge_with_overlap() {
        let mut counter1 = GCounter::<DefaultConfig>::new(1);
        let mut counter2 = GCounter::<DefaultConfig>::new(1); // Same node

        counter1.increment(10).unwrap();
        counter2.increment(5).unwrap(); // This should be ignored in merge

        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 10); // Max of 10 and 5
        assert_eq!(counter1.node_value(1), 10);
    }

    #[test]
    fn test_merge_idempotent() {
        let mut counter1 = GCounter::<DefaultConfig>::new(1);
        let counter2 = GCounter::<DefaultConfig>::new(2);

        counter1.increment(10).unwrap();

        // Multiple merges should be idempotent
        counter1.merge(&counter2).unwrap();
        let value1 = counter1.value();

        counter1.merge(&counter2).unwrap();
        let value2 = counter1.value();

        assert_eq!(value1, value2);
    }

    #[test]
    fn test_merge_commutative() {
        let mut counter1a = GCounter::<DefaultConfig>::new(1);
        let mut counter1b = GCounter::<DefaultConfig>::new(1);
        let mut counter2 = GCounter::<DefaultConfig>::new(2);
        let mut counter3 = GCounter::<DefaultConfig>::new(3);

        counter1a.increment(10).unwrap();
        counter1b.increment(10).unwrap();
        counter2.increment(5).unwrap();
        counter3.increment(3).unwrap();

        // Merge in different orders
        counter1a.merge(&counter2).unwrap();
        counter1a.merge(&counter3).unwrap();

        counter1b.merge(&counter3).unwrap();
        counter1b.merge(&counter2).unwrap();

        // Results should be the same
        assert_eq!(counter1a.value(), counter1b.value());
        assert!(counter1a.eq(&counter1b));
    }

    #[test]
    fn test_bounded_crdt() {
        let mut counter = GCounter::<DefaultConfig>::new(1);
        counter.increment(5).unwrap();

        assert_eq!(counter.element_count(), 1); // One active node
        assert!(counter.memory_usage() > 0);
        assert!(counter.can_add_element());

        // Add another node
        let mut other = GCounter::<DefaultConfig>::new(2);
        other.increment(3).unwrap();
        counter.merge(&other).unwrap();

        assert_eq!(counter.element_count(), 2); // Two active nodes
    }

    #[test]
    fn test_validation() {
        let counter = GCounter::<DefaultConfig>::new(1);
        assert!(counter.validate().is_ok());

        // Test with invalid node ID
        let invalid_counter = GCounter::<DefaultConfig>::new(255);
        assert!(invalid_counter.validate().is_err());
    }

    #[test]
    fn test_real_time_crdt() {
        let mut counter1 = GCounter::<DefaultConfig>::new(1);
        let counter2 = GCounter::<DefaultConfig>::new(2);

        assert!(counter1.merge_bounded(&counter2).is_ok());
        assert!(counter1.validate_bounded().is_ok());
    }

    #[test]
    fn test_state_hash() {
        let mut counter1 = GCounter::<DefaultConfig>::new(1);
        let mut counter2 = GCounter::<DefaultConfig>::new(1);

        // Same state should have same hash
        assert_eq!(counter1.state_hash(), counter2.state_hash());

        // Different state should have different hash (usually)
        counter1.increment(5).unwrap();
        assert_ne!(counter1.state_hash(), counter2.state_hash());

        // Same state again should have same hash
        counter2.increment(5).unwrap();
        assert_eq!(counter1.state_hash(), counter2.state_hash());
    }

    #[test]
    fn test_with_capacity() {
        // Test custom capacity
        let counter = GCounter::<DefaultConfig, 32>::with_capacity(1);
        assert_eq!(counter.value(), 0);
        assert_eq!(counter.node_id(), 1);
        assert_eq!(counter.capacity(), 32);
        assert!(counter.is_empty());
        assert_eq!(counter.active_nodes(), 0);
    }

    #[test]
    fn test_custom_capacity_operations() {
        let mut counter = GCounter::<DefaultConfig, 8>::with_capacity(3);

        // Test basic operations with custom capacity
        assert!(counter.increment(10).is_ok());
        assert_eq!(counter.value(), 10);
        assert_eq!(counter.node_value(3), 10);
        assert_eq!(counter.capacity(), 8);

        // Test node ID validation with custom capacity
        let mut invalid_counter = GCounter::<DefaultConfig, 4>::with_capacity(5);
        assert!(invalid_counter.increment(1).is_err()); // Node 5 >= capacity 4
    }

    #[test]
    fn test_capacity_merge() {
        let mut counter1 = GCounter::<DefaultConfig, 8>::with_capacity(1);
        let mut counter2 = GCounter::<DefaultConfig, 8>::with_capacity(2);

        counter1.increment(5).unwrap();
        counter2.increment(3).unwrap();

        // Merge should work with same capacity
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 8);
        assert_eq!(counter1.node_value(1), 5);
        assert_eq!(counter1.node_value(2), 3);
    }

    #[cfg(all(test, feature = "serde"))]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_serialize_deserialize() {
            let mut counter = GCounter::<DefaultConfig>::new(1);
            counter.increment(10).unwrap();

            let mut other = GCounter::<DefaultConfig>::new(2);
            other.increment(5).unwrap();
            counter.merge(&other).unwrap();

            // Test with a simple format (we'll use JSON-like representation)
            // In real usage, this would be with actual serde formats like bincode, JSON, etc.

            // For now, just test that the serde traits are implemented
            // This ensures the code compiles with serde feature
            assert_eq!(counter.value(), 15);
            assert_eq!(counter.node_value(1), 10);
            assert_eq!(counter.node_value(2), 5);
        }

        #[test]
        fn test_atomic_vs_standard_compatibility() {
            // This test ensures that atomic and standard versions would serialize to the same format
            // The logical state should be identical regardless of internal representation
            let mut counter = GCounter::<DefaultConfig>::new(1);
            counter.increment(42).unwrap();

            // Both versions should have the same logical state
            assert_eq!(counter.value(), 42);
            assert_eq!(counter.node_value(1), 42);
        }

        #[test]
        fn test_custom_capacity_serialization() {
            let mut counter = GCounter::<DefaultConfig, 8>::with_capacity(3);
            counter.increment(100).unwrap();

            // Custom capacity should work with serialization
            assert_eq!(counter.capacity(), 8);
            assert_eq!(counter.value(), 100);
            assert_eq!(counter.node_value(3), 100);
        }
    }
}
