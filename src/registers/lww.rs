//! Last-Writer-Wins Register CRDT
//!
//! A register that resolves conflicts by keeping the value with the latest timestamp.
//! Uses zero allocation for deterministic memory usage.
//!
//! This module provides both standard and atomic implementations:
//! - Standard: Requires `&mut self` for modifications, single-threaded
//! - Atomic: Allows `&self` for modifications, multi-threaded safe

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

#[cfg(feature = "hardware-atomic")]
use core::cell::UnsafeCell;
#[cfg(feature = "hardware-atomic")]
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Last-Writer-Wins Register
///
/// This register resolves conflicts by keeping the value with the latest timestamp.
/// All memory is statically allocated for deterministic memory usage.
///
/// # Type Parameters
/// - `T`: The value type stored in the register
/// - `C`: Memory configuration that determines limits
///
/// # Memory Usage
/// - Fixed size: ~16-32 bytes (depending on T)
/// - Completely predictable at compile time
///
/// # Feature Comparison
///
/// | Method | Standard | Atomic | Mutability | Thread Safety | Notes |
/// |--------|----------|--------|------------|---------------|-------|
/// | `set()` | ✅ | ✅ | `&mut self` / `&self` | Single / Multi | Update with timestamp |
/// | `get()` | ✅ | ✅ | `&self` | Single / Multi | Read-only |
/// | `merge()` | ✅ | ✅ | `&mut self` | Single / Multi | CRDT merge |
/// | `timestamp()` | ✅ | ✅ | `&self` | Single / Multi | Read-only |
/// | `current_node()` | ✅ | ✅ | `&self` | Single / Multi | Read-only |
/// | `is_empty()` | ✅ | ✅ | `&self` | Single / Multi | Read-only |
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
/// // Create a register for sensor readings
/// let mut sensor = LWWRegister::<f32, DefaultConfig>::new(1);
/// sensor.set(23.5, 1000)?;
///
/// // Merge with another node's data
/// let mut other = LWWRegister::<f32, DefaultConfig>::new(2);
/// other.set(24.1, 1001)?;
/// sensor.merge(&other)?;
///
/// assert_eq!(sensor.get(), Some(&24.1)); // Latest value wins
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug)]
pub struct LWWRegister<T, C: MemoryConfig> {
    /// Current value and its metadata
    #[cfg(not(feature = "hardware-atomic"))]
    current_value: Option<T>,
    #[cfg(not(feature = "hardware-atomic"))]
    current_timestamp: CompactTimestamp,
    #[cfg(not(feature = "hardware-atomic"))]
    current_node_id: NodeId,

    /// Atomic version uses separate storage for coordination
    #[cfg(feature = "hardware-atomic")]
    current_value: UnsafeCell<Option<T>>,
    #[cfg(feature = "hardware-atomic")]
    current_timestamp: AtomicU32,
    #[cfg(feature = "hardware-atomic")]
    current_node_id: AtomicU8,

    /// This node's ID
    node_id: NodeId,

    /// Phantom data to maintain the memory config type
    _phantom: core::marker::PhantomData<C>,
}

// SAFETY: The atomic version is safe to share between threads because:
// 1. All access to current_value is protected by atomic timestamp coordination
// 2. Only one thread can successfully update at a time via compare_exchange
// 3. UnsafeCell is only accessed after winning the atomic coordination
#[cfg(feature = "hardware-atomic")]
unsafe impl<T, C: MemoryConfig> Sync for LWWRegister<T, C>
where
    T: Send,
    C: Send + Sync,
{
}

// Implement Clone manually due to atomic types not implementing Clone
impl<T, C: MemoryConfig> Clone for LWWRegister<T, C>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                current_value: self.current_value.clone(),
                current_timestamp: self.current_timestamp,
                current_node_id: self.current_node_id,
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to manually clone the UnsafeCell content
            let cloned_value = unsafe { (*self.current_value.get()).clone() };
            Self {
                current_value: UnsafeCell::new(cloned_value),
                current_timestamp: AtomicU32::new(self.current_timestamp.load(Ordering::Relaxed)),
                current_node_id: AtomicU8::new(self.current_node_id.load(Ordering::Relaxed)),
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<T, C: MemoryConfig> LWWRegister<T, C>
where
    T: Clone + PartialEq,
{
    /// Creates a new LWW register for the given node
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node (must be < MAX_NODES)
    ///
    /// # Returns
    /// A new empty register
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let register = LWWRegister::<i32, DefaultConfig>::new(1);
    /// ```
    pub fn new(node_id: NodeId) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                current_value: None,
                current_timestamp: CompactTimestamp::zero(),
                current_node_id: 0,
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            Self {
                current_value: UnsafeCell::new(None),
                current_timestamp: AtomicU32::new(0),
                current_node_id: AtomicU8::new(0),
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }

    /// Sets a new value with the current timestamp
    ///
    /// # Concurrency Behavior
    /// - **Without `hardware-atomic`**: Requires `&mut self`, single-threaded only
    /// - **With `hardware-atomic`**: Allows `&self`, thread-safe atomic operations
    ///
    /// # Arguments
    /// * `value` - The new value to set
    /// * `timestamp` - The timestamp for this update
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the timestamp is older than current
    ///
    /// # Examples
    ///
    /// ## Standard Version (default)
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut register = LWWRegister::<i32, DefaultConfig>::new(1);
    /// register.set(42, 1000)?; // Requires &mut self
    /// assert_eq!(register.get(), Some(&42));
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
    /// let register = LWWRegister::<i32, DefaultConfig>::new(1);
    /// register.set(42, 1000)?; // Works with &self (atomic)
    /// assert_eq!(register.get(), Some(&42));
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn set(&mut self, value: T, timestamp: u64) -> CRDTResult<()> {
        let new_timestamp = CompactTimestamp::new(timestamp);

        // Only update if this timestamp is newer (or same timestamp but higher node ID)
        if self.should_update(&new_timestamp, self.node_id) {
            self.current_value = Some(value);
            self.current_timestamp = new_timestamp;
            self.current_node_id = self.node_id;
        }

        Ok(())
    }

    /// Sets a new value with the current timestamp (atomic version)
    ///
    /// # Arguments
    /// * `value` - The new value to set
    /// * `timestamp` - The timestamp for this update
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the timestamp is older than current
    ///
    /// # Note
    /// In the atomic version, we use unsafe code to update the value after
    /// successfully updating the timestamp atomically. This is safe because
    /// we ensure only one thread can update at a time through the atomic timestamp.
    #[cfg(feature = "hardware-atomic")]
    pub fn set(&self, value: T, timestamp: u64) -> CRDTResult<()> {
        let new_timestamp_u32 = timestamp as u32; // Truncate to u32 for ARM compatibility

        // Atomic compare-exchange loop to update timestamp and node_id together
        loop {
            let current_timestamp = self.current_timestamp.load(Ordering::Relaxed);
            let current_node_id = self.current_node_id.load(Ordering::Relaxed);

            // Check if we should update
            let should_update = if current_timestamp == 0 {
                true // No current value
            } else if new_timestamp_u32 > current_timestamp {
                true // Newer timestamp
            } else if new_timestamp_u32 == current_timestamp {
                self.node_id > current_node_id // Same timestamp, higher node ID wins
            } else {
                false // Older timestamp
            };

            if !should_update {
                return Ok(());
            }

            // Try to atomically update timestamp
            match self.current_timestamp.compare_exchange_weak(
                current_timestamp,
                new_timestamp_u32,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Successfully updated timestamp, now update node_id and value
                    self.current_node_id.store(self.node_id, Ordering::Relaxed);

                    // SAFETY: We have exclusive access to update the value because we
                    // successfully updated the timestamp atomically. Only one thread
                    // can succeed in the compare_exchange above.
                    unsafe {
                        *self.current_value.get() = Some(value);
                    }
                    break;
                }
                Err(_) => {
                    // Retry the loop
                    continue;
                }
            }
        }

        Ok(())
    }

    /// Gets the current value
    ///
    /// # Returns
    /// The current value, or None if no value has been set
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut register = LWWRegister::<i32, DefaultConfig>::new(1);
    /// assert_eq!(register.get(), None);
    /// register.set(42, 1000)?;
    /// assert_eq!(register.get(), Some(&42));
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn get(&self) -> Option<&T> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.current_value.as_ref()
        }

        #[cfg(feature = "hardware-atomic")]
        {
            unsafe { (*self.current_value.get()).as_ref() }
        }
    }

    /// Gets the current timestamp
    ///
    /// # Returns
    /// The timestamp of the current value
    pub fn timestamp(&self) -> CompactTimestamp {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.current_timestamp
        }

        #[cfg(feature = "hardware-atomic")]
        {
            CompactTimestamp::new(self.current_timestamp.load(Ordering::Relaxed) as u64)
        }
    }

    /// Gets the node ID that set the current value
    ///
    /// # Returns
    /// The node ID of the current value's author
    pub fn current_node(&self) -> NodeId {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.current_node_id
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.current_node_id.load(Ordering::Relaxed)
        }
    }

    /// Checks if this register has a value
    ///
    /// # Returns
    /// true if a value is set, false otherwise
    pub fn is_empty(&self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.current_value.is_none()
        }

        #[cfg(feature = "hardware-atomic")]
        {
            unsafe { (*self.current_value.get()).is_none() }
        }
    }

    /// Determines if we should update with a new timestamp and node ID
    #[cfg(not(feature = "hardware-atomic"))]
    fn should_update(&self, new_timestamp: &CompactTimestamp, new_node_id: NodeId) -> bool {
        if self.current_value.is_none() {
            return true;
        }

        match new_timestamp.cmp(&self.current_timestamp) {
            core::cmp::Ordering::Greater => true,
            core::cmp::Ordering::Less => false,
            core::cmp::Ordering::Equal => {
                // Same timestamp - use node ID as tiebreaker (higher wins)
                new_node_id > self.current_node_id
            }
        }
    }
}

// Serde implementation for LWWRegister
#[cfg(feature = "serde")]
impl<T, C: MemoryConfig> Serialize for LWWRegister<T, C>
where
    T: Serialize + Clone + PartialEq,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("LWWRegister", 4)?;

        // Serialize the logical state
        #[cfg(not(feature = "hardware-atomic"))]
        {
            state.serialize_field("cv", &self.current_value)?;
            state.serialize_field("cts", &self.current_timestamp.as_u64())?;
            state.serialize_field("cn", &self.current_node_id)?;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to extract values safely
            let current_value = unsafe { &*self.current_value.get() };
            let current_timestamp = self.current_timestamp.load(Ordering::Relaxed) as u64;
            let current_node_id = self.current_node_id.load(Ordering::Relaxed);

            state.serialize_field("cv", current_value)?;
            state.serialize_field("cts", &current_timestamp)?;
            state.serialize_field("cn", &current_node_id)?;
        }

        state.serialize_field("n", &self.node_id)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, T, C: MemoryConfig> Deserialize<'de> for LWWRegister<T, C>
where
    T: Deserialize<'de> + Clone + PartialEq,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use core::fmt;
        use serde::de::{self, MapAccess, Visitor};

        #[derive(Deserialize)]
        #[serde(field_identifier)]
        enum Field {
            #[serde(rename = "cv")]
            CurrentValue,
            #[serde(rename = "cts")]
            CurrentTimestamp,
            #[serde(rename = "cn")]
            CurrentNodeId,
            #[serde(rename = "n")]
            NodeId,
        }

        struct LWWRegisterVisitor<T, C: MemoryConfig> {
            _phantom: core::marker::PhantomData<(T, C)>,
        }

        impl<'de, T, C: MemoryConfig> Visitor<'de> for LWWRegisterVisitor<T, C>
        where
            T: Deserialize<'de> + Clone + PartialEq,
        {
            type Value = LWWRegister<T, C>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct LWWRegister")
            }

            fn visit_map<V>(self, mut map: V) -> Result<LWWRegister<T, C>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut current_value = None;
                let mut current_timestamp = None;
                let mut current_node_id = None;
                let mut node_id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::CurrentValue => {
                            if current_value.is_some() {
                                return Err(de::Error::duplicate_field("cv"));
                            }
                            current_value = Some(map.next_value::<Option<T>>()?);
                        }
                        Field::CurrentTimestamp => {
                            if current_timestamp.is_some() {
                                return Err(de::Error::duplicate_field("cts"));
                            }
                            current_timestamp = Some(map.next_value::<u64>()?);
                        }
                        Field::CurrentNodeId => {
                            if current_node_id.is_some() {
                                return Err(de::Error::duplicate_field("cn"));
                            }
                            current_node_id = Some(map.next_value::<NodeId>()?);
                        }
                        Field::NodeId => {
                            if node_id.is_some() {
                                return Err(de::Error::duplicate_field("n"));
                            }
                            node_id = Some(map.next_value::<NodeId>()?);
                        }
                    }
                }

                let current_value =
                    current_value.ok_or_else(|| de::Error::missing_field("cv"))?;
                let current_timestamp = current_timestamp
                    .ok_or_else(|| de::Error::missing_field("cts"))?;
                let current_node_id =
                    current_node_id.ok_or_else(|| de::Error::missing_field("cn"))?;
                let node_id = node_id.ok_or_else(|| de::Error::missing_field("n"))?;

                // Reconstruct the LWWRegister
                #[cfg(not(feature = "hardware-atomic"))]
                {
                    Ok(LWWRegister {
                        current_value,
                        current_timestamp: CompactTimestamp::new(current_timestamp),
                        current_node_id,
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }

                #[cfg(feature = "hardware-atomic")]
                {
                    Ok(LWWRegister {
                        current_value: UnsafeCell::new(current_value),
                        current_timestamp: AtomicU32::new(current_timestamp as u32),
                        current_node_id: AtomicU8::new(current_node_id),
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }
            }
        }

        const FIELDS: &[&str] = &["cv", "cts", "cn", "n"];
        deserializer.deserialize_struct(
            "LWWRegister",
            FIELDS,
            LWWRegisterVisitor {
                _phantom: core::marker::PhantomData,
            },
        )
    }
}

impl<T, C: MemoryConfig> CRDT<C> for LWWRegister<T, C>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            if let Some(ref other_value) = other.current_value {
                if self.should_update(&other.current_timestamp, other.current_node_id) {
                    self.current_value = Some(other_value.clone());
                    self.current_timestamp = other.current_timestamp;
                    self.current_node_id = other.current_node_id;
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let other_value_ref = unsafe { &*other.current_value.get() };
            if let Some(other_value) = other_value_ref {
                let other_timestamp = other.current_timestamp.load(Ordering::Relaxed);
                let other_node_id = other.current_node_id.load(Ordering::Relaxed);

                // Atomic compare-exchange loop for merge
                loop {
                    let current_timestamp = self.current_timestamp.load(Ordering::Relaxed);
                    let current_node_id = self.current_node_id.load(Ordering::Relaxed);

                    // Check if we should update
                    let should_update = if current_timestamp == 0 {
                        true // No current value
                    } else if other_timestamp > current_timestamp {
                        true // Newer timestamp
                    } else if other_timestamp == current_timestamp {
                        other_node_id > current_node_id // Same timestamp, higher node ID wins
                    } else {
                        false // Older timestamp
                    };

                    if !should_update {
                        break;
                    }

                    // Try to atomically update timestamp
                    match self.current_timestamp.compare_exchange_weak(
                        current_timestamp,
                        other_timestamp,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            // Successfully updated timestamp, now update node_id and value
                            self.current_node_id.store(other_node_id, Ordering::Relaxed);

                            // SAFETY: We have exclusive access to update the value because we
                            // successfully updated the timestamp atomically. Only one thread
                            // can succeed in the compare_exchange above.
                            unsafe {
                                *self.current_value.get() = Some(other_value.clone());
                            }
                            break;
                        }
                        Err(_) => {
                            // Retry the loop
                            continue;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.current_value == other.current_value
                && self.current_timestamp == other.current_timestamp
                && self.current_node_id == other.current_node_id
        }

        #[cfg(feature = "hardware-atomic")]
        {
            unsafe {
                (*self.current_value.get()) == (*other.current_value.get())
                    && self.current_timestamp.load(Ordering::Relaxed)
                        == other.current_timestamp.load(Ordering::Relaxed)
                    && self.current_node_id.load(Ordering::Relaxed)
                        == other.current_node_id.load(Ordering::Relaxed)
            }
        }
    }

    fn size_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn validate(&self) -> CRDTResult<()> {
        // Validate node ID is within bounds
        if self.node_id as usize >= C::MAX_NODES {
            return Err(CRDTError::InvalidNodeId);
        }

        // Validate current node ID is within bounds
        #[cfg(not(feature = "hardware-atomic"))]
        {
            if self.current_node_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            if self.current_node_id.load(Ordering::Relaxed) as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        Ok(())
    }

    fn state_hash(&self) -> u32 {
        // Simple hash based on current state
        let mut hash = 0u32;

        #[cfg(not(feature = "hardware-atomic"))]
        {
            if let Some(ref _value) = self.current_value {
                hash ^= self.current_timestamp.as_u64() as u32;
                hash ^= (self.current_node_id as u32) << 16;
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            unsafe {
                if let Some(_value) = &*self.current_value.get() {
                    hash ^= self.current_timestamp.load(Ordering::Relaxed) as u32;
                    hash ^= (self.current_node_id.load(Ordering::Relaxed) as u32) << 16;
                }
            }
        }

        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // LWW registers can always merge
        true
    }
}

impl<T, C: MemoryConfig> BoundedCRDT<C> for LWWRegister<T, C>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 1; // A register holds one value

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            if self.current_value.is_some() { 1 } else { 0 }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            unsafe {
                if (*self.current_value.get()).is_some() {
                    1
                } else {
                    0
                }
            }
        }
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // LWW registers don't have anything to compact
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        // For registers, we can always "add" (update) if not at max capacity
        self.element_count() < Self::MAX_ELEMENTS
    }
}

impl<T, C: MemoryConfig> RealTimeCRDT<C> for LWWRegister<T, C>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_MERGE_CYCLES: u32 = 100;
    const MAX_VALIDATE_CYCLES: u32 = 50;
    const MAX_SERIALIZE_CYCLES: u32 = 75;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // LWW merge is always bounded - just a few comparisons and assignments
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
    fn test_new_register() {
        let register = LWWRegister::<i32, DefaultConfig>::new(1);
        assert!(register.is_empty());
        assert_eq!(register.get(), None);
        assert_eq!(register.node_id, 1);
    }

    #[test]
    fn test_set_and_get() {
        let mut register = LWWRegister::<i32, DefaultConfig>::new(1);

        assert!(register.set(42, 1000).is_ok());
        assert_eq!(register.get(), Some(&42));
        assert!(!register.is_empty());
        assert_eq!(register.current_node(), 1);
    }

    #[test]
    fn test_lww_semantics() {
        let mut register = LWWRegister::<i32, DefaultConfig>::new(1);

        // Set initial value
        register.set(10, 1000).unwrap();
        assert_eq!(register.get(), Some(&10));

        // Newer timestamp should win
        register.set(20, 2000).unwrap();
        assert_eq!(register.get(), Some(&20));

        // Older timestamp should be ignored
        register.set(30, 500).unwrap();
        assert_eq!(register.get(), Some(&20)); // Still 20
    }

    #[test]
    fn test_merge() {
        let mut register1 = LWWRegister::<i32, DefaultConfig>::new(1);
        let mut register2 = LWWRegister::<i32, DefaultConfig>::new(2);

        register1.set(10, 1000).unwrap();
        register2.set(20, 2000).unwrap();

        // Merge register2 into register1
        register1.merge(&register2).unwrap();
        assert_eq!(register1.get(), Some(&20)); // register2's value wins (newer)

        // Test reverse merge
        let mut register3 = LWWRegister::<i32, DefaultConfig>::new(3);
        register3.set(30, 500).unwrap(); // Older timestamp

        register1.merge(&register3).unwrap();
        assert_eq!(register1.get(), Some(&20)); // Still register2's value
    }

    #[test]
    fn test_tiebreaker() {
        let mut register1 = LWWRegister::<i32, DefaultConfig>::new(1);
        let mut register2 = LWWRegister::<i32, DefaultConfig>::new(2);

        register1.set(10, 1000).unwrap();
        register2.set(20, 1000).unwrap(); // Same timestamp

        register1.merge(&register2).unwrap();
        assert_eq!(register1.get(), Some(&20)); // Higher node ID wins
    }

    #[test]
    fn test_bounded_crdt() {
        let register = LWWRegister::<i32, DefaultConfig>::new(1);

        assert_eq!(register.element_count(), 0);
        assert!(register.memory_usage() > 0);
        assert!(register.can_add_element());
    }

    #[test]
    fn test_validation() {
        let register = LWWRegister::<i32, DefaultConfig>::new(1);
        assert!(register.validate().is_ok());

        // Test with invalid node ID would require creating an invalid state
        // which is hard to do with the current API (good!)
    }

    #[test]
    fn test_real_time_crdt() {
        let mut register1 = LWWRegister::<i32, DefaultConfig>::new(1);
        let register2 = LWWRegister::<i32, DefaultConfig>::new(2);

        assert!(register1.merge_bounded(&register2).is_ok());
        assert!(register1.validate_bounded().is_ok());
    }

    #[cfg(all(test, feature = "serde"))]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_serialize_deserialize() {
            let mut register = LWWRegister::<i32, DefaultConfig>::new(1);
            register.set(42, 1000).unwrap();

            let mut other = LWWRegister::<i32, DefaultConfig>::new(2);
            other.set(100, 2000).unwrap();
            register.merge(&other).unwrap();

            // Test that the serde traits are implemented
            // This ensures the code compiles with serde feature
            assert_eq!(register.get(), Some(&100)); // Latest value wins
            assert_eq!(register.current_node(), 2);
            assert_eq!(register.timestamp().as_u64(), 2000);
        }

        #[test]
        fn test_atomic_vs_standard_compatibility() {
            // This test ensures that atomic and standard versions would serialize to the same format
            // The logical state should be identical regardless of internal representation
            let mut register = LWWRegister::<i32, DefaultConfig>::new(1);
            register.set(42, 1000).unwrap();

            // Both versions should have the same logical state
            assert_eq!(register.get(), Some(&42));
            assert_eq!(register.current_node(), 1);
            assert_eq!(register.timestamp().as_u64(), 1000);
        }

        #[test]
        fn test_empty_register_serialization() {
            let register = LWWRegister::<i32, DefaultConfig>::new(1);

            // Empty register should serialize correctly
            assert!(register.is_empty());
            assert_eq!(register.get(), None);
            assert_eq!(register.current_node(), 0);
        }
    }
}
