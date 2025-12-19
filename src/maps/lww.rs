//! Last-Writer-Wins Map CRDT
//!
//! A map that resolves conflicts by keeping the value with the latest timestamp for each key.
//! Uses zero allocation with a fixed array for deterministic memory usage.

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

/// Last-Writer-Wins Map with configurable entry array
///
/// This map resolves conflicts by keeping the value with the latest timestamp
/// for each key. All memory is statically allocated using embedded arrays.
///
/// # Type Parameters
/// - `K`: The key type
/// - `V`: The value type
/// - `C`: Memory configuration that determines the default maximum number of entries
/// - `CAPACITY`: The maximum number of entries this map can hold (defaults to 8)
///
/// # Memory Usage
/// - Fixed size: (sizeof(K) + sizeof(V) + 9) * CAPACITY + 8 bytes
/// - Example: For (u16, u32) with 8 entries = ~112 bytes, with 16 entries = ~208 bytes
/// - Completely predictable at compile time
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
///
/// // Create maps for device configuration with default capacity
/// let mut config1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
/// config1.insert(1, 100, 1000)?; // Setting 1 = 100
/// config1.insert(2, 200, 1001)?; // Setting 2 = 200
///
/// let mut config2 = LWWMap::<u8, u32, DefaultConfig>::new(2);
/// config2.insert(2, 250, 1005)?; // Setting 2 = 250 (newer)
/// config2.insert(3, 300, 1003)?; // Setting 3 = 300
///
/// // Merge the maps
/// config1.merge(&config2)?;
/// assert_eq!(config1.get(&1), Some(&100)); // Original value
/// assert_eq!(config1.get(&2), Some(&250)); // Newer value wins
/// assert_eq!(config1.get(&3), Some(&300)); // New entry
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug)]
pub struct LWWMap<K, V, C: MemoryConfig, const CAPACITY: usize = 8> {
    /// Entries in the map
    #[cfg(not(feature = "hardware-atomic"))]
    entries: [Option<Entry<K, V>>; CAPACITY],
    #[cfg(not(feature = "hardware-atomic"))]
    count: usize,

    /// Atomic version uses UnsafeCell for the entries array
    #[cfg(feature = "hardware-atomic")]
    entries: UnsafeCell<[Option<Entry<K, V>>; CAPACITY]>,
    #[cfg(feature = "hardware-atomic")]
    count: AtomicUsize,

    /// This node's ID
    node_id: NodeId,

    /// Phantom data to maintain the memory config type
    _phantom: core::marker::PhantomData<C>,
}

// SAFETY: The atomic version is safe to share between threads because:
// 1. All access to arrays is protected by atomic count coordination
// 2. Only one thread can successfully modify at a time via compare_exchange
// 3. UnsafeCell is only accessed after winning the atomic coordination
#[cfg(feature = "hardware-atomic")]
unsafe impl<K, V, C: MemoryConfig> Sync for LWWMap<K, V, C>
where
    K: Send,
    V: Send,
    C: Send + Sync,
{
}

// Implement Clone manually due to atomic types not implementing Clone
impl<K, V, C: MemoryConfig, const CAPACITY: usize> Clone for LWWMap<K, V, C, CAPACITY>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                entries: self.entries.clone(),
                count: self.count,
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to manually clone the UnsafeCell content
            let cloned_entries = unsafe { (*self.entries.get()).clone() };
            Self {
                entries: UnsafeCell::new(cloned_entries),
                count: AtomicUsize::new(self.count.load(Ordering::Relaxed)),
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

/// Map entry with timestamp and node ID for conflict resolution
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct Entry<K, V> {
    #[cfg_attr(feature = "serde", serde(rename = "k"))]
    key: K,
    #[cfg_attr(feature = "serde", serde(rename = "v"))]
    value: V,
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

// Serde implementation for LWWMap
#[cfg(feature = "serde")]
impl<K, V, C: MemoryConfig, const CAPACITY: usize> Serialize for LWWMap<K, V, C, CAPACITY>
where
    K: Serialize + Clone + PartialEq,
    V: Serialize + Clone + PartialEq,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("LWWMap", 3)?;

        // Serialize the logical state (entries array, count, and node_id)
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Serialize only the used portion of the array as a slice
            state.serialize_field("en", &&self.entries[..self.count])?;
            state.serialize_field("ct", &self.count)?;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to extract values safely
            let current_count = self.count.load(Ordering::Relaxed);
            let entries_ref = unsafe { &*self.entries.get() };
            state.serialize_field("en", &&entries_ref[..current_count])?;
            state.serialize_field("ct", &current_count)?;
        }

        state.serialize_field("n", &self.node_id)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, K, V, C: MemoryConfig, const CAPACITY: usize> Deserialize<'de>
    for LWWMap<K, V, C, CAPACITY>
where
    K: Deserialize<'de> + Clone + PartialEq,
    V: Deserialize<'de> + Clone + PartialEq,
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
            #[serde(rename = "en")]
            Entries,
            #[serde(rename = "ct")]
            Count,
            #[serde(rename = "n")]
            NodeId,
        }

        struct LWWMapVisitor<K, V, C: MemoryConfig, const CAPACITY: usize> {
            _phantom: core::marker::PhantomData<(K, V, C)>,
        }

        impl<'de, K, V, C: MemoryConfig, const CAPACITY: usize> Visitor<'de>
            for LWWMapVisitor<K, V, C, CAPACITY>
        where
            K: Deserialize<'de> + Clone + PartialEq,
            V: Deserialize<'de> + Clone + PartialEq,
        {
            type Value = LWWMap<K, V, C, CAPACITY>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct LWWMap")
            }

            fn visit_map<A>(self, mut map: A) -> Result<LWWMap<K, V, C, CAPACITY>, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut entries = None;
                let mut count = None;
                let mut node_id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Entries => {
                            if entries.is_some() {
                                return Err(de::Error::duplicate_field("en"));
                            }
                            // Use a custom deserializer that doesn't require Vec
                            use serde::de::SeqAccess;

                            struct EntriesDeserializer<K, V, const CAPACITY: usize> {
                                _phantom: core::marker::PhantomData<(K, V)>,
                            }

                            impl<'de, K, V, const CAPACITY: usize> serde::de::DeserializeSeed<'de>
                                for EntriesDeserializer<K, V, CAPACITY>
                            where
                                K: Deserialize<'de>,
                                V: Deserialize<'de>,
                            {
                                type Value = [Option<Entry<K, V>>; CAPACITY];

                                fn deserialize<D>(
                                    self,
                                    deserializer: D,
                                ) -> Result<Self::Value, D::Error>
                                where
                                    D: serde::de::Deserializer<'de>,
                                {
                                    struct EntriesVisitor<K, V, const CAPACITY: usize> {
                                        _phantom: core::marker::PhantomData<(K, V)>,
                                    }

                                    impl<'de, K, V, const CAPACITY: usize> serde::de::Visitor<'de> for EntriesVisitor<K, V, CAPACITY>
                                    where
                                        K: Deserialize<'de>,
                                        V: Deserialize<'de>,
                                    {
                                        type Value = [Option<Entry<K, V>>; CAPACITY];

                                        fn expecting(
                                            &self,
                                            formatter: &mut core::fmt::Formatter,
                                        ) -> core::fmt::Result
                                        {
                                            write!(
                                                formatter,
                                                "a sequence of at most {} entries",
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

                                            while let Some(entry) =
                                                seq.next_element::<Option<Entry<K, V>>>()?
                                            {
                                                if index >= CAPACITY {
                                                    return Err(serde::de::Error::custom(
                                                        "too many entries for capacity",
                                                    ));
                                                }
                                                array[index] = entry;
                                                index += 1;
                                            }

                                            Ok(array)
                                        }
                                    }

                                    deserializer.deserialize_seq(EntriesVisitor::<K, V, CAPACITY> {
                                        _phantom: core::marker::PhantomData,
                                    })
                                }
                            }

                            let entries_array =
                                map.next_value_seed(EntriesDeserializer::<K, V, CAPACITY> {
                                    _phantom: core::marker::PhantomData,
                                })?;
                            entries = Some(entries_array);
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

                let entries_array = entries.ok_or_else(|| de::Error::missing_field("en"))?;
                let count = count.ok_or_else(|| de::Error::missing_field("ct"))?;
                let node_id = node_id.ok_or_else(|| de::Error::missing_field("n"))?;

                // Validate count is within capacity
                if count > CAPACITY {
                    return Err(de::Error::custom("count exceeds capacity"));
                }

                // Reconstruct the LWWMap
                #[cfg(not(feature = "hardware-atomic"))]
                {
                    Ok(LWWMap {
                        entries: entries_array,
                        count,
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }

                #[cfg(feature = "hardware-atomic")]
                {
                    Ok(LWWMap {
                        entries: UnsafeCell::new(entries_array),
                        count: AtomicUsize::new(count),
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }
            }
        }

        const FIELDS: &[&str] = &["en", "ct", "n"];
        deserializer.deserialize_struct(
            "LWWMap",
            FIELDS,
            LWWMapVisitor {
                _phantom: core::marker::PhantomData,
            },
        )
    }
}

impl<K, V, C: MemoryConfig, const CAPACITY: usize> LWWMap<K, V, C, CAPACITY>
where
    K: Clone + PartialEq,
    V: Clone + PartialEq,
{
    /// Creates a new LWW map for the given node with custom capacity
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node (must be < MAX_NODES)
    ///
    /// # Returns
    /// A new empty map
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let map = LWWMap::<u8, u32, DefaultConfig, 16>::with_capacity(1);
    /// assert!(map.is_empty());
    /// ```
    pub fn with_capacity(node_id: NodeId) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                entries: [const { None }; CAPACITY],
                count: 0,
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            Self {
                entries: UnsafeCell::new([const { None }; CAPACITY]),
                count: AtomicUsize::new(0),
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<K, V, C: MemoryConfig> LWWMap<K, V, C, 8>
where
    K: Clone + PartialEq,
    V: Clone + PartialEq,
{
    /// Creates a new LWW map for the given node with default capacity
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node (must be < MAX_NODES)
    ///
    /// # Returns
    /// A new empty map
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let map = LWWMap::<u8, u32, DefaultConfig>::new(1);
    /// assert!(map.is_empty());
    /// ```
    pub fn new(node_id: NodeId) -> Self {
        Self::with_capacity(node_id)
    }
}

impl<K, V, C: MemoryConfig, const CAPACITY: usize> LWWMap<K, V, C, CAPACITY>
where
    K: Clone + PartialEq,
    V: Clone + PartialEq,
{
    /// Inserts or updates a key-value pair with the given timestamp
    ///
    /// # Arguments
    /// * `key` - The key to insert/update
    /// * `value` - The value to associate with the key
    /// * `timestamp` - The timestamp for this update
    ///
    /// # Returns
    /// Ok(true) if this was a new key, Ok(false) if an existing key was updated,
    /// or an error if the operation failed
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
    /// assert!(map.insert(1, 100, 1000)?);  // New key
    /// assert!(!map.insert(1, 200, 2000)?); // Updated existing key
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn insert(&mut self, key: K, value: V, timestamp: u64) -> CRDTResult<bool> {
        let new_timestamp = CompactTimestamp::new(timestamp);

        // Check if key already exists
        for i in 0..self.count {
            if let Some(entry) = &mut self.entries[i] {
                if entry.key == key {
                    // Key exists, check if we should update
                    let should_update = if new_timestamp > entry.timestamp {
                        true // Newer timestamp always wins
                    } else if new_timestamp == entry.timestamp {
                        if self.node_id == entry.node_id {
                            true // Same node: last write wins
                        } else {
                            self.node_id > entry.node_id // Different nodes: higher node ID wins
                        }
                    } else {
                        false // Older timestamp loses
                    };

                    if should_update {
                        entry.value = value;
                        entry.timestamp = new_timestamp;
                        entry.node_id = self.node_id;
                    }
                    return Ok(false); // Existing key
                }
            }
        }

        // New key - check if we have space
        if self.count >= CAPACITY {
            return Err(CRDTError::BufferOverflow);
        }

        // Insert new entry
        self.entries[self.count] = Some(Entry {
            key,
            value,
            timestamp: new_timestamp,
            node_id: self.node_id,
        });
        self.count += 1;
        Ok(true)
    }

    /// Inserts or updates a key-value pair with the given timestamp (atomic version)
    ///
    /// # Arguments
    /// * `key` - The key to insert/update
    /// * `value` - The value to associate with the key
    /// * `timestamp` - The timestamp for this update
    ///
    /// # Returns
    /// Ok(true) if this was a new key, Ok(false) if an existing key was updated,
    /// or an error if the operation failed
    #[cfg(feature = "hardware-atomic")]
    pub fn insert(&self, key: K, value: V, timestamp: u64) -> CRDTResult<bool> {
        let new_timestamp = CompactTimestamp::new(timestamp);

        // First, try to update existing key
        let current_count = self.count.load(Ordering::Relaxed);
        let entries_ptr = self.entries.get();
        let entries_mut = unsafe { &mut *entries_ptr };

        // Check if key already exists and update if needed
        for i in 0..current_count {
            if let Some(entry) = &mut entries_mut[i] {
                if entry.key == key {
                    // Key exists, check if we should update
                    let should_update = if new_timestamp > entry.timestamp {
                        true // Newer timestamp always wins
                    } else if new_timestamp == entry.timestamp {
                        if self.node_id == entry.node_id {
                            true // Same node: last write wins
                        } else {
                            self.node_id > entry.node_id // Different nodes: higher node ID wins
                        }
                    } else {
                        false // Older timestamp loses
                    };

                    if should_update {
                        entry.value = value;
                        entry.timestamp = new_timestamp;
                        entry.node_id = self.node_id;
                    }
                    return Ok(false); // Existing key
                }
            }
        }

        // New key - atomic compare-exchange loop for coordination
        loop {
            let current_count = self.count.load(Ordering::Relaxed);

            // Double-check that key doesn't exist (race condition protection)
            let entries_ref = unsafe { &*entries_ptr };
            for i in 0..current_count {
                if let Some(entry) = &entries_ref[i] {
                    if entry.key == key {
                        // Key was added by another thread, try to update it
                        let entries_mut = unsafe { &mut *entries_ptr };
                        if let Some(entry) = &mut entries_mut[i] {
                            let should_update = new_timestamp > entry.timestamp
                                || (new_timestamp == entry.timestamp
                                    && self.node_id > entry.node_id);
                            if should_update {
                                entry.value = value;
                                entry.timestamp = new_timestamp;
                                entry.node_id = self.node_id;
                            }
                        }
                        return Ok(false);
                    }
                }
            }

            // New key - check if we have space
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
                    // Successfully reserved slot, now insert the entry
                    let entries_mut = unsafe { &mut *entries_ptr };
                    entries_mut[current_count] = Some(Entry {
                        key,
                        value,
                        timestamp: new_timestamp,
                        node_id: self.node_id,
                    });
                    return Ok(true);
                }
                Err(_) => {
                    // Count changed, retry the loop
                    continue;
                }
            }
        }
    }

    /// Gets the value for a key
    ///
    /// # Arguments
    /// * `key` - The key to look up
    ///
    /// # Returns
    /// The value associated with the key, or None if the key doesn't exist
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
    /// map.insert(1, 100, 1000)?;
    /// assert_eq!(map.get(&1), Some(&100));
    /// assert_eq!(map.get(&2), None);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn get(&self, key: &K) -> Option<&V> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            for entry in self.entries.iter().take(self.count) {
                if let Some(entry) = entry {
                    if entry.key == *key {
                        return Some(&entry.value);
                    }
                }
            }
            None
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let entries_ref = unsafe { &*self.entries.get() };
            for entry in entries_ref.iter().take(current_count) {
                if let Some(entry) = entry {
                    if entry.key == *key {
                        return Some(&entry.value);
                    }
                }
            }
            None
        }
    }

    /// Gets the timestamp for a key
    ///
    /// # Arguments
    /// * `key` - The key to look up
    ///
    /// # Returns
    /// The timestamp of the value, or None if the key doesn't exist
    pub fn get_timestamp(&self, key: &K) -> Option<CompactTimestamp> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            for entry in self.entries.iter().take(self.count) {
                if let Some(entry) = entry {
                    if entry.key == *key {
                        return Some(entry.timestamp);
                    }
                }
            }
            None
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let entries_ref = unsafe { &*self.entries.get() };
            for entry in entries_ref.iter().take(current_count) {
                if let Some(entry) = entry {
                    if entry.key == *key {
                        return Some(entry.timestamp);
                    }
                }
            }
            None
        }
    }

    /// Gets the node ID that last updated a key
    ///
    /// # Arguments
    /// * `key` - The key to look up
    ///
    /// # Returns
    /// The node ID that last updated this key, or None if the key doesn't exist
    pub fn get_node_id(&self, key: &K) -> Option<NodeId> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            for entry in self.entries.iter().take(self.count) {
                if let Some(entry) = entry {
                    if entry.key == *key {
                        return Some(entry.node_id);
                    }
                }
            }
            None
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let entries_ref = unsafe { &*self.entries.get() };
            for entry in entries_ref.iter().take(current_count) {
                if let Some(entry) = entry {
                    if entry.key == *key {
                        return Some(entry.node_id);
                    }
                }
            }
            None
        }
    }

    /// Checks if the map contains a key
    ///
    /// # Arguments
    /// * `key` - The key to check for
    ///
    /// # Returns
    /// true if the key exists in the map, false otherwise
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
    /// map.insert(1, 100, 1000)?;
    /// assert!(map.contains_key(&1));
    /// assert!(!map.contains_key(&2));
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Returns the number of key-value pairs in the map
    ///
    /// # Returns
    /// The count of entries
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
    /// assert_eq!(map.len(), 0);
    /// map.insert(1, 100, 1000)?;
    /// assert_eq!(map.len(), 1);
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

    /// Checks if the map is empty
    ///
    /// # Returns
    /// true if the map contains no entries, false otherwise
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
    /// assert!(map.is_empty());
    /// map.insert(1, 100, 1000)?;
    /// assert!(!map.is_empty());
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Checks if the map is full
    ///
    /// # Returns
    /// true if the map cannot accept more entries, false otherwise
    pub fn is_full(&self) -> bool {
        self.len() >= CAPACITY
    }

    /// Returns the maximum capacity of the map
    ///
    /// # Returns
    /// The maximum number of entries this map can hold
    pub fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Returns the remaining capacity
    ///
    /// # Returns
    /// The number of additional entries that can be inserted
    pub fn remaining_capacity(&self) -> usize {
        CAPACITY - self.len()
    }

    /// Gets this node's ID
    ///
    /// # Returns
    /// The node ID of this map
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Returns an iterator over the key-value pairs
    ///
    /// # Returns
    /// An iterator over (key, value) pairs
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.entries
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| (&entry.key, &entry.value)))
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let entries_ref = unsafe { &*self.entries.get() };
            entries_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| (&entry.key, &entry.value)))
        }
    }

    /// Returns an iterator over the keys
    ///
    /// # Returns
    /// An iterator over keys
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.entries
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| &entry.key))
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let entries_ref = unsafe { &*self.entries.get() };
            entries_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| &entry.key))
        }
    }

    /// Returns an iterator over the values
    ///
    /// # Returns
    /// An iterator over values
    pub fn values(&self) -> impl Iterator<Item = &V> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.entries
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref().map(|entry| &entry.value))
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let entries_ref = unsafe { &*self.entries.get() };
            entries_ref
                .iter()
                .take(current_count)
                .filter_map(|opt| opt.as_ref().map(|entry| &entry.value))
        }
    }

    /// Removes a key from the map and returns the associated value
    ///
    /// # Arguments
    /// * `key` - The key to remove
    ///
    /// # Returns
    /// The value that was associated with the key, or None if the key wasn't present
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
    /// map.insert(1, 100, 1000)?;
    /// assert_eq!(map.remove(&1), Some(100));
    /// assert!(!map.contains_key(&1));
    /// assert_eq!(map.remove(&1), None); // Key no longer exists
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn remove(&mut self, key: &K) -> Option<V> {
        // Find the key in the entries
        for i in 0..self.count {
            if let Some(entry) = &self.entries[i] {
                if entry.key == *key {
                    // Extract the value to return
                    let removed_value = entry.value.clone();

                    // Shift all subsequent entries left to fill the gap
                    for j in i..(self.count - 1) {
                        self.entries[j] = self.entries[j + 1].take();
                    }

                    // Clear the last entry and decrement count
                    self.entries[self.count - 1] = None;
                    self.count -= 1;

                    return Some(removed_value);
                }
            }
        }
        None
    }

    /// Removes a key from the map and returns the associated value (atomic version)
    ///
    /// # Arguments
    /// * `key` - The key to remove
    ///
    /// # Returns
    /// The value that was associated with the key, or None if the key wasn't present
    #[cfg(feature = "hardware-atomic")]
    pub fn remove(&self, key: &K) -> Option<V> {
        // For atomic version, we need to coordinate access
        let entries_ptr = self.entries.get();
        let entries_mut = unsafe { &mut *entries_ptr };

        loop {
            let current_count = self.count.load(Ordering::Relaxed);

            // Find the key in the entries and extract needed data
            let mut found_index = None;
            let mut removed_value = None;
            let mut entry_timestamp = None;
            let mut entry_node_id = None;

            for i in 0..current_count {
                if let Some(entry) = &entries_mut[i] {
                    if entry.key == *key {
                        found_index = Some(i);
                        removed_value = Some(entry.value.clone());
                        entry_timestamp = Some(entry.timestamp);
                        entry_node_id = Some(entry.node_id);
                        break;
                    }
                }
            }

            if let Some(i) = found_index {
                let removed_val = removed_value.unwrap();

                // Shift all subsequent entries left to fill the gap
                for j in i..(current_count - 1) {
                    entries_mut[j] = entries_mut[j + 1].take();
                }

                // Clear the last entry
                entries_mut[current_count - 1] = None;

                // Try to atomically decrement count
                match self.count.compare_exchange_weak(
                    current_count,
                    current_count - 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return Some(removed_val),
                    Err(_) => {
                        // Count changed, restore the entry and retry
                        // This is a rare race condition - another thread modified the map
                        // We need to restore the state and retry
                        for j in (i + 1..current_count).rev() {
                            entries_mut[j] = entries_mut[j - 1].take();
                        }
                        entries_mut[i] = Some(Entry {
                            key: key.clone(),
                            value: removed_val,
                            timestamp: entry_timestamp.unwrap(),
                            node_id: entry_node_id.unwrap(),
                        });
                        continue; // Retry the whole operation
                    }
                }
            } else {
                // Key not found
                return None;
            }
        }
    }
}

impl<K, V, C: MemoryConfig, const CAPACITY: usize> CRDT<C> for LWWMap<K, V, C, CAPACITY>
where
    K: Clone + PartialEq + core::fmt::Debug,
    V: Clone + PartialEq + core::fmt::Debug,
{
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Process each entry from other
            for other_entry in other.entries.iter().take(other.count) {
                if let Some(other_entry) = other_entry {
                    // Check if we have this key
                    let mut found = false;
                    for i in 0..self.count {
                        if let Some(our_entry) = &mut self.entries[i] {
                            if our_entry.key == other_entry.key {
                                found = true;
                                // Key exists, check if we should update
                                let should_update = if other_entry.timestamp > our_entry.timestamp {
                                    true // Newer timestamp always wins
                                } else if other_entry.timestamp == our_entry.timestamp {
                                    if other_entry.node_id == our_entry.node_id {
                                        true // Same node: last write wins
                                    } else {
                                        other_entry.node_id > our_entry.node_id // Different nodes: higher node ID wins
                                    }
                                } else {
                                    false // Older timestamp loses
                                };
                                if should_update {
                                    our_entry.value = other_entry.value.clone();
                                    our_entry.timestamp = other_entry.timestamp;
                                    our_entry.node_id = other_entry.node_id;
                                }
                                break;
                            }
                        }
                    }

                    if !found {
                        // New key - check if we have space
                        if self.count >= CAPACITY {
                            return Err(CRDTError::BufferOverflow);
                        }

                        // Insert new entry
                        self.entries[self.count] = Some(Entry {
                            key: other_entry.key.clone(),
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
            // For atomic version, merge requires &mut self so it's not thread-safe during merge
            // But we can still implement the same logic using unsafe access to the UnsafeCell
            let other_count = other.count.load(Ordering::Relaxed);
            let other_entries_ref = unsafe { &*other.entries.get() };

            let self_entries_mut = unsafe { &mut *self.entries.get() };
            let mut self_count = self.count.load(Ordering::Relaxed);

            // Process each entry from other
            for other_entry in other_entries_ref.iter().take(other_count) {
                if let Some(other_entry) = other_entry {
                    // Check if we have this key
                    let mut found = false;
                    for i in 0..self_count {
                        if let Some(our_entry) = &mut self_entries_mut[i] {
                            if our_entry.key == other_entry.key {
                                found = true;
                                // Key exists, check if we should update
                                let should_update = if other_entry.timestamp > our_entry.timestamp {
                                    true // Newer timestamp always wins
                                } else if other_entry.timestamp == our_entry.timestamp {
                                    if other_entry.node_id == our_entry.node_id {
                                        true // Same node: last write wins
                                    } else {
                                        other_entry.node_id > our_entry.node_id // Different nodes: higher node ID wins
                                    }
                                } else {
                                    false // Older timestamp loses
                                };
                                if should_update {
                                    our_entry.value = other_entry.value.clone();
                                    our_entry.timestamp = other_entry.timestamp;
                                    our_entry.node_id = other_entry.node_id;
                                }
                                break;
                            }
                        }
                    }

                    if !found {
                        // New key - check if we have space
                        if self_count >= CAPACITY {
                            return Err(CRDTError::BufferOverflow);
                        }

                        // Insert new entry
                        self_entries_mut[self_count] = Some(Entry {
                            key: other_entry.key.clone(),
                            value: other_entry.value.clone(),
                            timestamp: other_entry.timestamp,
                            node_id: other_entry.node_id,
                        });
                        self_count += 1;
                    }
                }
            }

            // Update the atomic count
            self.count.store(self_count, Ordering::Relaxed);
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            if self.count != other.count {
                return false;
            }

            // Check that all entries match
            for entry in self.entries.iter().take(self.count) {
                if let Some(entry) = entry {
                    if let Some(other_value) = other.get(&entry.key) {
                        if entry.value != *other_value {
                            return false;
                        }
                        // Also check timestamps for exact equality
                        if let Some(other_timestamp) = other.get_timestamp(&entry.key) {
                            if entry.timestamp != other_timestamp {
                                return false;
                            }
                        }
                        if let Some(other_node_id) = other.get_node_id(&entry.key) {
                            if entry.node_id != other_node_id {
                                return false;
                            }
                        }
                    } else {
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

            let self_entries_ref = unsafe { &*self.entries.get() };

            // Check that all entries match
            for entry in self_entries_ref.iter().take(self_count) {
                if let Some(entry) = entry {
                    if let Some(other_value) = other.get(&entry.key) {
                        if entry.value != *other_value {
                            return false;
                        }
                        // Also check timestamps for exact equality
                        if let Some(other_timestamp) = other.get_timestamp(&entry.key) {
                            if entry.timestamp != other_timestamp {
                                return false;
                            }
                        }
                        if let Some(other_node_id) = other.get_node_id(&entry.key) {
                            if entry.node_id != other_node_id {
                                return false;
                            }
                        }
                    } else {
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

            // Validate that we don't exceed the configured maximum entries
            if self.count > C::MAX_MAP_ENTRIES {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate no duplicate keys (this should never happen with correct implementation)
            for i in 0..self.count {
                if let Some(entry_i) = &self.entries[i] {
                    for j in (i + 1)..self.count {
                        if let Some(entry_j) = &self.entries[j] {
                            if entry_i.key == entry_j.key {
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
            let entries_ref = unsafe { &*self.entries.get() };

            // Validate count is within bounds
            if current_count > CAPACITY {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate that we don't exceed the configured maximum entries
            if current_count > C::MAX_MAP_ENTRIES {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate no duplicate keys (this should never happen with correct implementation)
            for i in 0..current_count {
                if let Some(entry_i) = &entries_ref[i] {
                    for j in (i + 1)..current_count {
                        if let Some(entry_j) = &entries_ref[j] {
                            if entry_i.key == entry_j.key {
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
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Simple hash based on entries (order-independent)
            let mut hash = 0u32;
            for entry in self.entries.iter().take(self.count) {
                if let Some(entry) = entry {
                    // This is a simplified hash - in practice you'd want a proper hash function
                    let key_ptr = &entry.key as *const K as usize;
                    let value_ptr = &entry.value as *const V as usize;
                    hash ^=
                        (key_ptr as u32) ^ (value_ptr as u32) ^ (entry.timestamp.as_u64() as u32);
                }
            }
            hash ^= self.count as u32;
            hash
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let entries_ref = unsafe { &*self.entries.get() };

            // Simple hash based on entries (order-independent)
            let mut hash = 0u32;
            for entry in entries_ref.iter().take(current_count) {
                if let Some(entry) = entry {
                    // This is a simplified hash - in practice you'd want a proper hash function
                    let key_ptr = &entry.key as *const K as usize;
                    let value_ptr = &entry.value as *const V as usize;
                    hash ^=
                        (key_ptr as u32) ^ (value_ptr as u32) ^ (entry.timestamp.as_u64() as u32);
                }
            }
            hash ^= current_count as u32;
            hash
        }
    }

    fn can_merge(&self, other: &Self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Check if merging would exceed capacity
            let mut new_keys = 0;
            for other_entry in other.entries.iter().take(other.count) {
                if let Some(other_entry) = other_entry {
                    if !self.contains_key(&other_entry.key) {
                        new_keys += 1;
                    }
                }
            }

            self.count + new_keys <= CAPACITY
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let self_count = self.count.load(Ordering::Relaxed);
            let other_count = other.count.load(Ordering::Relaxed);
            let other_entries_ref = unsafe { &*other.entries.get() };

            // Check if merging would exceed capacity
            let mut new_keys = 0;
            for other_entry in other_entries_ref.iter().take(other_count) {
                if let Some(other_entry) = other_entry {
                    if !self.contains_key(&other_entry.key) {
                        new_keys += 1;
                    }
                }
            }

            self_count + new_keys <= CAPACITY
        }
    }
}

impl<K, V, C: MemoryConfig, const CAPACITY: usize> BoundedCRDT<C> for LWWMap<K, V, C, CAPACITY>
where
    K: Clone + PartialEq + core::fmt::Debug,
    V: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = CAPACITY; // Maximum number of entries

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
        // LWWMaps can't be compacted without losing data
        // This is a no-op that returns 0 bytes freed
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        // For fixed-size arrays, only check element count, not memory usage
        self.element_count() < Self::MAX_ELEMENTS
    }
}

impl<K, V, C: MemoryConfig, const CAPACITY: usize> RealTimeCRDT<C> for LWWMap<K, V, C, CAPACITY>
where
    K: Clone + PartialEq + core::fmt::Debug,
    V: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_MERGE_CYCLES: u32 = 300; // Linear in number of entries, but with nested loops
    const MAX_VALIDATE_CYCLES: u32 = 150;
    const MAX_SERIALIZE_CYCLES: u32 = 200;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // LWWMap merge is bounded by the number of entries
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded by the number of entries
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
    fn test_new_map() {
        let map = LWWMap::<u8, u32, DefaultConfig>::new(1);
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert_eq!(map.capacity(), 8);
        assert_eq!(map.remaining_capacity(), 8);
        assert!(!map.is_full());
        assert_eq!(map.node_id(), 1);
    }

    #[test]
    fn test_insert_and_get() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Insert new key
        assert!(map.insert(1, 100, 1000).unwrap());
        assert_eq!(map.len(), 1);
        assert!(!map.is_empty());
        assert_eq!(map.get(&1), Some(&100));
        assert!(map.contains_key(&1));

        // Update existing key with newer timestamp
        assert!(!map.insert(1, 200, 2000).unwrap());
        assert_eq!(map.len(), 1); // Still 1 entry
        assert_eq!(map.get(&1), Some(&200));

        // Try to update with older timestamp (should be ignored)
        assert!(!map.insert(1, 300, 500).unwrap());
        assert_eq!(map.get(&1), Some(&200)); // Still 200

        // Insert another key
        assert!(map.insert(2, 400, 3000).unwrap());
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&2), Some(&400));
    }

    #[test]
    fn test_timestamps_and_node_ids() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        map.insert(1, 100, 1000).unwrap();

        assert_eq!(map.get_timestamp(&1).unwrap().as_u64(), 1000);
        assert_eq!(map.get_node_id(&1), Some(1));
        assert_eq!(map.get_timestamp(&2), None);
        assert_eq!(map.get_node_id(&2), None);
    }

    #[test]
    fn test_capacity_limits() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Fill the map to capacity
        for i in 0..8 {
            assert!(map.insert(i, i as u32 * 10, 1000 + i as u64).is_ok());
        }

        assert!(map.is_full());
        assert_eq!(map.remaining_capacity(), 0);

        // Try to insert one more (should fail)
        assert!(map.insert(8, 80, 2000).is_err());
    }

    #[test]
    fn test_iterators() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        map.insert(1, 10, 1000).unwrap();
        map.insert(3, 30, 1001).unwrap();
        map.insert(2, 20, 1002).unwrap();

        // Test key-value iterator
        let mut pairs = [(0u8, 0u32); 3];
        let mut i = 0;
        for (&k, &v) in map.iter() {
            pairs[i] = (k, v);
            i += 1;
        }
        pairs.sort_by_key(|&(k, _)| k);
        assert_eq!(pairs, [(1, 10), (2, 20), (3, 30)]);

        // Test keys iterator
        let mut keys = [0u8; 3];
        let mut i = 0;
        for &k in map.keys() {
            keys[i] = k;
            i += 1;
        }
        keys.sort();
        assert_eq!(keys, [1, 2, 3]);

        // Test values iterator
        let mut values = [0u32; 3];
        let mut i = 0;
        for &v in map.values() {
            values[i] = v;
            i += 1;
        }
        values.sort();
        assert_eq!(values, [10, 20, 30]);
    }

    #[test]
    fn test_merge() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

        map1.insert(1, 10, 1000).unwrap();
        map1.insert(2, 20, 1001).unwrap();

        map2.insert(2, 25, 2000).unwrap(); // Newer value for key 2
        map2.insert(3, 30, 2001).unwrap(); // New key

        // Before merge
        assert_eq!(map1.len(), 2);
        assert_eq!(map2.len(), 2);

        // Merge map2 into map1
        map1.merge(&map2).unwrap();

        assert_eq!(map1.len(), 3);
        assert_eq!(map1.get(&1), Some(&10)); // Unchanged
        assert_eq!(map1.get(&2), Some(&25)); // Updated to newer value
        assert_eq!(map1.get(&3), Some(&30)); // New entry
    }

    #[test]
    fn test_merge_tiebreaker() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

        map1.insert(1, 10, 1000).unwrap();
        map2.insert(1, 20, 1000).unwrap(); // Same timestamp, higher node ID

        map1.merge(&map2).unwrap();
        assert_eq!(map1.get(&1), Some(&20)); // Higher node ID wins
    }

    #[test]
    fn test_merge_overflow() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

        // Fill map1 to capacity
        for i in 0..8 {
            map1.insert(i, i as u32 * 10, 1000).unwrap();
        }

        // Add a different key to map2
        map2.insert(100, 1000, 2000).unwrap();

        // Merge should fail due to overflow
        assert!(map1.merge(&map2).is_err());
    }

    #[test]
    fn test_merge_idempotent() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

        map1.insert(1, 10, 1000).unwrap();

        // Multiple merges should be idempotent
        map1.merge(&map2).unwrap();
        let len1 = map1.len();

        map1.merge(&map2).unwrap();
        let len2 = map1.len();

        assert_eq!(len1, len2);
    }

    #[test]
    fn test_merge_commutative() {
        let mut map1a = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map1b = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);
        let mut map3 = LWWMap::<u8, u32, DefaultConfig>::new(3);

        map1a.insert(1, 10, 1000).unwrap();
        map1b.insert(1, 10, 1000).unwrap();
        map2.insert(2, 20, 2000).unwrap();
        map3.insert(3, 30, 3000).unwrap();

        // Merge in different orders
        map1a.merge(&map2).unwrap();
        map1a.merge(&map3).unwrap();

        map1b.merge(&map3).unwrap();
        map1b.merge(&map2).unwrap();

        // Results should be the same
        assert_eq!(map1a.len(), map1b.len());
        assert!(map1a.eq(&map1b));
    }

    #[test]
    fn test_bounded_crdt() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
        map.insert(1, 10, 1000).unwrap();

        assert_eq!(map.element_count(), 1);
        assert!(map.memory_usage() > 0);
        assert!(map.can_add_element());

        // Fill to capacity
        for i in 2..8 {
            map.insert(i, i as u32 * 10, 1000 + i as u64).unwrap();
        }

        assert_eq!(map.element_count(), 7);
        assert!(map.can_add_element());

        map.insert(8, 80, 2000).unwrap();
        assert_eq!(map.element_count(), 8);
        assert!(!map.can_add_element());
    }

    #[test]
    fn test_validation() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
        map.insert(1, 10, 1000).unwrap();

        assert!(map.validate().is_ok());
    }

    #[test]
    fn test_real_time_crdt() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

        assert!(map1.merge_bounded(&map2).is_ok());
        assert!(map1.validate_bounded().is_ok());
    }

    #[test]
    fn test_can_merge() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

        // Fill map1 to capacity
        for i in 0..8 {
            map1.insert(i, i as u32 * 10, 1000).unwrap();
        }

        // Empty map2 should be mergeable
        assert!(map1.can_merge(&map2));

        // Map2 with overlapping key should be mergeable
        map2.insert(5, 50, 2000).unwrap();
        assert!(map1.can_merge(&map2));

        // Map2 with new key should not be mergeable
        map2.insert(100, 1000, 3000).unwrap();
        assert!(!map1.can_merge(&map2));
    }

    #[test]
    fn test_eq() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Empty maps should be equal
        assert!(map1.eq(&map2));

        // Add same entries
        map1.insert(1, 10, 1000).unwrap();
        map2.insert(1, 10, 1000).unwrap();
        assert!(map1.eq(&map2));

        // Different values should not be equal
        map2.insert(2, 20, 1001).unwrap(); // Different key
        assert!(!map1.eq(&map2));
    }

    #[test]
    fn test_with_capacity() {
        // Test custom capacity
        let map = LWWMap::<u8, u32, DefaultConfig, 16>::with_capacity(1);
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert_eq!(map.capacity(), 16);
        assert_eq!(map.remaining_capacity(), 16);
        assert!(!map.is_full());
        assert_eq!(map.node_id(), 1);
    }

    #[test]
    fn test_custom_capacity_operations() {
        let mut map = LWWMap::<u8, u32, DefaultConfig, 4>::with_capacity(1);

        // Test basic operations with custom capacity
        assert!(map.insert(1, 100, 1000).is_ok());
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&1), Some(&100));
        assert_eq!(map.capacity(), 4);

        // Fill to custom capacity
        for i in 2..=4 {
            assert!(map.insert(i, i as u32 * 100, 1000 + i as u64).is_ok());
        }

        assert!(map.is_full());
        assert_eq!(map.remaining_capacity(), 0);

        // Try to insert one more (should fail)
        assert!(map.insert(5, 500, 2000).is_err());
    }

    #[test]
    fn test_capacity_merge() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig, 4>::with_capacity(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig, 4>::with_capacity(2);

        map1.insert(1, 10, 1000).unwrap();
        map1.insert(2, 20, 1001).unwrap();

        map2.insert(2, 25, 2000).unwrap(); // Newer value for key 2
        map2.insert(3, 30, 2001).unwrap(); // New key

        // Merge should work with same capacity
        map1.merge(&map2).unwrap();
        assert_eq!(map1.len(), 3);
        assert_eq!(map1.get(&1), Some(&10)); // Unchanged
        assert_eq!(map1.get(&2), Some(&25)); // Updated to newer value
        assert_eq!(map1.get(&3), Some(&30)); // New entry
    }

    #[cfg(all(test, feature = "serde"))]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_serialize_deserialize() {
            let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
            map.insert(1, 100, 1000).unwrap();
            map.insert(2, 200, 1500).unwrap();
            map.insert(1, 150, 2000).unwrap(); // Update key 1 with newer timestamp

            // Test that the serde traits are implemented
            // This ensures the code compiles with serde feature
            assert_eq!(map.len(), 2);
            assert_eq!(map.get(&1), Some(&150)); // Updated value
            assert_eq!(map.get(&2), Some(&200));
            assert_eq!(map.get_timestamp(&1).unwrap().as_u64(), 2000);
            assert_eq!(map.get_timestamp(&2).unwrap().as_u64(), 1500);
            assert_eq!(map.node_id(), 1);
        }

        #[test]
        fn test_atomic_vs_standard_compatibility() {
            // This test ensures that atomic and standard versions would serialize to the same format
            // The logical state should be identical regardless of internal representation
            let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);
            map.insert(1, 100, 1000).unwrap();
            map.insert(2, 200, 1500).unwrap();
            map.insert(3, 300, 2000).unwrap();

            // Both versions should have the same logical state
            assert_eq!(map.len(), 3);
            assert_eq!(map.get(&1), Some(&100));
            assert_eq!(map.get(&2), Some(&200));
            assert_eq!(map.get(&3), Some(&300));
        }

        #[test]
        fn test_empty_map_serialization() {
            let map = LWWMap::<u8, u32, DefaultConfig>::new(1);

            // Should handle empty map correctly
            assert_eq!(map.len(), 0);
            assert!(map.is_empty());
            assert_eq!(map.node_id(), 1);
        }

        #[test]
        fn test_lww_semantics_serialization() {
            let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
            let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

            // Complex LWW semantics: conflicts resolved by timestamp and node ID
            map1.insert(1, 100, 1000).unwrap();
            map1.insert(2, 200, 1500).unwrap();

            map2.insert(1, 150, 2000).unwrap(); // Newer timestamp for key 1
            map2.insert(2, 250, 1500).unwrap(); // Same timestamp, higher node ID
            map2.insert(3, 300, 1600).unwrap(); // New key

            map1.merge(&map2).unwrap();

            // Should handle LWW conflict resolution correctly
            assert_eq!(map1.len(), 3);
            assert_eq!(map1.get(&1), Some(&150)); // Newer timestamp wins
            assert_eq!(map1.get(&2), Some(&250)); // Higher node ID wins on tie
            assert_eq!(map1.get(&3), Some(&300)); // New key
            assert_eq!(map1.get_node_id(&1), Some(2)); // Updated by node 2
            assert_eq!(map1.get_node_id(&2), Some(2)); // Updated by node 2
            assert_eq!(map1.get_node_id(&3), Some(2)); // Added by node 2
        }

        #[test]
        fn test_custom_capacity_serialization() {
            let mut map = LWWMap::<u8, u32, DefaultConfig, 4>::with_capacity(1);
            map.insert(1, 100, 1000).unwrap();
            map.insert(2, 200, 1100).unwrap();
            map.insert(3, 300, 1200).unwrap();

            // Should handle custom capacity correctly
            assert_eq!(map.len(), 3);
            assert_eq!(map.capacity(), 4);
            assert_eq!(map.get(&1), Some(&100));
            assert_eq!(map.get(&2), Some(&200));
            assert_eq!(map.get(&3), Some(&300));
        }
    }

    #[test]
    fn test_remove_basic() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Insert some entries
        map.insert(1, 100, 1000).unwrap();
        map.insert(2, 200, 1001).unwrap();
        map.insert(3, 300, 1002).unwrap();
        assert_eq!(map.len(), 3);

        // Remove existing key
        assert_eq!(map.remove(&2), Some(200));
        assert_eq!(map.len(), 2);
        assert!(!map.contains_key(&2));
        assert_eq!(map.get(&2), None);

        // Other keys should still be present
        assert_eq!(map.get(&1), Some(&100));
        assert_eq!(map.get(&3), Some(&300));

        // Remove non-existent key
        assert_eq!(map.remove(&99), None);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_remove_and_reinsert() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Insert, remove, then re-insert
        map.insert(1, 100, 1000).unwrap();
        assert_eq!(map.len(), 1);

        let removed = map.remove(&1);
        assert_eq!(removed, Some(100));
        assert_eq!(map.len(), 0);
        assert!(!map.contains_key(&1));

        // Re-insert the same key
        map.insert(1, 200, 2000).unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&1), Some(&200));
    }

    #[test]
    fn test_remove_capacity_freed() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Fill to capacity
        for i in 0..8 {
            map.insert(i, i as u32 * 10, 1000 + i as u64).unwrap();
        }
        assert!(map.is_full());
        assert_eq!(map.remaining_capacity(), 0);

        // Remove one entry
        map.remove(&3);
        assert!(!map.is_full());
        assert_eq!(map.remaining_capacity(), 1);
        assert_eq!(map.len(), 7);

        // Should be able to insert a new entry
        map.insert(99, 990, 2000).unwrap();
        assert_eq!(map.len(), 8);
        assert!(map.contains_key(&99));
    }

    #[test]
    fn test_remove_all_entries() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Insert some entries
        map.insert(1, 100, 1000).unwrap();
        map.insert(2, 200, 1001).unwrap();
        map.insert(3, 300, 1002).unwrap();

        // Remove all entries
        assert_eq!(map.remove(&1), Some(100));
        assert_eq!(map.remove(&2), Some(200));
        assert_eq!(map.remove(&3), Some(300));

        // Map should be empty
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert_eq!(map.remaining_capacity(), 8);
    }

    #[test]
    fn test_remove_order_independence() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Insert same entries in both maps
        for i in 1..=5 {
            map1.insert(i, i as u32 * 10, 1000 + i as u64).unwrap();
            map2.insert(i, i as u32 * 10, 1000 + i as u64).unwrap();
        }

        // Remove in different orders
        map1.remove(&2);
        map1.remove(&4);

        map2.remove(&4);
        map2.remove(&2);

        // Both should have the same final state
        assert_eq!(map1.len(), map2.len());
        assert_eq!(map1.get(&1), map2.get(&1));
        assert_eq!(map1.get(&3), map2.get(&3));
        assert_eq!(map1.get(&5), map2.get(&5));
        assert!(!map1.contains_key(&2));
        assert!(!map1.contains_key(&4));
        assert!(!map2.contains_key(&2));
        assert!(!map2.contains_key(&4));
    }

    #[test]
    fn test_remove_with_custom_capacity() {
        let mut map = LWWMap::<u8, u32, DefaultConfig, 4>::with_capacity(1);

        // Fill custom capacity
        for i in 1..=4 {
            map.insert(i, i as u32 * 100, 1000 + i as u64).unwrap();
        }
        assert_eq!(map.len(), 4);
        assert!(map.is_full());

        // Remove middle entry
        assert_eq!(map.remove(&2), Some(200));
        assert_eq!(map.len(), 3);
        assert!(!map.is_full());

        // Verify remaining entries
        assert_eq!(map.get(&1), Some(&100));
        assert_eq!(map.get(&3), Some(&300));
        assert_eq!(map.get(&4), Some(&400));
        assert!(!map.contains_key(&2));
    }

    #[test]
    fn test_remove_empty_map() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Remove from empty map
        assert_eq!(map.remove(&1), None);
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
    }

    #[test]
    fn test_remove_single_entry() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Insert single entry and remove it
        map.insert(42, 420, 1000).unwrap();
        assert_eq!(map.len(), 1);

        assert_eq!(map.remove(&42), Some(420));
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert!(!map.contains_key(&42));
    }

    #[test]
    fn test_remove_preserves_order() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Insert entries
        map.insert(1, 10, 1000).unwrap();
        map.insert(2, 20, 1001).unwrap();
        map.insert(3, 30, 1002).unwrap();
        map.insert(4, 40, 1003).unwrap();

        // Remove middle entry
        map.remove(&2);

        // Collect remaining entries
        let mut entries = [(&0u8, &0u32); 4];
        let mut count = 0;
        for (k, v) in map.iter() {
            entries[count] = (k, v);
            count += 1;
        }

        // Sort the collected entries
        entries[..count].sort_by_key(|&(k, _)| k);

        // Should have entries 1, 3, 4 in order
        assert_eq!(count, 3);
        assert_eq!(entries[0], (&1, &10));
        assert_eq!(entries[1], (&3, &30));
        assert_eq!(entries[2], (&4, &40));
    }
}
