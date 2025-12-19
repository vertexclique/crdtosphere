//! Increment/Decrement Counter CRDT
//!
//! A counter that can be both incremented and decremented.
//! Uses zero allocation with two fixed arrays for deterministic memory usage.
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

/// Increment/Decrement Counter with configurable node arrays
///
/// This counter supports both increment and decrement operations and provides
/// eventual consistency across multiple nodes. Each node maintains separate
/// positive and negative counter values.
///
/// # Type Parameters
/// - `C`: Memory configuration that determines the default maximum number of nodes
/// - `CAPACITY`: The maximum number of nodes this counter can track (defaults to 16)
///
/// # Memory Usage
/// - Fixed size: 8 * CAPACITY + 8 bytes (2x GCounter)
/// - Example: For 16 nodes = 136 bytes, for 64 nodes = 520 bytes
/// - Completely predictable at compile time
///
/// # Feature Comparison
///
/// | Method | Standard | Atomic | Mutability | Thread Safety | Notes |
/// |--------|----------|--------|------------|---------------|-------|
/// | `increment()` | ✅ | ✅ | `&mut self` / `&self` | Single / Multi | Positive increment |
/// | `decrement()` | ✅ | ✅ | `&mut self` / `&self` | Single / Multi | Negative increment |
/// | `inc()` | ✅ | ✅ | `&mut self` / `&self` | Single / Multi | Convenience +1 |
/// | `dec()` | ✅ | ✅ | `&mut self` / `&self` | Single / Multi | Convenience -1 |
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
/// let mut counter1 = PNCounter::<DefaultConfig>::new(1);
/// counter1.increment(10)?;
/// counter1.decrement(3)?;
///
/// let mut counter2 = PNCounter::<DefaultConfig>::new(2);
/// counter2.increment(5)?;
///
/// // Merge the counters
/// counter1.merge(&counter2)?;
/// assert_eq!(counter1.value(), 12); // (10-3) + 5 = 12
///
/// // Example with custom capacity (separate from merging)
/// let counter3 = PNCounter::<DefaultConfig, 32>::with_capacity(3);
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
pub struct PNCounter<C: MemoryConfig, const CAPACITY: usize = 16> {
    /// Positive increments for each node (indexed by node ID)
    #[cfg(not(feature = "hardware-atomic"))]
    positive: [u32; CAPACITY],

    /// Negative increments for each node (indexed by node ID)
    #[cfg(not(feature = "hardware-atomic"))]
    negative: [u32; CAPACITY],

    /// Atomic positive increments for each node (indexed by node ID)
    #[cfg(feature = "hardware-atomic")]
    positive: [AtomicU32; CAPACITY],

    /// Atomic negative increments for each node (indexed by node ID)
    #[cfg(feature = "hardware-atomic")]
    negative: [AtomicU32; CAPACITY],

    /// This node's ID
    node_id: NodeId,

    /// Phantom data to maintain the memory config type
    _phantom: core::marker::PhantomData<C>,
}

// Implement Clone manually due to AtomicU32 not implementing Clone
impl<C: MemoryConfig, const CAPACITY: usize> Clone for PNCounter<C, CAPACITY> {
    fn clone(&self) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                positive: self.positive,
                negative: self.negative,
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to read each atomic value
            let new_positive = [const { AtomicU32::new(0) }; CAPACITY];
            let new_negative = [const { AtomicU32::new(0) }; CAPACITY];

            for i in 0..CAPACITY {
                new_positive[i].store(self.positive[i].load(Ordering::Relaxed), Ordering::Relaxed);
                new_negative[i].store(self.negative[i].load(Ordering::Relaxed), Ordering::Relaxed);
            }

            Self {
                positive: new_positive,
                negative: new_negative,
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> PNCounter<C, CAPACITY> {
    /// Creates a new increment/decrement counter for the given node with custom capacity
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
    /// let counter = PNCounter::<DefaultConfig, 32>::with_capacity(1);
    /// assert_eq!(counter.value(), 0);
    /// ```
    pub fn with_capacity(node_id: NodeId) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                positive: [0; CAPACITY],
                negative: [0; CAPACITY],
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            Self {
                positive: [const { AtomicU32::new(0) }; CAPACITY],
                negative: [const { AtomicU32::new(0) }; CAPACITY],
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<C: MemoryConfig> PNCounter<C, 16> {
    /// Creates a new increment/decrement counter for the given node with default capacity
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
    /// let counter = PNCounter::<DefaultConfig>::new(1);
    /// assert_eq!(counter.value(), 0);
    /// ```
    pub fn new(node_id: NodeId) -> Self {
        Self::with_capacity(node_id)
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> PNCounter<C, CAPACITY> {
    /// Increments this node's counter by the given amount
    ///
    /// # Arguments
    /// * `amount` - The amount to increment by (must be > 0)
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the increment would cause overflow
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut counter = PNCounter::<DefaultConfig>::new(1);
    /// counter.increment(5)?;
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
        if self.positive[node_index] > u32::MAX - amount {
            return Err(CRDTError::BufferOverflow);
        }

        self.positive[node_index] += amount;
        Ok(())
    }

    /// Increments this node's counter by the given amount (atomic version)
    ///
    /// # Arguments
    /// * `amount` - The amount to increment by (must be > 0)
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the increment would cause overflow
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
        let old_value = self.positive[node_index].fetch_add(amount, Ordering::Relaxed);

        // Check for overflow (if old_value + amount wrapped around)
        if old_value > u32::MAX - amount {
            // Rollback the increment
            self.positive[node_index].fetch_sub(amount, Ordering::Relaxed);
            return Err(CRDTError::BufferOverflow);
        }

        Ok(())
    }

    /// Decrements this node's counter by the given amount
    ///
    /// # Arguments
    /// * `amount` - The amount to decrement by (must be > 0)
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the decrement would cause overflow
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut counter = PNCounter::<DefaultConfig>::new(1);
    /// counter.increment(10)?;
    /// counter.decrement(3)?;
    /// assert_eq!(counter.value(), 7);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn decrement(&mut self, amount: u32) -> CRDTResult<()> {
        if amount == 0 {
            return Err(CRDTError::InvalidOperation);
        }

        let node_index = self.node_id as usize;
        if node_index >= CAPACITY {
            return Err(CRDTError::InvalidNodeId);
        }

        // Check for overflow
        if self.negative[node_index] > u32::MAX - amount {
            return Err(CRDTError::BufferOverflow);
        }

        self.negative[node_index] += amount;
        Ok(())
    }

    /// Decrements this node's counter by the given amount (atomic version)
    ///
    /// # Arguments
    /// * `amount` - The amount to decrement by (must be > 0)
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the decrement would cause overflow
    #[cfg(feature = "hardware-atomic")]
    pub fn decrement(&self, amount: u32) -> CRDTResult<()> {
        if amount == 0 {
            return Err(CRDTError::InvalidOperation);
        }

        let node_index = self.node_id as usize;
        if node_index >= CAPACITY {
            return Err(CRDTError::InvalidNodeId);
        }

        // Use atomic fetch_add with overflow check
        let old_value = self.negative[node_index].fetch_add(amount, Ordering::Relaxed);

        // Check for overflow (if old_value + amount wrapped around)
        if old_value > u32::MAX - amount {
            // Rollback the increment
            self.negative[node_index].fetch_sub(amount, Ordering::Relaxed);
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
    /// let mut counter = PNCounter::<DefaultConfig>::new(1);
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

    /// Decrements this node's counter by 1
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the decrement would cause overflow
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut counter = PNCounter::<DefaultConfig>::new(1);
    /// counter.inc()?;
    /// counter.dec()?;
    /// assert_eq!(counter.value(), 0);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn dec(&mut self) -> CRDTResult<()> {
        self.decrement(1)
    }

    /// Decrements this node's counter by 1 (atomic version)
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the decrement would cause overflow
    #[cfg(feature = "hardware-atomic")]
    pub fn dec(&self) -> CRDTResult<()> {
        self.decrement(1)
    }

    /// Gets the total value of the counter (positive - negative)
    ///
    /// # Returns
    /// The net value of the counter (sum of positive - sum of negative)
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut counter = PNCounter::<DefaultConfig>::new(1);
    /// counter.increment(10)?;
    /// counter.decrement(3)?;
    /// assert_eq!(counter.value(), 7);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn value(&self) -> i64 {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            let positive_sum: u64 = self.positive.iter().map(|&x| x as u64).sum();
            let negative_sum: u64 = self.negative.iter().map(|&x| x as u64).sum();

            // Handle potential overflow by using saturating arithmetic
            if positive_sum >= negative_sum {
                (positive_sum - negative_sum) as i64
            } else {
                -((negative_sum - positive_sum) as i64)
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let positive_sum: u64 = self
                .positive
                .iter()
                .map(|atomic| atomic.load(Ordering::Relaxed) as u64)
                .sum();
            let negative_sum: u64 = self
                .negative
                .iter()
                .map(|atomic| atomic.load(Ordering::Relaxed) as u64)
                .sum();

            // Handle potential overflow by using saturating arithmetic
            if positive_sum >= negative_sum {
                (positive_sum - negative_sum) as i64
            } else {
                -((negative_sum - positive_sum) as i64)
            }
        }
    }

    /// Gets the positive value for a specific node
    ///
    /// # Arguments
    /// * `node_id` - The node ID to get the positive value for
    ///
    /// # Returns
    /// The positive counter value for that node, or 0 if the node ID is invalid
    pub fn node_positive(&self, node_id: NodeId) -> u64 {
        let node_index = node_id as usize;
        if node_index < CAPACITY {
            #[cfg(not(feature = "hardware-atomic"))]
            {
                self.positive[node_index] as u64
            }

            #[cfg(feature = "hardware-atomic")]
            {
                self.positive[node_index].load(Ordering::Relaxed) as u64
            }
        } else {
            0
        }
    }

    /// Gets the negative value for a specific node
    ///
    /// # Arguments
    /// * `node_id` - The node ID to get the negative value for
    ///
    /// # Returns
    /// The negative counter value for that node, or 0 if the node ID is invalid
    pub fn node_negative(&self, node_id: NodeId) -> u64 {
        let node_index = node_id as usize;
        if node_index < CAPACITY {
            #[cfg(not(feature = "hardware-atomic"))]
            {
                self.negative[node_index] as u64
            }

            #[cfg(feature = "hardware-atomic")]
            {
                self.negative[node_index].load(Ordering::Relaxed) as u64
            }
        } else {
            0
        }
    }

    /// Gets the net value for a specific node
    ///
    /// # Arguments
    /// * `node_id` - The node ID to get the net value for
    ///
    /// # Returns
    /// The net value (positive - negative) for that node
    pub fn node_value(&self, node_id: NodeId) -> i64 {
        let positive = self.node_positive(node_id) as i64;
        let negative = self.node_negative(node_id) as i64;
        positive - negative
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

    /// Gets the positive value for all nodes as an array
    ///
    /// # Returns
    /// An array of positive counter values (snapshot for atomic version)
    pub fn positive_counters(&self) -> [u64; CAPACITY] {
        let mut result = [0u64; CAPACITY];

        #[cfg(not(feature = "hardware-atomic"))]
        {
            for i in 0..CAPACITY {
                result[i] = self.positive[i] as u64;
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            for i in 0..CAPACITY {
                result[i] = self.positive[i].load(Ordering::Relaxed) as u64;
            }
        }

        result
    }

    /// Gets the negative value for all nodes as an array
    ///
    /// # Returns
    /// An array of negative counter values (snapshot for atomic version)
    pub fn negative_counters(&self) -> [u64; CAPACITY] {
        let mut result = [0u64; CAPACITY];

        #[cfg(not(feature = "hardware-atomic"))]
        {
            for i in 0..CAPACITY {
                result[i] = self.negative[i] as u64;
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            for i in 0..CAPACITY {
                result[i] = self.negative[i].load(Ordering::Relaxed) as u64;
            }
        }

        result
    }

    /// Checks if the counter is empty (all values are 0)
    ///
    /// # Returns
    /// true if all positive and negative counters are 0, false otherwise
    pub fn is_empty(&self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.positive.iter().all(|&x| x == 0) && self.negative.iter().all(|&x| x == 0)
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.positive
                .iter()
                .all(|atomic| atomic.load(Ordering::Relaxed) == 0)
                && self
                    .negative
                    .iter()
                    .all(|atomic| atomic.load(Ordering::Relaxed) == 0)
        }
    }

    /// Gets the number of nodes that have non-zero values
    ///
    /// # Returns
    /// The count of active nodes (nodes with non-zero positive or negative values)
    pub fn active_nodes(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            let mut active = 0;
            for i in 0..CAPACITY {
                if self.positive[i] > 0 || self.negative[i] > 0 {
                    active += 1;
                }
            }
            active
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let mut active = 0;
            for i in 0..CAPACITY {
                if self.positive[i].load(Ordering::Relaxed) > 0
                    || self.negative[i].load(Ordering::Relaxed) > 0
                {
                    active += 1;
                }
            }
            active
        }
    }

    /// Gets the total positive value across all nodes
    ///
    /// # Returns
    /// The sum of all positive increments
    pub fn total_positive(&self) -> u64 {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.positive.iter().map(|&x| x as u64).sum()
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.positive
                .iter()
                .map(|atomic| atomic.load(Ordering::Relaxed) as u64)
                .sum()
        }
    }

    /// Gets the total negative value across all nodes
    ///
    /// # Returns
    /// The sum of all negative increments
    pub fn total_negative(&self) -> u64 {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.negative.iter().map(|&x| x as u64).sum()
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.negative
                .iter()
                .map(|atomic| atomic.load(Ordering::Relaxed) as u64)
                .sum()
        }
    }
}

// Serde implementation for PNCounter
#[cfg(feature = "serde")]
impl<C: MemoryConfig> Serialize for PNCounter<C> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("PNCounter", 3)?;

        // Serialize the logical state (positive and negative counter values) as slices
        #[cfg(not(feature = "hardware-atomic"))]
        {
            state.serialize_field("p", &self.positive[..])?;
            state.serialize_field("ng", &self.negative[..])?;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to extract values into temporary arrays
            let mut positive = [0u32; 16];
            let mut negative = [0u32; 16];
            for i in 0..16 {
                positive[i] = self.positive[i].load(Ordering::Relaxed);
                negative[i] = self.negative[i].load(Ordering::Relaxed);
            }
            state.serialize_field("p", &positive[..])?;
            state.serialize_field("ng", &negative[..])?;
        }

        state.serialize_field("n", &self.node_id)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, C: MemoryConfig> Deserialize<'de> for PNCounter<C> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use core::fmt;
        use serde::de::{self, MapAccess, Visitor};

        #[derive(Deserialize)]
        #[serde(field_identifier)]
        enum Field {
            #[serde(rename = "p")]
            Positive,
            #[serde(rename = "ng")]
            Negative,
            #[serde(rename = "n")]
            NodeId,
        }

        struct PNCounterVisitor<C: MemoryConfig> {
            _phantom: core::marker::PhantomData<C>,
        }

        impl<'de, C: MemoryConfig> Visitor<'de> for PNCounterVisitor<C> {
            type Value = PNCounter<C>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct PNCounter")
            }

            fn visit_map<V>(self, mut map: V) -> Result<PNCounter<C>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut positive = None;
                let mut negative = None;
                let mut node_id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Positive => {
                            if positive.is_some() {
                                return Err(de::Error::duplicate_field("p"));
                            }
                            // Use custom array deserializer for no_std compatibility
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

                            positive = Some(map.next_value_seed(ArrayDeserializer::<16>)?);
                        }
                        Field::Negative => {
                            if negative.is_some() {
                                return Err(de::Error::duplicate_field("ng"));
                            }
                            // Use the same custom array deserializer
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

                            negative = Some(map.next_value_seed(ArrayDeserializer::<16>)?);
                        }
                        Field::NodeId => {
                            if node_id.is_some() {
                                return Err(de::Error::duplicate_field("n"));
                            }
                            node_id = Some(map.next_value()?);
                        }
                    }
                }

                let positive = positive.ok_or_else(|| de::Error::missing_field("p"))?;
                let negative = negative.ok_or_else(|| de::Error::missing_field("ng"))?;
                let node_id = node_id.ok_or_else(|| de::Error::missing_field("n"))?;

                // Reconstruct the PNCounter
                #[cfg(not(feature = "hardware-atomic"))]
                {
                    Ok(PNCounter {
                        positive,
                        negative,
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }

                #[cfg(feature = "hardware-atomic")]
                {
                    let atomic_positive = [const { AtomicU32::new(0) }; 16];
                    let atomic_negative = [const { AtomicU32::new(0) }; 16];
                    for i in 0..16 {
                        atomic_positive[i].store(positive[i], Ordering::Relaxed);
                        atomic_negative[i].store(negative[i], Ordering::Relaxed);
                    }

                    Ok(PNCounter {
                        positive: atomic_positive,
                        negative: atomic_negative,
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }
            }
        }

        const FIELDS: &[&str] = &["p", "ng", "n"];
        deserializer.deserialize_struct(
            "PNCounter",
            FIELDS,
            PNCounterVisitor {
                _phantom: core::marker::PhantomData,
            },
        )
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> CRDT<C> for PNCounter<C, CAPACITY> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Take the maximum value for each node in both arrays
        #[cfg(not(feature = "hardware-atomic"))]
        {
            for i in 0..CAPACITY {
                self.positive[i] = self.positive[i].max(other.positive[i]);
                self.negative[i] = self.negative[i].max(other.negative[i]);
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            for i in 0..CAPACITY {
                // Handle positive array
                let other_pos_value = other.positive[i].load(Ordering::Relaxed);
                let mut current_pos = self.positive[i].load(Ordering::Relaxed);

                while other_pos_value > current_pos {
                    match self.positive[i].compare_exchange_weak(
                        current_pos,
                        other_pos_value,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(actual) => current_pos = actual,
                    }
                }

                // Handle negative array
                let other_neg_value = other.negative[i].load(Ordering::Relaxed);
                let mut current_neg = self.negative[i].load(Ordering::Relaxed);

                while other_neg_value > current_neg {
                    match self.negative[i].compare_exchange_weak(
                        current_neg,
                        other_neg_value,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(actual) => current_neg = actual,
                    }
                }
            }
        }
        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.positive == other.positive && self.negative == other.negative
        }

        #[cfg(feature = "hardware-atomic")]
        {
            for i in 0..CAPACITY {
                if self.positive[i].load(Ordering::Relaxed)
                    != other.positive[i].load(Ordering::Relaxed)
                    || self.negative[i].load(Ordering::Relaxed)
                        != other.negative[i].load(Ordering::Relaxed)
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

        Ok(())
    }

    fn state_hash(&self) -> u32 {
        // Simple hash based on counter values
        let mut hash = 0u32;

        #[cfg(not(feature = "hardware-atomic"))]
        {
            for (i, (&pos, &neg)) in self.positive.iter().zip(self.negative.iter()).enumerate() {
                if pos > 0 || neg > 0 {
                    hash ^= pos ^ (neg << 8) ^ ((i as u32) << 16);
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            for (i, (pos_atomic, neg_atomic)) in
                self.positive.iter().zip(self.negative.iter()).enumerate()
            {
                let pos = pos_atomic.load(Ordering::Relaxed);
                let neg = neg_atomic.load(Ordering::Relaxed);
                if pos > 0 || neg > 0 {
                    hash ^= pos ^ (neg << 8) ^ ((i as u32) << 16);
                }
            }
        }

        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // PNCounters can always merge
        true
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> BoundedCRDT<C> for PNCounter<C, CAPACITY> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = CAPACITY; // Maximum number of nodes

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.active_nodes()
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // PNCounters can't be compacted without losing data
        // This is a no-op that returns 0 bytes freed
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        // For counters, we can always "add" (increment/decrement) if not at max capacity
        self.element_count() < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig, const CAPACITY: usize> RealTimeCRDT<C> for PNCounter<C, CAPACITY> {
    const MAX_MERGE_CYCLES: u32 = 100; // Slightly more than GCounter due to two arrays
    const MAX_VALIDATE_CYCLES: u32 = 30;
    const MAX_SERIALIZE_CYCLES: u32 = 150;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // PNCounter merge is always bounded - just array max operations on two arrays
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

    #[test]
    fn test_new_counter() {
        let counter = PNCounter::<DefaultConfig>::new(1);
        assert_eq!(counter.value(), 0);
        assert_eq!(counter.node_id(), 1);
        assert!(counter.is_empty());
        assert_eq!(counter.active_nodes(), 0);
        assert_eq!(counter.total_positive(), 0);
        assert_eq!(counter.total_negative(), 0);
    }

    #[test]
    fn test_increment() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        assert!(counter.increment(5).is_ok());
        assert_eq!(counter.value(), 5);
        assert_eq!(counter.node_positive(1), 5);
        assert_eq!(counter.node_negative(1), 0);
        assert_eq!(counter.node_value(1), 5);
        assert!(!counter.is_empty());
        assert_eq!(counter.active_nodes(), 1);
    }

    #[test]
    fn test_decrement() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        assert!(counter.decrement(3).is_ok());
        assert_eq!(counter.value(), -3);
        assert_eq!(counter.node_positive(1), 0);
        assert_eq!(counter.node_negative(1), 3);
        assert_eq!(counter.node_value(1), -3);
        assert!(!counter.is_empty());
        assert_eq!(counter.active_nodes(), 1);
    }

    #[test]
    fn test_inc_dec() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        assert!(counter.inc().is_ok());
        assert_eq!(counter.value(), 1);

        assert!(counter.dec().is_ok());
        assert_eq!(counter.value(), 0);

        assert!(counter.dec().is_ok());
        assert_eq!(counter.value(), -1);
    }

    #[test]
    fn test_mixed_operations() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        counter.increment(10).unwrap();
        counter.decrement(3).unwrap();
        counter.increment(2).unwrap();
        counter.decrement(1).unwrap();

        assert_eq!(counter.value(), 8); // (10+2) - (3+1) = 8
        assert_eq!(counter.node_positive(1), 12);
        assert_eq!(counter.node_negative(1), 4);
        assert_eq!(counter.total_positive(), 12);
        assert_eq!(counter.total_negative(), 4);
    }

    #[test]
    fn test_invalid_operations() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        // Zero increment/decrement should fail
        assert!(counter.increment(0).is_err());
        assert!(counter.decrement(0).is_err());
        assert_eq!(counter.value(), 0);
    }

    #[test]
    fn test_overflow_protection() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        // Set to near max value
        #[cfg(not(feature = "hardware-atomic"))]
        {
            counter.positive[1] = u32::MAX - 1;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            counter.positive[1].store(u32::MAX - 1, Ordering::Relaxed);
        }

        // This should succeed
        assert!(counter.increment(1).is_ok());
        assert_eq!(counter.node_positive(1), u32::MAX as u64);

        // This should fail (overflow)
        assert!(counter.increment(1).is_err());

        // Test negative overflow
        #[cfg(not(feature = "hardware-atomic"))]
        {
            counter.negative[1] = u32::MAX - 1;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            counter.negative[1].store(u32::MAX - 1, Ordering::Relaxed);
        }

        assert!(counter.decrement(1).is_ok());
        assert_eq!(counter.node_negative(1), u32::MAX as u64);
        assert!(counter.decrement(1).is_err());
    }

    #[test]
    fn test_merge() {
        let mut counter1 = PNCounter::<DefaultConfig>::new(1);
        let mut counter2 = PNCounter::<DefaultConfig>::new(2);

        counter1.increment(10).unwrap();
        counter1.decrement(2).unwrap();

        counter2.increment(5).unwrap();
        counter2.decrement(1).unwrap();

        // Before merge
        assert_eq!(counter1.value(), 8); // 10 - 2
        assert_eq!(counter2.value(), 4); // 5 - 1

        // Merge counter2 into counter1
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 12); // (10+5) - (2+1) = 12
        assert_eq!(counter1.node_positive(1), 10);
        assert_eq!(counter1.node_negative(1), 2);
        assert_eq!(counter1.node_positive(2), 5);
        assert_eq!(counter1.node_negative(2), 1);
        assert_eq!(counter1.active_nodes(), 2);
    }

    #[test]
    fn test_merge_with_overlap() {
        let mut counter1 = PNCounter::<DefaultConfig>::new(1);
        let mut counter2 = PNCounter::<DefaultConfig>::new(1); // Same node

        counter1.increment(10).unwrap();
        counter1.decrement(2).unwrap();

        counter2.increment(5).unwrap(); // Should be ignored (10 > 5)
        counter2.decrement(3).unwrap(); // Should win (3 > 2)

        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 7); // 10 - 3 = 7
        assert_eq!(counter1.node_positive(1), 10); // max(10, 5)
        assert_eq!(counter1.node_negative(1), 3); // max(2, 3)
    }

    #[test]
    fn test_merge_commutative() {
        let mut counter1a = PNCounter::<DefaultConfig>::new(1);
        let mut counter1b = PNCounter::<DefaultConfig>::new(1);
        let mut counter2 = PNCounter::<DefaultConfig>::new(2);
        let mut counter3 = PNCounter::<DefaultConfig>::new(3);

        counter1a.increment(10).unwrap();
        counter1a.decrement(1).unwrap();
        counter1b.increment(10).unwrap();
        counter1b.decrement(1).unwrap();

        counter2.increment(5).unwrap();
        counter2.decrement(2).unwrap();

        counter3.increment(3).unwrap();
        counter3.decrement(1).unwrap();

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
        let mut counter = PNCounter::<DefaultConfig>::new(1);
        counter.increment(5).unwrap();
        counter.decrement(2).unwrap();

        assert_eq!(counter.element_count(), 1); // One active node
        assert!(counter.memory_usage() > 0);
        assert!(counter.can_add_element());

        // Add another node
        let mut other = PNCounter::<DefaultConfig>::new(2);
        other.increment(3).unwrap();
        counter.merge(&other).unwrap();

        assert_eq!(counter.element_count(), 2); // Two active nodes
    }

    #[test]
    fn test_validation() {
        let counter = PNCounter::<DefaultConfig>::new(1);
        assert!(counter.validate().is_ok());

        // Test with invalid node ID
        let invalid_counter = PNCounter::<DefaultConfig>::new(255);
        assert!(invalid_counter.validate().is_err());
    }

    #[test]
    fn test_real_time_crdt() {
        let mut counter1 = PNCounter::<DefaultConfig>::new(1);
        let counter2 = PNCounter::<DefaultConfig>::new(2);

        assert!(counter1.merge_bounded(&counter2).is_ok());
        assert!(counter1.validate_bounded().is_ok());
    }

    #[test]
    fn test_state_hash() {
        let mut counter1 = PNCounter::<DefaultConfig>::new(1);
        let mut counter2 = PNCounter::<DefaultConfig>::new(1);

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
    fn test_negative_values() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        // Start with negative value
        counter.decrement(10).unwrap();
        assert_eq!(counter.value(), -10);

        // Add some positive
        counter.increment(3).unwrap();
        assert_eq!(counter.value(), -7);

        // Make it positive
        counter.increment(10).unwrap();
        assert_eq!(counter.value(), 3);
    }

    #[test]
    fn test_with_capacity() {
        // Test custom capacity
        let counter = PNCounter::<DefaultConfig, 32>::with_capacity(1);
        assert_eq!(counter.value(), 0);
        assert_eq!(counter.node_id(), 1);
        assert_eq!(counter.capacity(), 32);
        assert!(counter.is_empty());
        assert_eq!(counter.active_nodes(), 0);
    }

    #[test]
    fn test_custom_capacity_operations() {
        let mut counter = PNCounter::<DefaultConfig, 8>::with_capacity(3);

        // Test basic operations with custom capacity
        assert!(counter.increment(10).is_ok());
        assert!(counter.decrement(3).is_ok());
        assert_eq!(counter.value(), 7);
        assert_eq!(counter.node_positive(3), 10);
        assert_eq!(counter.node_negative(3), 3);
        assert_eq!(counter.capacity(), 8);

        // Test node ID validation with custom capacity
        let mut invalid_counter = PNCounter::<DefaultConfig, 4>::with_capacity(5);
        assert!(invalid_counter.increment(1).is_err()); // Node 5 >= capacity 4
        assert!(invalid_counter.decrement(1).is_err()); // Node 5 >= capacity 4
    }

    #[test]
    fn test_capacity_merge() {
        let mut counter1 = PNCounter::<DefaultConfig, 8>::with_capacity(1);
        let mut counter2 = PNCounter::<DefaultConfig, 8>::with_capacity(2);

        counter1.increment(5).unwrap();
        counter1.decrement(1).unwrap();
        counter2.increment(3).unwrap();
        counter2.decrement(2).unwrap();

        // Merge should work with same capacity
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 5); // (5+3) - (1+2) = 5
        assert_eq!(counter1.node_positive(1), 5);
        assert_eq!(counter1.node_negative(1), 1);
        assert_eq!(counter1.node_positive(2), 3);
        assert_eq!(counter1.node_negative(2), 2);
    }

    #[cfg(all(test, feature = "serde"))]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_serialize_deserialize() {
            let mut counter = PNCounter::<DefaultConfig>::new(1);
            counter.increment(10).unwrap();
            counter.decrement(3).unwrap();

            let mut other = PNCounter::<DefaultConfig>::new(2);
            other.increment(5).unwrap();
            other.decrement(1).unwrap();
            counter.merge(&other).unwrap();

            // Test that the serde traits are implemented
            // This ensures the code compiles with serde feature
            assert_eq!(counter.value(), 11); // (10+5) - (3+1) = 11
            assert_eq!(counter.node_positive(1), 10);
            assert_eq!(counter.node_negative(1), 3);
            assert_eq!(counter.node_positive(2), 5);
            assert_eq!(counter.node_negative(2), 1);
        }

        #[test]
        fn test_atomic_vs_standard_compatibility() {
            // This test ensures that atomic and standard versions would serialize to the same format
            // The logical state should be identical regardless of internal representation
            let mut counter = PNCounter::<DefaultConfig>::new(1);
            counter.increment(42).unwrap();
            counter.decrement(7).unwrap();

            // Both versions should have the same logical state
            assert_eq!(counter.value(), 35);
            assert_eq!(counter.node_positive(1), 42);
            assert_eq!(counter.node_negative(1), 7);
        }

        #[test]
        fn test_negative_value_serialization() {
            let mut counter = PNCounter::<DefaultConfig>::new(1);
            counter.decrement(100).unwrap();
            counter.increment(30).unwrap();

            // Should handle negative values correctly
            assert_eq!(counter.value(), -70);
            assert_eq!(counter.node_positive(1), 30);
            assert_eq!(counter.node_negative(1), 100);
        }

        #[test]
        fn test_custom_capacity_serialization() {
            let mut counter = PNCounter::<DefaultConfig, 8>::with_capacity(1);
            counter.increment(100).unwrap();
            counter.decrement(25).unwrap();

            // Custom capacity should work with serialization
            assert_eq!(counter.capacity(), 8);
            assert_eq!(counter.value(), 75);
            assert_eq!(counter.node_positive(1), 100);
            assert_eq!(counter.node_negative(1), 25);
        }
    }
}
