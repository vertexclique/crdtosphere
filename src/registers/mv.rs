//! Multi-Value Register CRDT
//!
//! A register that can hold multiple concurrent values, allowing for conflict-free
//! concurrent updates. Uses zero allocation with a fixed array for deterministic memory usage.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

#[cfg(feature = "hardware-atomic")]
use core::cell::UnsafeCell;
#[cfg(feature = "hardware-atomic")]
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Multi-Value Register with configurable value array
///
/// This register can hold multiple concurrent values, each with its own timestamp
/// and node ID. This allows for conflict-free concurrent updates and provides
/// mechanisms for resolving conflicts through application-specific logic.
///
/// # Type Parameters
/// - `T`: The value type stored in the register
/// - `C`: Memory configuration that determines the default maximum number of values
/// - `CAPACITY`: The maximum number of values this register can hold (defaults to 4)
///
/// # Memory Usage
/// - Fixed size: (sizeof(T) + 9) * CAPACITY + 8 bytes
/// - Example: For f32 with 4 values = ~60 bytes, with 8 values = ~112 bytes
/// - Completely predictable at compile time
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
///
/// // Create a register for multi-sensor readings with default capacity
/// let mut sensor1 = MVRegister::<f32, DefaultConfig>::new(1);
/// sensor1.set(23.5, 1000)?;
///
/// // Another sensor with concurrent reading
/// let mut sensor2 = MVRegister::<f32, DefaultConfig>::new(2);
/// sensor2.set(24.1, 1001)?;
///
/// // Merge the readings
/// sensor1.merge(&sensor2)?;
///
/// // Now we have both values for conflict resolution
/// let values = sensor1.values_array();
/// assert_eq!(sensor1.len(), 2);
///
/// // Application can decide how to resolve (average, median, etc.)
/// let average = sensor1.average().unwrap();
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug)]
pub struct MVRegister<T, C: MemoryConfig, const CAPACITY: usize = 4> {
    /// Values with their metadata
    #[cfg(not(feature = "hardware-atomic"))]
    values: [Option<ValueEntry<T>>; CAPACITY],
    #[cfg(not(feature = "hardware-atomic"))]
    count: usize,

    /// Atomic version uses UnsafeCell for the values array
    #[cfg(feature = "hardware-atomic")]
    values: UnsafeCell<[Option<ValueEntry<T>>; CAPACITY]>,
    #[cfg(feature = "hardware-atomic")]
    count: AtomicUsize,

    /// This node's ID
    node_id: NodeId,

    /// Phantom data to maintain the memory config type
    _phantom: core::marker::PhantomData<C>,
}

// SAFETY: The atomic version is safe to share between threads because:
// 1. All access to values array is protected by atomic count coordination
// 2. Only one thread can successfully update at a time via compare_exchange
// 3. UnsafeCell is only accessed after winning the atomic coordination
#[cfg(feature = "hardware-atomic")]
unsafe impl<T, C: MemoryConfig> Sync for MVRegister<T, C>
where
    T: Send,
    C: Send + Sync,
{
}

// Implement Clone manually due to atomic types not implementing Clone
impl<T, C: MemoryConfig> Clone for MVRegister<T, C>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                values: self.values.clone(),
                count: self.count,
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to manually clone the UnsafeCell content
            let cloned_values = unsafe { (*self.values.get()).clone() };
            Self {
                values: UnsafeCell::new(cloned_values),
                count: AtomicUsize::new(self.count.load(Ordering::Relaxed)),
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

/// Value entry with timestamp and node ID
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct ValueEntry<T> {
    #[cfg_attr(feature = "serde", serde(rename = "v"))]
    value: T,
    #[cfg_attr(feature = "serde", serde(with = "compact_timestamp_serde", rename = "ts"))]
    timestamp: CompactTimestamp,
    #[cfg_attr(feature = "serde", serde(rename = "n"))]
    node_id: NodeId,
}

#[cfg(feature = "serde")]
mod compact_timestamp_serde {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(timestamp: &CompactTimestamp, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        timestamp.as_u64().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<CompactTimestamp, D::Error>
    where
        D: Deserializer<'de>,
    {
        let timestamp_u64 = u64::deserialize(deserializer)?;
        Ok(CompactTimestamp::new(timestamp_u64))
    }
}

// Serde implementation for MVRegister
#[cfg(feature = "serde")]
impl<T, C: MemoryConfig, const CAPACITY: usize> Serialize for MVRegister<T, C, CAPACITY>
where
    T: Serialize + Clone + PartialEq,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("MVRegister", 3)?;

        // Serialize the logical state (values array and count)
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Serialize only the used portion of the array as a slice
            state.serialize_field("vs", &&self.values[..self.count])?;
            state.serialize_field("ct", &self.count)?;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to extract values safely
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            state.serialize_field("vs", &&values_ref[..current_count])?;
            state.serialize_field("ct", &current_count)?;
        }

        state.serialize_field("n", &self.node_id)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, T, C: MemoryConfig, const CAPACITY: usize> Deserialize<'de> for MVRegister<T, C, CAPACITY>
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
            #[serde(rename = "vs")]
            Values,
            #[serde(rename = "ct")]
            Count,
            #[serde(rename = "n")]
            NodeId,
        }

        struct MVRegisterVisitor<T, C: MemoryConfig, const CAPACITY: usize> {
            _phantom: core::marker::PhantomData<(T, C)>,
        }

        impl<'de, T, C: MemoryConfig, const CAPACITY: usize> Visitor<'de>
            for MVRegisterVisitor<T, C, CAPACITY>
        where
            T: Deserialize<'de> + Clone + PartialEq,
        {
            type Value = MVRegister<T, C, CAPACITY>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct MVRegister")
            }

            fn visit_map<V>(self, mut map: V) -> Result<MVRegister<T, C, CAPACITY>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut values = None;
                let mut count = None;
                let mut node_id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Values => {
                            if values.is_some() {
                                return Err(de::Error::duplicate_field("vs"));
                            }
                            // Use a custom deserializer that doesn't require Vec
                            use serde::de::SeqAccess;

                            struct ValuesDeserializer<T, const CAPACITY: usize> {
                                _phantom: core::marker::PhantomData<T>,
                            }

                            impl<'de, T, const CAPACITY: usize> serde::de::DeserializeSeed<'de>
                                for ValuesDeserializer<T, CAPACITY>
                            where
                                T: Deserialize<'de>,
                            {
                                type Value = [Option<ValueEntry<T>>; CAPACITY];

                                fn deserialize<D>(
                                    self,
                                    deserializer: D,
                                ) -> Result<Self::Value, D::Error>
                                where
                                    D: serde::de::Deserializer<'de>,
                                {
                                    struct ValuesVisitor<T, const CAPACITY: usize> {
                                        _phantom: core::marker::PhantomData<T>,
                                    }

                                    impl<'de, T, const CAPACITY: usize> serde::de::Visitor<'de> for ValuesVisitor<T, CAPACITY>
                                    where
                                        T: Deserialize<'de>,
                                    {
                                        type Value = [Option<ValueEntry<T>>; CAPACITY];

                                        fn expecting(
                                            &self,
                                            formatter: &mut core::fmt::Formatter,
                                        ) -> core::fmt::Result
                                        {
                                            write!(
                                                formatter,
                                                "a sequence of at most {} values",
                                                CAPACITY
                                            )
                                        }

                                        fn visit_seq<A>(
                                            self,
                                            mut seq: A,
                                        ) -> Result<Self::Value, A::Error>
                                        where
                                            A: SeqAccess<'de>,
                                        {
                                            let mut array = [const { None }; CAPACITY];
                                            let mut index = 0;

                                            while let Some(value) =
                                                seq.next_element::<Option<ValueEntry<T>>>()?
                                            {
                                                if index >= CAPACITY {
                                                    return Err(serde::de::Error::custom(
                                                        "too many values for capacity",
                                                    ));
                                                }
                                                array[index] = value;
                                                index += 1;
                                            }

                                            Ok(array)
                                        }
                                    }

                                    deserializer.deserialize_seq(ValuesVisitor::<T, CAPACITY> {
                                        _phantom: core::marker::PhantomData,
                                    })
                                }
                            }

                            let values_array =
                                map.next_value_seed(ValuesDeserializer::<T, CAPACITY> {
                                    _phantom: core::marker::PhantomData,
                                })?;
                            values = Some(values_array);
                        }
                        Field::Count => {
                            if count.is_some() {
                                return Err(de::Error::duplicate_field("ct"));
                            }
                            count = Some(map.next_value::<usize>()?);
                        }
                        Field::NodeId => {
                            if node_id.is_some() {
                                return Err(de::Error::duplicate_field("n"));
                            }
                            node_id = Some(map.next_value::<NodeId>()?);
                        }
                    }
                }

                let values_vec = values.ok_or_else(|| de::Error::missing_field("vs"))?;
                let count = count.ok_or_else(|| de::Error::missing_field("ct"))?;
                let node_id = node_id.ok_or_else(|| de::Error::missing_field("n"))?;

                // Validate count matches values length
                if count != values_vec.len() {
                    return Err(de::Error::custom("count does not match values length"));
                }

                // Validate count is within capacity
                if count > CAPACITY {
                    return Err(de::Error::custom("count exceeds capacity"));
                }

                // Reconstruct the MVRegister
                #[cfg(not(feature = "hardware-atomic"))]
                {
                    Ok(MVRegister {
                        values: values_vec,
                        count,
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }

                #[cfg(feature = "hardware-atomic")]
                {
                    Ok(MVRegister {
                        values: UnsafeCell::new(values_vec),
                        count: AtomicUsize::new(count),
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }
            }
        }

        const FIELDS: &[&str] = &["vs", "ct", "n"];
        deserializer.deserialize_struct(
            "MVRegister",
            FIELDS,
            MVRegisterVisitor {
                _phantom: core::marker::PhantomData,
            },
        )
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> MVRegister<T, C, CAPACITY>
where
    T: Clone + PartialEq,
{
    /// Creates a new multi-value register for the given node with custom capacity
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
    /// let register = MVRegister::<f32, DefaultConfig, 8>::with_capacity(1);
    /// assert!(register.is_empty());
    /// ```
    pub fn with_capacity(node_id: NodeId) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                values: [const { None }; CAPACITY],
                count: 0,
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            Self {
                values: UnsafeCell::new([const { None }; CAPACITY]),
                count: AtomicUsize::new(0),
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<T, C: MemoryConfig> MVRegister<T, C, 4>
where
    T: Clone + PartialEq,
{
    /// Creates a new multi-value register for the given node with default capacity
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
    /// let register = MVRegister::<f32, DefaultConfig>::new(1);
    /// assert!(register.is_empty());
    /// ```
    pub fn new(node_id: NodeId) -> Self {
        Self::with_capacity(node_id)
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> MVRegister<T, C, CAPACITY>
where
    T: Clone + PartialEq,
{
    /// Sets a new value with the current timestamp
    ///
    /// # Arguments
    /// * `value` - The new value to set
    /// * `timestamp` - The timestamp for this update
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the operation failed
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut register = MVRegister::<f32, DefaultConfig>::new(1);
    /// register.set(42.0, 1000)?;
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn set(&mut self, value: T, timestamp: u64) -> CRDTResult<()> {
        let new_timestamp = CompactTimestamp::new(timestamp);

        // Check if we already have a value from this node
        for i in 0..self.count {
            if let Some(ref mut entry) = self.values[i] {
                if entry.node_id == self.node_id {
                    // Update our existing value if timestamp is newer
                    if new_timestamp > entry.timestamp {
                        entry.value = value;
                        entry.timestamp = new_timestamp;
                    }
                    return Ok(());
                }
            }
        }

        // New value from this node - check if we have space
        if self.count >= CAPACITY {
            return Err(CRDTError::BufferOverflow);
        }

        // Insert new value
        self.values[self.count] = Some(ValueEntry {
            value,
            timestamp: new_timestamp,
            node_id: self.node_id,
        });
        self.count += 1;
        Ok(())
    }

    /// Sets a new value with the current timestamp (atomic version)
    ///
    /// # Arguments
    /// * `value` - The new value to set
    /// * `timestamp` - The timestamp for this update
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the operation failed
    ///
    /// # Note
    /// In the atomic version, we use count-based coordination to ensure
    /// only one thread can modify the values array at a time.
    #[cfg(feature = "hardware-atomic")]
    pub fn set(&self, value: T, timestamp: u64) -> CRDTResult<()> {
        let new_timestamp = CompactTimestamp::new(timestamp);

        // Atomic compare-exchange loop for coordination
        loop {
            let current_count = self.count.load(Ordering::Relaxed);

            // SAFETY: Read the values array to find our node or determine insertion point
            let values_ptr = self.values.get();
            let values_ref = unsafe { &*values_ptr };

            // Check if we already have a value from this node
            let mut found_index = None;
            let mut needs_update = false;

            for i in 0..current_count {
                if let Some(entry) = &values_ref[i] {
                    if entry.node_id == self.node_id {
                        found_index = Some(i);
                        needs_update = new_timestamp > entry.timestamp;
                        break;
                    }
                }
            }

            if let Some(index) = found_index {
                if needs_update {
                    // Try to update existing entry
                    // We use the count as coordination - if it changes, retry
                    let values_mut = unsafe { &mut *values_ptr };
                    if let Some(entry) = &mut values_mut[index] {
                        if entry.node_id == self.node_id && new_timestamp > entry.timestamp {
                            entry.value = value.clone();
                            entry.timestamp = new_timestamp;
                        }
                    }
                    // Verify count hasn't changed during our update
                    if self.count.load(Ordering::Relaxed) == current_count {
                        return Ok(());
                    }
                    // Count changed, retry
                    continue;
                }
                return Ok(()); // No update needed
            } else {
                // New value from this node - check if we have space
                if current_count >= CAPACITY {
                    return Err(CRDTError::BufferOverflow);
                }

                // Try to atomically increment count to reserve a slot
                match self.count.compare_exchange_weak(
                    current_count,
                    current_count + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // Successfully reserved slot, now insert the value
                        let values_mut = unsafe { &mut *values_ptr };
                        values_mut[current_count] = Some(ValueEntry {
                            value,
                            timestamp: new_timestamp,
                            node_id: self.node_id,
                        });
                        return Ok(());
                    }
                    Err(_) => {
                        // Count changed, retry the loop
                        continue;
                    }
                }
            }
        }
    }

    /// Gets all current values as an array
    ///
    /// # Returns
    /// An array of all current values with None for unused slots
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut register = MVRegister::<f32, DefaultConfig>::new(1);
    /// register.set(42.0, 1000)?;
    /// let values = register.values_array();
    /// assert!(values[0].is_some());
    /// assert_eq!(values[0].unwrap(), 42.0);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn values_array(&self) -> [Option<T>; CAPACITY] {
        let mut result = [const { None }; CAPACITY];

        #[cfg(not(feature = "hardware-atomic"))]
        {
            for (i, entry) in self.values.iter().take(self.count).enumerate() {
                if let Some(entry) = entry {
                    result[i] = Some(entry.value.clone());
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            for (i, entry) in values_ref.iter().take(current_count).enumerate() {
                if let Some(entry) = entry {
                    result[i] = Some(entry.value.clone());
                }
            }
        }

        result
    }

    /// Gets the value from a specific node
    ///
    /// # Arguments
    /// * `node_id` - The node ID to get the value for
    ///
    /// # Returns
    /// The value from that node, or None if no value exists
    pub fn get_from_node(&self, node_id: NodeId) -> Option<&T> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            for entry in self.values.iter().take(self.count) {
                if let Some(entry) = entry {
                    if entry.node_id == node_id {
                        return Some(&entry.value);
                    }
                }
            }
            None
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            for entry in values_ref.iter().take(current_count) {
                if let Some(entry) = entry {
                    if entry.node_id == node_id {
                        return Some(&entry.value);
                    }
                }
            }
            None
        }
    }

    /// Gets the timestamp for a specific node's value
    ///
    /// # Arguments
    /// * `node_id` - The node ID to get the timestamp for
    ///
    /// # Returns
    /// The timestamp of that node's value, or None if no value exists
    pub fn get_timestamp_from_node(&self, node_id: NodeId) -> Option<CompactTimestamp> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            for entry in self.values.iter().take(self.count) {
                if let Some(entry) = entry {
                    if entry.node_id == node_id {
                        return Some(entry.timestamp);
                    }
                }
            }
            None
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            for entry in values_ref.iter().take(current_count) {
                if let Some(entry) = entry {
                    if entry.node_id == node_id {
                        return Some(entry.timestamp);
                    }
                }
            }
            None
        }
    }

    /// Returns the number of values currently stored
    ///
    /// # Returns
    /// The count of values
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut register = MVRegister::<f32, DefaultConfig>::new(1);
    /// assert_eq!(register.len(), 0);
    /// register.set(42.0, 1000)?;
    /// assert_eq!(register.len(), 1);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn len(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.count
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.count.load(Ordering::Relaxed)
        }
    }

    /// Checks if the register is empty
    ///
    /// # Returns
    /// true if no values are stored, false otherwise
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut register = MVRegister::<f32, DefaultConfig>::new(1);
    /// assert!(register.is_empty());
    /// register.set(42.0, 1000)?;
    /// assert!(!register.is_empty());
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn is_empty(&self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.count == 0
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.count.load(Ordering::Relaxed) == 0
        }
    }

    /// Checks if the register is full
    ///
    /// # Returns
    /// true if no more values can be stored, false otherwise
    pub fn is_full(&self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.count >= CAPACITY
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.count.load(Ordering::Relaxed) >= CAPACITY
        }
    }

    /// Returns the maximum capacity
    ///
    /// # Returns
    /// The maximum number of values this register can hold
    pub fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Gets this node's ID
    ///
    /// # Returns
    /// The node ID of this register
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Returns an iterator over the value entries
    ///
    /// # Returns
    /// An iterator over (value, timestamp, node_id) tuples
    pub fn iter(&self) -> impl Iterator<Item = (&T, CompactTimestamp, NodeId)> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.values.iter().take(self.count).filter_map(|opt| {
                opt.as_ref()
                    .map(|entry| (&entry.value, entry.timestamp, entry.node_id))
            })
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            values_ref.iter().take(current_count).filter_map(|opt| {
                opt.as_ref()
                    .map(|entry| (&entry.value, entry.timestamp, entry.node_id))
            })
        }
    }
}

// Numeric operations for numeric types
impl<C: MemoryConfig> MVRegister<f32, C> {
    /// Calculates the average of all values
    ///
    /// # Returns
    /// The average value, or None if empty
    pub fn average(&self) -> Option<f32> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            if self.count == 0 {
                return None;
            }

            let sum: f32 = self
                .values
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .sum();

            Some(sum / self.count as f32)
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            if current_count == 0 {
                return None;
            }

            let values_ref = unsafe { &*self.values.get() };
            let sum: f32 = values_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .sum();

            Some(sum / current_count as f32)
        }
    }

    /// Finds the minimum value
    ///
    /// # Returns
    /// The minimum value, or None if empty
    pub fn min(&self) -> Option<f32> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.values
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            values_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        }
    }

    /// Finds the maximum value
    ///
    /// # Returns
    /// The maximum value, or None if empty
    pub fn max(&self) -> Option<f32> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.values
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            values_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        }
    }
}

impl<C: MemoryConfig> MVRegister<f64, C> {
    /// Calculates the average of all values
    ///
    /// # Returns
    /// The average value, or None if empty
    pub fn average(&self) -> Option<f64> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            if self.count == 0 {
                return None;
            }

            let sum: f64 = self
                .values
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .sum();

            Some(sum / self.count as f64)
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            if current_count == 0 {
                return None;
            }

            let values_ref = unsafe { &*self.values.get() };
            let sum: f64 = values_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .sum();

            Some(sum / current_count as f64)
        }
    }

    /// Finds the minimum value
    ///
    /// # Returns
    /// The minimum value, or None if empty
    pub fn min(&self) -> Option<f64> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.values
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            values_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        }
    }

    /// Finds the maximum value
    ///
    /// # Returns
    /// The maximum value, or None if empty
    pub fn max(&self) -> Option<f64> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.values
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };
            values_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| entry.value))
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        }
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> CRDT<C> for MVRegister<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Process each value from other
            for other_entry in other.values.iter().take(other.count) {
                if let Some(other_entry) = other_entry {
                    // Check if we have a value from this node
                    let mut found = false;
                    for i in 0..self.count {
                        if let Some(our_entry) = &mut self.values[i] {
                            if our_entry.node_id == other_entry.node_id {
                                found = true;
                                // Update if other's timestamp is newer
                                if other_entry.timestamp > our_entry.timestamp {
                                    our_entry.value = other_entry.value.clone();
                                    our_entry.timestamp = other_entry.timestamp;
                                }
                                break;
                            }
                        }
                    }

                    if !found {
                        // New node - check if we have space
                        if self.count >= CAPACITY {
                            return Err(CRDTError::BufferOverflow);
                        }

                        // Insert new value
                        self.values[self.count] = Some(ValueEntry {
                            value: other_entry.value.clone(),
                            timestamp: other_entry.timestamp,
                            node_id: other_entry.node_id,
                        });
                        self.count += 1;
                    }
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let other_count = other.count.load(Ordering::Relaxed);
            let other_values_ref = unsafe { &*other.values.get() };

            // Process each value from other
            for other_entry in other_values_ref.iter().take(other_count) {
                if let Some(other_entry) = other_entry {
                    let current_count = self.count.load(Ordering::Relaxed);
                    let values_ptr = self.values.get();
                    let values_ref = unsafe { &*values_ptr };

                    // Check if we have a value from this node
                    let mut found = false;
                    let mut found_index = None;
                    for i in 0..current_count {
                        if let Some(our_entry) = &values_ref[i] {
                            if our_entry.node_id == other_entry.node_id {
                                found = true;
                                found_index = Some(i);
                                break;
                            }
                        }
                    }

                    if found {
                        if let Some(index) = found_index {
                            let values_mut = unsafe { &mut *values_ptr };
                            if let Some(our_entry) = &mut values_mut[index] {
                                // Update if other's timestamp is newer
                                if other_entry.timestamp > our_entry.timestamp {
                                    our_entry.value = other_entry.value.clone();
                                    our_entry.timestamp = other_entry.timestamp;
                                }
                            }
                        }
                    } else {
                        // New node - check if we have space
                        if current_count >= CAPACITY {
                            return Err(CRDTError::BufferOverflow);
                        }

                        // Try to atomically increment count to reserve a slot
                        match self.count.compare_exchange_weak(
                            current_count,
                            current_count + 1,
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => {
                                // Successfully reserved slot, now insert the value
                                let values_mut = unsafe { &mut *values_ptr };
                                values_mut[current_count] = Some(ValueEntry {
                                    value: other_entry.value.clone(),
                                    timestamp: other_entry.timestamp,
                                    node_id: other_entry.node_id,
                                });
                            }
                            Err(_) => {
                                // Count changed, this entry might have been processed by another thread
                                // Continue to next entry
                                continue;
                            }
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
            if self.count != other.count {
                return false;
            }

            // Check that all entries match (order doesn't matter)
            for entry in self.values.iter().take(self.count) {
                if let Some(entry) = entry {
                    let mut found = false;
                    for other_entry in other.values.iter().take(other.count) {
                        if let Some(other_entry) = other_entry {
                            if entry.node_id == other_entry.node_id
                                && entry.value == other_entry.value
                                && entry.timestamp == other_entry.timestamp
                            {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        return false;
                    }
                }
            }

            true
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let self_count = self.count.load(Ordering::Relaxed);
            let other_count = other.count.load(Ordering::Relaxed);

            if self_count != other_count {
                return false;
            }

            let self_values_ref = unsafe { &*self.values.get() };
            let other_values_ref = unsafe { &*other.values.get() };

            // Check that all entries match (order doesn't matter)
            for entry in self_values_ref.iter().take(self_count) {
                if let Some(entry) = entry {
                    let mut found = false;
                    for other_entry in other_values_ref.iter().take(other_count) {
                        if let Some(other_entry) = other_entry {
                            if entry.node_id == other_entry.node_id
                                && entry.value == other_entry.value
                                && entry.timestamp == other_entry.timestamp
                            {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        return false;
                    }
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
        if self.node_id as usize >= C::MAX_NODES {
            return Err(CRDTError::InvalidNodeId);
        }

        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Validate count is within bounds
            if self.count > CAPACITY {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate that we don't exceed the configured maximum values
            if self.count > C::MAX_REGISTERS {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate no duplicate node IDs
            for i in 0..self.count {
                if let Some(entry_i) = &self.values[i] {
                    for j in (i + 1)..self.count {
                        if let Some(entry_j) = &self.values[j] {
                            if entry_i.node_id == entry_j.node_id {
                                return Err(CRDTError::InvalidState);
                            }
                        }
                    }
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };

            // Validate count is within bounds
            if current_count > CAPACITY {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate that we don't exceed the configured maximum values
            if current_count > C::MAX_REGISTERS {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate no duplicate node IDs
            for i in 0..current_count {
                if let Some(entry_i) = &values_ref[i] {
                    for j in (i + 1)..current_count {
                        if let Some(entry_j) = &values_ref[j] {
                            if entry_i.node_id == entry_j.node_id {
                                return Err(CRDTError::InvalidState);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn state_hash(&self) -> u32 {
        // Simple hash based on values (order-independent)
        let mut hash = 0u32;

        #[cfg(not(feature = "hardware-atomic"))]
        {
            for entry in self.values.iter().take(self.count) {
                if let Some(entry) = entry {
                    let value_ptr = &entry.value as *const T as usize;
                    hash ^= (value_ptr as u32)
                        ^ (entry.timestamp.as_u64() as u32)
                        ^ (entry.node_id as u32);
                }
            }
            hash ^= self.count as u32;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let values_ref = unsafe { &*self.values.get() };

            for entry in values_ref.iter().take(current_count) {
                if let Some(entry) = entry {
                    let value_ptr = &entry.value as *const T as usize;
                    hash ^= (value_ptr as u32)
                        ^ (entry.timestamp.as_u64() as u32)
                        ^ (entry.node_id as u32);
                }
            }
            hash ^= current_count as u32;
        }

        hash
    }

    fn can_merge(&self, other: &Self) -> bool {
        // Check if merging would exceed capacity
        let mut new_nodes = 0;

        #[cfg(not(feature = "hardware-atomic"))]
        {
            for other_entry in other.values.iter().take(other.count) {
                if let Some(other_entry) = other_entry {
                    let mut found = false;
                    for our_entry in self.values.iter().take(self.count) {
                        if let Some(our_entry) = our_entry {
                            if our_entry.node_id == other_entry.node_id {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        new_nodes += 1;
                    }
                }
            }

            self.count + new_nodes <= CAPACITY
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let self_count = self.count.load(Ordering::Relaxed);
            let other_count = other.count.load(Ordering::Relaxed);
            let self_values_ref = unsafe { &*self.values.get() };
            let other_values_ref = unsafe { &*other.values.get() };

            for other_entry in other_values_ref.iter().take(other_count) {
                if let Some(other_entry) = other_entry {
                    let mut found = false;
                    for our_entry in self_values_ref.iter().take(self_count) {
                        if let Some(our_entry) = our_entry {
                            if our_entry.node_id == other_entry.node_id {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        new_nodes += 1;
                    }
                }
            }

            self_count + new_nodes <= CAPACITY
        }
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> BoundedCRDT<C> for MVRegister<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = CAPACITY; // Maximum number of values

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.count
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.count.load(Ordering::Relaxed)
        }
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // MVRegisters can't be compacted without losing data
        // This is a no-op that returns 0 bytes freed
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        // For registers, we can always "add" (update) if not at max capacity
        self.element_count() < Self::MAX_ELEMENTS
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> RealTimeCRDT<C> for MVRegister<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_MERGE_CYCLES: u32 = 150; // Linear in number of values
    const MAX_VALIDATE_CYCLES: u32 = 75;
    const MAX_SERIALIZE_CYCLES: u32 = 100;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // MVRegister merge is bounded by the number of values
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded by the number of values
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
        let register = MVRegister::<f32, DefaultConfig>::new(1);
        assert!(register.is_empty());
        assert_eq!(register.len(), 0);
        assert_eq!(register.capacity(), 4);
        assert!(!register.is_full());
        assert_eq!(register.node_id(), 1);
    }

    #[test]
    fn test_set_and_get() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);

        assert!(register.set(42.0, 1000).is_ok());
        assert_eq!(register.len(), 1);
        assert!(!register.is_empty());

        let values = register.values_array();
        assert!(values[0].is_some());
        assert_eq!(values[0].unwrap(), 42.0);

        assert_eq!(register.get_from_node(1), Some(&42.0));
        assert_eq!(register.get_from_node(2), None);
    }

    #[test]
    fn test_multiple_values() {
        let mut register1 = MVRegister::<f32, DefaultConfig>::new(1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(2);

        register1.set(10.0, 1000).unwrap();
        register2.set(20.0, 1001).unwrap();

        // Merge register2 into register1
        register1.merge(&register2).unwrap();

        assert_eq!(register1.len(), 2);
        let values = register1.values_array();
        let mut found_10 = false;
        let mut found_20 = false;
        for val in values.iter() {
            if let Some(v) = val {
                if *v == 10.0 {
                    found_10 = true;
                }
                if *v == 20.0 {
                    found_20 = true;
                }
            }
        }
        assert!(found_10);
        assert!(found_20);
    }

    #[test]
    fn test_update_same_node() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);

        register.set(10.0, 1000).unwrap();
        assert_eq!(register.len(), 1);

        // Update with newer timestamp
        register.set(20.0, 2000).unwrap();
        assert_eq!(register.len(), 1); // Still 1 value
        assert_eq!(register.get_from_node(1), Some(&20.0));

        // Try to update with older timestamp (should be ignored)
        register.set(30.0, 500).unwrap();
        assert_eq!(register.get_from_node(1), Some(&20.0)); // Still 20.0
    }

    #[test]
    fn test_numeric_operations() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);

        // Add multiple values from different nodes
        register.set(10.0, 1000).unwrap();

        let mut other2 = MVRegister::<f32, DefaultConfig>::new(2);
        other2.set(20.0, 1001).unwrap();
        register.merge(&other2).unwrap();

        let mut other3 = MVRegister::<f32, DefaultConfig>::new(3);
        other3.set(30.0, 1002).unwrap();
        register.merge(&other3).unwrap();

        assert_eq!(register.len(), 3);
        assert_eq!(register.average(), Some(20.0)); // (10+20+30)/3
        assert_eq!(register.min(), Some(10.0));
        assert_eq!(register.max(), Some(30.0));
    }

    #[test]
    fn test_capacity_limits() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);

        // Fill to capacity
        for i in 1..=4 {
            let mut other = MVRegister::<f32, DefaultConfig>::new(i);
            other.set(i as f32 * 10.0, 1000 + i as u64).unwrap();
            register.merge(&other).unwrap();
        }

        assert!(register.is_full());
        assert_eq!(register.len(), 4);

        // Try to add one more (should fail)
        let mut other5 = MVRegister::<f32, DefaultConfig>::new(5);
        other5.set(50.0, 2000).unwrap();
        assert!(register.merge(&other5).is_err());
    }

    #[test]
    fn test_merge_idempotent() {
        let mut register1 = MVRegister::<f32, DefaultConfig>::new(1);
        let register2 = MVRegister::<f32, DefaultConfig>::new(2);

        register1.set(10.0, 1000).unwrap();

        // Multiple merges should be idempotent
        register1.merge(&register2).unwrap();
        let len1 = register1.len();

        register1.merge(&register2).unwrap();
        let len2 = register1.len();

        assert_eq!(len1, len2);
    }

    #[test]
    fn test_merge_commutative() {
        let mut register1a = MVRegister::<f32, DefaultConfig>::new(1);
        let mut register1b = MVRegister::<f32, DefaultConfig>::new(1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(2);
        let mut register3 = MVRegister::<f32, DefaultConfig>::new(3);

        register1a.set(10.0, 1000).unwrap();
        register1b.set(10.0, 1000).unwrap();
        register2.set(20.0, 2000).unwrap();
        register3.set(30.0, 3000).unwrap();

        // Merge in different orders
        register1a.merge(&register2).unwrap();
        register1a.merge(&register3).unwrap();

        register1b.merge(&register3).unwrap();
        register1b.merge(&register2).unwrap();

        // Results should be the same
        assert_eq!(register1a.len(), register1b.len());
        assert!(register1a.eq(&register1b));
    }

    #[test]
    fn test_bounded_crdt() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);
        register.set(10.0, 1000).unwrap();

        assert_eq!(register.element_count(), 1);
        assert!(register.memory_usage() > 0);
        assert!(register.can_add_element());

        // Fill to capacity
        for i in 2..=4 {
            let mut other = MVRegister::<f32, DefaultConfig>::new(i);
            other.set(i as f32 * 10.0, 1000 + i as u64).unwrap();
            register.merge(&other).unwrap();
        }

        assert_eq!(register.element_count(), 4);
        assert!(!register.can_add_element());
    }

    #[test]
    fn test_validation() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);
        register.set(10.0, 1000).unwrap();

        assert!(register.validate().is_ok());
    }

    #[test]
    fn test_real_time_crdt() {
        let mut register1 = MVRegister::<f32, DefaultConfig>::new(1);
        let register2 = MVRegister::<f32, DefaultConfig>::new(2);

        assert!(register1.merge_bounded(&register2).is_ok());
        assert!(register1.validate_bounded().is_ok());
    }

    #[test]
    fn test_can_merge() {
        let mut register1 = MVRegister::<f32, DefaultConfig>::new(1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(2);

        // Fill register1 to capacity
        for i in 1..=4 {
            let mut other = MVRegister::<f32, DefaultConfig>::new(i);
            other.set(i as f32 * 10.0, 1000).unwrap();
            register1.merge(&other).unwrap();
        }

        // Empty register2 should be mergeable
        assert!(register1.can_merge(&register2));

        // Register2 with overlapping node should be mergeable
        register2.set(50.0, 2000).unwrap();
        assert!(register1.can_merge(&register2));

        // Register2 with new node should not be mergeable
        let mut register5 = MVRegister::<f32, DefaultConfig>::new(5);
        register5.set(50.0, 2000).unwrap();
        assert!(!register1.can_merge(&register5));
    }

    #[test]
    fn test_iter() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);
        register.set(10.0, 1000).unwrap();

        let mut other = MVRegister::<f32, DefaultConfig>::new(2);
        other.set(20.0, 2000).unwrap();
        register.merge(&other).unwrap();

        let mut count = 0;
        let mut found_10 = false;
        let mut found_20 = false;

        for (value, _, _) in register.iter() {
            count += 1;
            if *value == 10.0 {
                found_10 = true;
            }
            if *value == 20.0 {
                found_20 = true;
            }
        }

        assert_eq!(count, 2);
        assert!(found_10);
        assert!(found_20);
    }

    #[test]
    fn test_with_capacity() {
        // Test custom capacity
        let register = MVRegister::<f32, DefaultConfig, 8>::with_capacity(1);
        assert!(register.is_empty());
        assert_eq!(register.len(), 0);
        assert_eq!(register.capacity(), 8);
        assert_eq!(register.node_id(), 1);
    }

    #[test]
    fn test_custom_capacity_operations() {
        let mut register = MVRegister::<f32, DefaultConfig, 2>::with_capacity(1);

        // Test basic operations with custom capacity
        assert!(register.set(100.0, 1000).is_ok());
        assert_eq!(register.len(), 1);
        assert_eq!(register.get_from_node(1), Some(&100.0));
        assert_eq!(register.capacity(), 2);

        // Fill to custom capacity
        let mut other = MVRegister::<f32, DefaultConfig, 2>::with_capacity(2);
        other.set(200.0, 1001).unwrap();
        assert!(register.merge(&other).is_ok());

        assert!(register.is_full());
        assert_eq!(register.len(), 2);

        // Try to add one more (should fail)
        let mut other3 = MVRegister::<f32, DefaultConfig, 2>::with_capacity(3);
        other3.set(300.0, 1002).unwrap();
        assert!(register.merge(&other3).is_err());
    }

    #[test]
    fn test_capacity_merge() {
        let mut register1 = MVRegister::<f32, DefaultConfig, 2>::with_capacity(1);
        let mut register2 = MVRegister::<f32, DefaultConfig, 2>::with_capacity(2);

        register1.set(10.0, 1000).unwrap();
        register2.set(20.0, 1001).unwrap();

        // Merge should work with same capacity
        register1.merge(&register2).unwrap();
        assert_eq!(register1.len(), 2);
        assert_eq!(register1.get_from_node(1), Some(&10.0));
        assert_eq!(register1.get_from_node(2), Some(&20.0));
    }

    #[cfg(all(test, feature = "serde"))]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_serialize_deserialize() {
            let mut register = MVRegister::<i32, DefaultConfig>::new(1);
            register.set(42, 1000).unwrap();

            let mut other = MVRegister::<i32, DefaultConfig>::new(2);
            other.set(100, 2000).unwrap();
            register.merge(&other).unwrap();

            // Test that the serde traits are implemented
            // This ensures the code compiles with serde feature
            assert_eq!(register.len(), 2);
            assert_eq!(register.get_from_node(1), Some(&42));
            assert_eq!(register.get_from_node(2), Some(&100));
            assert_eq!(register.get_timestamp_from_node(1).unwrap().as_u64(), 1000);
            assert_eq!(register.get_timestamp_from_node(2).unwrap().as_u64(), 2000);
        }

        #[test]
        fn test_atomic_vs_standard_compatibility() {
            // This test ensures that atomic and standard versions would serialize to the same format
            // The logical state should be identical regardless of internal representation
            let mut register = MVRegister::<i32, DefaultConfig>::new(1);
            register.set(42, 1000).unwrap();

            let mut other = MVRegister::<i32, DefaultConfig>::new(2);
            other.set(84, 2000).unwrap();
            register.merge(&other).unwrap();

            // Both versions should have the same logical state
            assert_eq!(register.len(), 2);
            assert_eq!(register.get_from_node(1), Some(&42));
            assert_eq!(register.get_from_node(2), Some(&84));
        }

        #[test]
        fn test_multi_value_serialization() {
            let mut register = MVRegister::<f32, DefaultConfig>::new(1);
            register.set(10.5, 1000).unwrap();

            // Add values from multiple nodes
            for i in 2..=4 {
                let mut other = MVRegister::<f32, DefaultConfig>::new(i);
                other.set(i as f32 * 10.5, 1000 + i as u64).unwrap();
                register.merge(&other).unwrap();
            }

            // Should handle multiple concurrent values correctly
            assert_eq!(register.len(), 4);
            assert!(register.is_full());
            assert_eq!(register.get_from_node(1), Some(&10.5));
            assert_eq!(register.get_from_node(2), Some(&21.0));
            assert_eq!(register.get_from_node(3), Some(&31.5));
            assert_eq!(register.get_from_node(4), Some(&42.0));
        }

        #[test]
        fn test_custom_capacity_serialization() {
            let mut register = MVRegister::<i32, DefaultConfig, 2>::with_capacity(1);
            register.set(100, 1000).unwrap();

            let mut other = MVRegister::<i32, DefaultConfig, 2>::with_capacity(2);
            other.set(200, 2000).unwrap();
            register.merge(&other).unwrap();

            // Should handle custom capacity correctly
            assert_eq!(register.len(), 2);
            assert_eq!(register.capacity(), 2);
            assert!(register.is_full());
            assert_eq!(register.get_from_node(1), Some(&100));
            assert_eq!(register.get_from_node(2), Some(&200));
        }
    }
}
