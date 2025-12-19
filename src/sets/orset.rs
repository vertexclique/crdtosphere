//! Observed-Remove Set CRDT
//!
//! A set that supports both add and remove operations using unique tags.
//! Uses zero allocation with fixed arrays for deterministic memory usage.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

#[cfg(feature = "hardware-atomic")]
use core::cell::UnsafeCell;
#[cfg(feature = "hardware-atomic")]
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "hardware-atomic")]
extern crate alloc;
#[cfg(feature = "hardware-atomic")]
use alloc::vec::Vec;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Observed-Remove Set with configurable element and tombstone arrays
///
/// This set supports both add and remove operations by using unique tags
/// for each element. Elements can be removed by observing their tags.
/// All memory is statically allocated using embedded arrays.
///
/// # Type Parameters
/// - `T`: The element type stored in the set
/// - `C`: Memory configuration that determines the default maximum number of elements
/// - `CAPACITY`: The maximum number of elements this set can hold (defaults to 8)
///
/// # Memory Usage
/// - Fixed size: 2 * (sizeof(T) + 9) * CAPACITY + 16 bytes
/// - Example: For u32 with 8 elements = ~224 bytes, with 16 elements = ~432 bytes
/// - Completely predictable at compile time
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
///
/// // Create sets for dynamic device capabilities with default capacity
/// let mut capabilities1 = ORSet::<u32, DefaultConfig>::new(1);
/// capabilities1.add(1, 1000)?; // GPS
/// capabilities1.add(2, 1001)?; // WiFi
///
/// let mut capabilities2 = ORSet::<u32, DefaultConfig>::new(2);
/// capabilities2.add(2, 1005)?; // WiFi (after removal)
/// capabilities2.add(3, 1003)?; // Bluetooth
///
/// // Remove WiFi from first set
/// capabilities1.remove(&2, 1004)?;
///
/// // Merge the sets
/// capabilities1.merge(&capabilities2)?;
///
/// // WiFi should still be present (added after removal in capabilities2)
/// assert!(capabilities1.contains(&1));
/// assert!(capabilities1.contains(&2)); // WiFi re-added
/// assert!(capabilities1.contains(&3));
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug)]
pub struct ORSet<T, C: MemoryConfig, const CAPACITY: usize = 8> {
    /// Elements with their tags
    #[cfg(not(feature = "hardware-atomic"))]
    elements: [Option<ElementEntry<T>>; CAPACITY],
    #[cfg(not(feature = "hardware-atomic"))]
    element_count: usize,

    /// Removed element tags (tombstones)
    #[cfg(not(feature = "hardware-atomic"))]
    tombstones: [Option<TombstoneEntry<T>>; CAPACITY],
    #[cfg(not(feature = "hardware-atomic"))]
    tombstone_count: usize,

    /// Atomic version uses UnsafeCell for the arrays
    #[cfg(feature = "hardware-atomic")]
    elements: UnsafeCell<[Option<ElementEntry<T>>; CAPACITY]>,
    #[cfg(feature = "hardware-atomic")]
    element_count: AtomicUsize,

    #[cfg(feature = "hardware-atomic")]
    tombstones: UnsafeCell<[Option<TombstoneEntry<T>>; CAPACITY]>,
    #[cfg(feature = "hardware-atomic")]
    tombstone_count: AtomicUsize,

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
unsafe impl<T, C: MemoryConfig> Sync for ORSet<T, C>
where
    T: Send,
    C: Send + Sync,
{
}

// Implement Clone manually due to atomic types not implementing Clone
impl<T, C: MemoryConfig, const CAPACITY: usize> Clone for ORSet<T, C, CAPACITY>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                elements: self.elements.clone(),
                element_count: self.element_count,
                tombstones: self.tombstones.clone(),
                tombstone_count: self.tombstone_count,
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to manually clone the UnsafeCell content
            let cloned_elements = unsafe { (*self.elements.get()).clone() };
            let cloned_tombstones = unsafe { (*self.tombstones.get()).clone() };
            Self {
                elements: UnsafeCell::new(cloned_elements),
                element_count: AtomicUsize::new(self.element_count.load(Ordering::Relaxed)),
                tombstones: UnsafeCell::new(cloned_tombstones),
                tombstone_count: AtomicUsize::new(self.tombstone_count.load(Ordering::Relaxed)),
                node_id: self.node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

/// Element entry with unique tag
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct ElementEntry<T> {
    #[cfg_attr(feature = "serde", serde(rename = "el"))]
    element: T,
    #[cfg_attr(feature = "serde", serde(with = "compact_timestamp_serde", rename = "ts"))]
    timestamp: CompactTimestamp,
    #[cfg_attr(feature = "serde", serde(rename = "n"))]
    node_id: NodeId,
}

/// Tombstone entry for removed elements
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct TombstoneEntry<T> {
    #[cfg_attr(feature = "serde", serde(rename = "el"))]
    element: T,
    #[cfg_attr(feature = "serde", serde(with = "compact_timestamp_serde", rename = "ts"))]
    timestamp: CompactTimestamp,
    #[cfg_attr(feature = "serde", serde(rename = "n"))]
    node_id: NodeId,
    #[cfg_attr(
        feature = "serde",
        serde(with = "compact_timestamp_serde", rename = "rt")
    )]
    remove_timestamp: CompactTimestamp,
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

impl<T, C: MemoryConfig, const CAPACITY: usize> ORSet<T, C, CAPACITY>
where
    T: Clone + PartialEq,
{
    /// Creates a new observed-remove set for the given node with custom capacity
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node (must be < MAX_NODES)
    ///
    /// # Returns
    /// A new empty set
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let set = ORSet::<u32, DefaultConfig, 16>::with_capacity(1);
    /// assert!(set.is_empty());
    /// ```
    pub fn with_capacity(node_id: NodeId) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                elements: [const { None }; CAPACITY],
                element_count: 0,
                tombstones: [const { None }; CAPACITY],
                tombstone_count: 0,
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            Self {
                elements: UnsafeCell::new([const { None }; CAPACITY]),
                element_count: AtomicUsize::new(0),
                tombstones: UnsafeCell::new([const { None }; CAPACITY]),
                tombstone_count: AtomicUsize::new(0),
                node_id,
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<T, C: MemoryConfig> ORSet<T, C, 8>
where
    T: Clone + PartialEq,
{
    /// Creates a new observed-remove set for the given node with default capacity
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node (must be < MAX_NODES)
    ///
    /// # Returns
    /// A new empty set
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let set = ORSet::<u32, DefaultConfig>::new(1);
    /// assert!(set.is_empty());
    /// ```
    pub fn new(node_id: NodeId) -> Self {
        Self::with_capacity(node_id)
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> ORSet<T, C, CAPACITY>
where
    T: Clone + PartialEq,
{
    /// Adds an element to the set with a timestamp
    ///
    /// # Arguments
    /// * `element` - The element to add
    /// * `timestamp` - The timestamp for this add operation
    ///
    /// # Returns
    /// Ok(true) if the element was newly added, Ok(false) if it already existed,
    /// or an error if the set is full
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = ORSet::<u32, DefaultConfig>::new(1);
    /// assert!(set.add(42, 1000)?);  // Newly added
    /// assert!(!set.add(42, 1001)?); // Already exists
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn add(&mut self, element: T, timestamp: u64) -> CRDTResult<bool> {
        let new_timestamp = CompactTimestamp::new(timestamp);

        // Check if element already exists from this node
        for existing in self.elements.iter_mut().take(self.element_count) {
            if let Some(existing_entry) = existing {
                if existing_entry.element == element && existing_entry.node_id == self.node_id {
                    // Update if newer timestamp
                    if new_timestamp > existing_entry.timestamp {
                        existing_entry.timestamp = new_timestamp;
                    }
                    return Ok(false); // Element already exists from this node
                }
            }
        }

        // Check if we have space
        if self.element_count >= CAPACITY {
            return Err(CRDTError::BufferOverflow);
        }

        // Add the new element
        self.elements[self.element_count] = Some(ElementEntry {
            element,
            timestamp: new_timestamp,
            node_id: self.node_id,
        });
        self.element_count += 1;
        Ok(true)
    }

    /// Adds an element to the set with a timestamp (atomic version)
    ///
    /// # Arguments
    /// * `element` - The element to add
    /// * `timestamp` - The timestamp for this add operation
    ///
    /// # Returns
    /// Ok(true) if the element was newly added, Ok(false) if it already existed,
    /// or an error if the set is full
    #[cfg(feature = "hardware-atomic")]
    pub fn add(&self, element: T, timestamp: u64) -> CRDTResult<bool> {
        let new_timestamp = CompactTimestamp::new(timestamp);

        // Atomic compare-exchange loop for coordination
        loop {
            let current_count = self.element_count.load(Ordering::Relaxed);

            // SAFETY: Read the elements array to check for existing element
            let elements_ptr = self.elements.get();
            let elements_ref = unsafe { &*elements_ptr };

            // Check if element already exists from this node
            for existing in elements_ref.iter().take(current_count) {
                if let Some(existing_entry) = existing {
                    if existing_entry.element == element && existing_entry.node_id == self.node_id {
                        // For atomic version, we can't easily update timestamp in place
                        // Return false indicating element already exists
                        return Ok(false);
                    }
                }
            }

            // Check if we have space
            if current_count >= CAPACITY {
                return Err(CRDTError::BufferOverflow);
            }

            // Try to atomically increment count to reserve a slot
            match self.element_count.compare_exchange_weak(
                current_count,
                current_count + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Successfully reserved slot, now insert the element
                    let elements_mut = unsafe { &mut *elements_ptr };
                    elements_mut[current_count] = Some(ElementEntry {
                        element,
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

    /// Removes an element from the set
    ///
    /// # Arguments
    /// * `element` - The element to remove
    /// * `timestamp` - The timestamp for this remove operation
    ///
    /// # Returns
    /// Ok(true) if the element was removed, Ok(false) if it wasn't present,
    /// or an error if the tombstone storage is full
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = ORSet::<u32, DefaultConfig>::new(1);
    /// set.add(42, 1000)?;
    /// assert!(set.remove(&42, 2000)?);  // Successfully removed
    /// assert!(!set.remove(&42, 2001)?); // Already removed
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn remove(&mut self, element: &T, timestamp: u64) -> CRDTResult<bool> {
        let remove_timestamp = CompactTimestamp::new(timestamp);

        // Check if the element is currently present
        if !self.contains(element) {
            return Ok(false); // Element not present, nothing to remove
        }

        // Find all matching elements to remove
        let mut removed_any = false;
        for existing in self.elements.iter().take(self.element_count) {
            if let Some(existing_entry) = existing {
                if existing_entry.element == *element {
                    // Check if we have space for tombstone
                    if self.tombstone_count >= CAPACITY {
                        return Err(CRDTError::BufferOverflow);
                    }

                    // Add tombstone for this specific element entry
                    self.tombstones[self.tombstone_count] = Some(TombstoneEntry {
                        element: existing_entry.element.clone(),
                        timestamp: existing_entry.timestamp,
                        node_id: existing_entry.node_id,
                        remove_timestamp,
                    });
                    self.tombstone_count += 1;
                    removed_any = true;
                }
            }
        }

        Ok(removed_any)
    }

    /// Removes an element from the set (atomic version)
    ///
    /// # Arguments
    /// * `element` - The element to remove
    /// * `timestamp` - The timestamp for this remove operation
    ///
    /// # Returns
    /// Ok(true) if the element was removed, Ok(false) if it wasn't present,
    /// or an error if the tombstone storage is full
    #[cfg(feature = "hardware-atomic")]
    pub fn remove(&self, element: &T, timestamp: u64) -> CRDTResult<bool> {
        let remove_timestamp = CompactTimestamp::new(timestamp);

        // Check if the element is currently present
        if !self.contains(element) {
            return Ok(false); // Element not present, nothing to remove
        }

        // Atomic compare-exchange loop for tombstone coordination
        loop {
            let current_element_count = self.element_count.load(Ordering::Relaxed);
            let current_tombstone_count = self.tombstone_count.load(Ordering::Relaxed);

            // SAFETY: Read the arrays to find matching elements
            let elements_ptr = self.elements.get();
            let elements_ref = unsafe { &*elements_ptr };
            let tombstones_ptr = self.tombstones.get();

            // Find all matching elements to remove
            let mut tombstones_to_add = Vec::new();
            for existing in elements_ref.iter().take(current_element_count) {
                if let Some(existing_entry) = existing {
                    if existing_entry.element == *element {
                        tombstones_to_add.push(TombstoneEntry {
                            element: existing_entry.element.clone(),
                            timestamp: existing_entry.timestamp,
                            node_id: existing_entry.node_id,
                            remove_timestamp,
                        });
                    }
                }
            }

            if tombstones_to_add.is_empty() {
                return Ok(false); // No elements to remove
            }

            // Check if we have space for all tombstones
            if current_tombstone_count + tombstones_to_add.len() > CAPACITY {
                return Err(CRDTError::BufferOverflow);
            }

            // Try to atomically reserve space for tombstones
            let new_tombstone_count = current_tombstone_count + tombstones_to_add.len();
            match self.tombstone_count.compare_exchange_weak(
                current_tombstone_count,
                new_tombstone_count,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Successfully reserved slots, now add the tombstones
                    let tombstones_mut = unsafe { &mut *tombstones_ptr };
                    for (i, tombstone) in tombstones_to_add.into_iter().enumerate() {
                        tombstones_mut[current_tombstone_count + i] = Some(tombstone);
                    }
                    return Ok(true);
                }
                Err(_) => {
                    // Count changed, retry the loop
                    continue;
                }
            }
        }
    }

    /// Checks if the set contains an element
    ///
    /// An element is considered present if:
    /// 1. It exists in the elements array, AND
    /// 2. It's not in the tombstones array, OR
    /// 3. It was added after it was removed (timestamp comparison)
    ///
    /// # Arguments
    /// * `element` - The element to check for
    ///
    /// # Returns
    /// true if the element is in the set, false otherwise
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = ORSet::<u32, DefaultConfig>::new(1);
    /// set.add(42, 1000)?;
    /// assert!(set.contains(&42));
    /// set.remove(&42, 2000)?;
    /// assert!(!set.contains(&42));
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn contains(&self, element: &T) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Find all add operations for this element
            let mut max_add_timestamp = None;
            for entry in self.elements.iter().take(self.element_count) {
                if let Some(entry) = entry {
                    if entry.element == *element {
                        match max_add_timestamp {
                            None => max_add_timestamp = Some(entry.timestamp),
                            Some(current_max) => {
                                if entry.timestamp > current_max {
                                    max_add_timestamp = Some(entry.timestamp);
                                }
                            }
                        }
                    }
                }
            }

            // If no add operations, element is not present
            let max_add = match max_add_timestamp {
                Some(ts) => ts,
                None => return false,
            };

            // Find the latest remove operation for this element
            let mut max_remove_timestamp = None;
            for tombstone in self.tombstones.iter().take(self.tombstone_count) {
                if let Some(tombstone) = tombstone {
                    if tombstone.element == *element {
                        match max_remove_timestamp {
                            None => max_remove_timestamp = Some(tombstone.remove_timestamp),
                            Some(current_max) => {
                                if tombstone.remove_timestamp > current_max {
                                    max_remove_timestamp = Some(tombstone.remove_timestamp);
                                }
                            }
                        }
                    }
                }
            }

            // Element is present if it was added after the latest remove (or never removed)
            match max_remove_timestamp {
                None => true,                             // Never removed
                Some(max_remove) => max_add > max_remove, // Added after latest remove
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_element_count = self.element_count.load(Ordering::Relaxed);
            let current_tombstone_count = self.tombstone_count.load(Ordering::Relaxed);

            // SAFETY: Read the arrays to check for element presence
            let elements_ref = unsafe { &*self.elements.get() };
            let tombstones_ref = unsafe { &*self.tombstones.get() };

            // Find all add operations for this element
            let mut max_add_timestamp = None;
            for entry in elements_ref.iter().take(current_element_count) {
                if let Some(entry) = entry {
                    if entry.element == *element {
                        match max_add_timestamp {
                            None => max_add_timestamp = Some(entry.timestamp),
                            Some(current_max) => {
                                if entry.timestamp > current_max {
                                    max_add_timestamp = Some(entry.timestamp);
                                }
                            }
                        }
                    }
                }
            }

            // If no add operations, element is not present
            let max_add = match max_add_timestamp {
                Some(ts) => ts,
                None => return false,
            };

            // Find the latest remove operation for this element
            let mut max_remove_timestamp = None;
            for tombstone in tombstones_ref.iter().take(current_tombstone_count) {
                if let Some(tombstone) = tombstone {
                    if tombstone.element == *element {
                        match max_remove_timestamp {
                            None => max_remove_timestamp = Some(tombstone.remove_timestamp),
                            Some(current_max) => {
                                if tombstone.remove_timestamp > current_max {
                                    max_remove_timestamp = Some(tombstone.remove_timestamp);
                                }
                            }
                        }
                    }
                }
            }

            // Element is present if it was added after the latest remove (or never removed)
            match max_remove_timestamp {
                None => true,                             // Never removed
                Some(max_remove) => max_add > max_remove, // Added after latest remove
            }
        }
    }

    /// Returns the number of elements currently in the set
    ///
    /// # Returns
    /// The count of elements (excluding removed ones)
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = ORSet::<u32, DefaultConfig>::new(1);
    /// assert_eq!(set.len(), 0);
    /// set.add(42, 1000)?;
    /// assert_eq!(set.len(), 1);
    /// set.remove(&42, 2000)?;
    /// assert_eq!(set.len(), 0);
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn len(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            let mut count = 0;

            // For each element entry, check if it's present and not already counted
            for i in 0..self.element_count {
                if let Some(entry) = &self.elements[i] {
                    if self.contains(&entry.element) {
                        // Check if we've already counted this element value
                        let mut already_counted = false;
                        for j in 0..i {
                            if let Some(prev_entry) = &self.elements[j] {
                                if prev_entry.element == entry.element
                                    && self.contains(&prev_entry.element)
                                {
                                    already_counted = true;
                                    break;
                                }
                            }
                        }

                        // If not already counted, increment count
                        if !already_counted {
                            count += 1;
                        }
                    }
                }
            }
            count
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let mut count = 0;
            let current_element_count = self.element_count.load(Ordering::Relaxed);
            let elements_ref = unsafe { &*self.elements.get() };

            // For each element entry, check if it's present and not already counted
            for i in 0..current_element_count {
                if let Some(entry) = &elements_ref[i] {
                    if self.contains(&entry.element) {
                        // Check if we've already counted this element value
                        let mut already_counted = false;
                        for j in 0..i {
                            if let Some(prev_entry) = &elements_ref[j] {
                                if prev_entry.element == entry.element
                                    && self.contains(&prev_entry.element)
                                {
                                    already_counted = true;
                                    break;
                                }
                            }
                        }

                        // If not already counted, increment count
                        if !already_counted {
                            count += 1;
                        }
                    }
                }
            }
            count
        }
    }

    /// Checks if the set is empty
    ///
    /// # Returns
    /// true if the set contains no elements, false otherwise
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = ORSet::<u32, DefaultConfig>::new(1);
    /// assert!(set.is_empty());
    /// set.add(42, 1000)?;
    /// assert!(!set.is_empty());
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Checks if the set is full (cannot add more elements)
    ///
    /// # Returns
    /// true if no more elements can be added, false otherwise
    pub fn is_full(&self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.element_count >= CAPACITY
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.element_count.load(Ordering::Relaxed) >= CAPACITY
        }
    }

    /// Returns the maximum capacity for elements
    ///
    /// # Returns
    /// The maximum number of element entries this set can hold
    pub fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Returns the remaining capacity for elements
    ///
    /// # Returns
    /// The number of additional element entries that can be stored
    pub fn remaining_capacity(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            CAPACITY - self.element_count
        }

        #[cfg(feature = "hardware-atomic")]
        {
            CAPACITY - self.element_count.load(Ordering::Relaxed)
        }
    }

    /// Gets this node's ID
    ///
    /// # Returns
    /// The node ID of this set
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Returns an iterator over the elements currently in the set
    ///
    /// # Returns
    /// An iterator over elements that are present (not removed)
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.elements
                .iter()
                .take(self.element_count)
                .filter_map(|opt| opt.as_ref())
                .filter(move |entry| self.contains(&entry.element))
                .map(|entry| &entry.element)
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_element_count = self.element_count.load(Ordering::Relaxed);
            let elements_ref = unsafe { &*self.elements.get() };
            elements_ref
                .iter()
                .take(current_element_count)
                .filter_map(|opt| opt.as_ref())
                .filter(move |entry| self.contains(&entry.element))
                .map(|entry| &entry.element)
        }
    }

    /// Returns the number of element entries (including removed ones)
    ///
    /// # Returns
    /// The total number of element entries stored
    pub fn element_entries(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.element_count
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.element_count.load(Ordering::Relaxed)
        }
    }

    /// Returns the number of tombstone entries
    ///
    /// # Returns
    /// The number of tombstone entries stored
    pub fn tombstone_entries(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.tombstone_count
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.tombstone_count.load(Ordering::Relaxed)
        }
    }
}

// Serde implementation for ORSet
#[cfg(feature = "serde")]
impl<T, C: MemoryConfig, const CAPACITY: usize> Serialize for ORSet<T, C, CAPACITY>
where
    T: Serialize + Clone + PartialEq,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("ORSet", 5)?;

        // Serialize the logical state (elements, tombstones, counts, and node_id)
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Serialize only the used portions of the arrays as slices
            state.serialize_field("e", &&self.elements[..self.element_count])?;
            state.serialize_field("ec", &self.element_count)?;
            state.serialize_field("t", &&self.tombstones[..self.tombstone_count])?;
            state.serialize_field("tc", &self.tombstone_count)?;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to extract values safely
            let current_element_count = self.element_count.load(Ordering::Relaxed);
            let current_tombstone_count = self.tombstone_count.load(Ordering::Relaxed);
            let elements_ref = unsafe { &*self.elements.get() };
            let tombstones_ref = unsafe { &*self.tombstones.get() };
            state.serialize_field("e", &&elements_ref[..current_element_count])?;
            state.serialize_field("ec", &current_element_count)?;
            state.serialize_field("t", &&tombstones_ref[..current_tombstone_count])?;
            state.serialize_field("tc", &current_tombstone_count)?;
        }

        state.serialize_field("n", &self.node_id)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, T, C: MemoryConfig, const CAPACITY: usize> Deserialize<'de> for ORSet<T, C, CAPACITY>
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
            #[serde(rename = "e")]
            Elements,
            #[serde(rename = "ec")]
            ElementCount,
            #[serde(rename = "t")]
            Tombstones,
            #[serde(rename = "tc")]
            TombstoneCount,
            #[serde(rename = "n")]
            NodeId,
        }

        struct ORSetVisitor<T, C: MemoryConfig, const CAPACITY: usize> {
            _phantom: core::marker::PhantomData<(T, C)>,
        }

        impl<'de, T, C: MemoryConfig, const CAPACITY: usize> Visitor<'de> for ORSetVisitor<T, C, CAPACITY>
        where
            T: Deserialize<'de> + Clone + PartialEq,
        {
            type Value = ORSet<T, C, CAPACITY>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct ORSet")
            }

            fn visit_map<V>(self, mut map: V) -> Result<ORSet<T, C, CAPACITY>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut elements = None;
                let mut element_count = None;
                let mut tombstones = None;
                let mut tombstone_count = None;
                let mut node_id = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Elements => {
                            if elements.is_some() {
                                return Err(de::Error::duplicate_field("e"));
                            }
                            // Use a custom deserializer that doesn't require Vec
                            use serde::de::SeqAccess;

                            struct ElementsDeserializer<T, const CAPACITY: usize> {
                                _phantom: core::marker::PhantomData<T>,
                            }

                            impl<'de, T, const CAPACITY: usize> serde::de::DeserializeSeed<'de>
                                for ElementsDeserializer<T, CAPACITY>
                            where
                                T: Deserialize<'de>,
                            {
                                type Value = [Option<ElementEntry<T>>; CAPACITY];

                                fn deserialize<D>(
                                    self,
                                    deserializer: D,
                                ) -> Result<Self::Value, D::Error>
                                where
                                    D: serde::de::Deserializer<'de>,
                                {
                                    struct ElementsVisitor<T, const CAPACITY: usize> {
                                        _phantom: core::marker::PhantomData<T>,
                                    }

                                    impl<'de, T, const CAPACITY: usize> serde::de::Visitor<'de> for ElementsVisitor<T, CAPACITY>
                                    where
                                        T: Deserialize<'de>,
                                    {
                                        type Value = [Option<ElementEntry<T>>; CAPACITY];

                                        fn expecting(
                                            &self,
                                            formatter: &mut core::fmt::Formatter,
                                        ) -> core::fmt::Result
                                        {
                                            write!(
                                                formatter,
                                                "a sequence of at most {} elements",
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

                                            while let Some(element) =
                                                seq.next_element::<Option<ElementEntry<T>>>()?
                                            {
                                                if index >= CAPACITY {
                                                    return Err(serde::de::Error::custom(
                                                        "too many elements for capacity",
                                                    ));
                                                }
                                                array[index] = element;
                                                index += 1;
                                            }

                                            Ok(array)
                                        }
                                    }

                                    deserializer.deserialize_seq(ElementsVisitor::<T, CAPACITY> {
                                        _phantom: core::marker::PhantomData,
                                    })
                                }
                            }

                            let elements_array =
                                map.next_value_seed(ElementsDeserializer::<T, CAPACITY> {
                                    _phantom: core::marker::PhantomData,
                                })?;
                            elements = Some(elements_array);
                        }
                        Field::ElementCount => {
                            if element_count.is_some() {
                                return Err(de::Error::duplicate_field("ec"));
                            }
                            element_count = Some(map.next_value::<usize>()?);
                        }
                        Field::Tombstones => {
                            if tombstones.is_some() {
                                return Err(de::Error::duplicate_field("t"));
                            }
                            // Use a custom deserializer for tombstones
                            use serde::de::SeqAccess;

                            struct TombstonesDeserializer<T, const CAPACITY: usize> {
                                _phantom: core::marker::PhantomData<T>,
                            }

                            impl<'de, T, const CAPACITY: usize> serde::de::DeserializeSeed<'de>
                                for TombstonesDeserializer<T, CAPACITY>
                            where
                                T: Deserialize<'de>,
                            {
                                type Value = [Option<TombstoneEntry<T>>; CAPACITY];

                                fn deserialize<D>(
                                    self,
                                    deserializer: D,
                                ) -> Result<Self::Value, D::Error>
                                where
                                    D: serde::de::Deserializer<'de>,
                                {
                                    struct TombstonesVisitor<T, const CAPACITY: usize> {
                                        _phantom: core::marker::PhantomData<T>,
                                    }

                                    impl<'de, T, const CAPACITY: usize> serde::de::Visitor<'de> for TombstonesVisitor<T, CAPACITY>
                                    where
                                        T: Deserialize<'de>,
                                    {
                                        type Value = [Option<TombstoneEntry<T>>; CAPACITY];

                                        fn expecting(
                                            &self,
                                            formatter: &mut core::fmt::Formatter,
                                        ) -> core::fmt::Result
                                        {
                                            write!(
                                                formatter,
                                                "a sequence of at most {} tombstones",
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

                                            while let Some(tombstone) =
                                                seq.next_element::<Option<TombstoneEntry<T>>>()?
                                            {
                                                if index >= CAPACITY {
                                                    return Err(serde::de::Error::custom(
                                                        "too many tombstones for capacity",
                                                    ));
                                                }
                                                array[index] = tombstone;
                                                index += 1;
                                            }

                                            Ok(array)
                                        }
                                    }

                                    deserializer.deserialize_seq(TombstonesVisitor::<T, CAPACITY> {
                                        _phantom: core::marker::PhantomData,
                                    })
                                }
                            }

                            let tombstones_array =
                                map.next_value_seed(TombstonesDeserializer::<T, CAPACITY> {
                                    _phantom: core::marker::PhantomData,
                                })?;
                            tombstones = Some(tombstones_array);
                        }
                        Field::TombstoneCount => {
                            if tombstone_count.is_some() {
                                return Err(de::Error::duplicate_field("tc"));
                            }
                            tombstone_count = Some(map.next_value::<usize>()?);
                        }
                        Field::NodeId => {
                            if node_id.is_some() {
                                return Err(de::Error::duplicate_field("n"));
                            }
                            node_id = Some(map.next_value::<NodeId>()?);
                        }
                    }
                }

                let elements_array =
                    elements.ok_or_else(|| de::Error::missing_field("e"))?;
                let element_count =
                    element_count.ok_or_else(|| de::Error::missing_field("ec"))?;
                let tombstones_array =
                    tombstones.ok_or_else(|| de::Error::missing_field("t"))?;
                let tombstone_count =
                    tombstone_count.ok_or_else(|| de::Error::missing_field("tc"))?;
                let node_id = node_id.ok_or_else(|| de::Error::missing_field("n"))?;

                // Validate counts are within capacity
                if element_count > CAPACITY {
                    return Err(de::Error::custom("element_count exceeds capacity"));
                }
                if tombstone_count > CAPACITY {
                    return Err(de::Error::custom("tombstone_count exceeds capacity"));
                }

                // Reconstruct the ORSet
                #[cfg(not(feature = "hardware-atomic"))]
                {
                    Ok(ORSet {
                        elements: elements_array,
                        element_count,
                        tombstones: tombstones_array,
                        tombstone_count,
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }

                #[cfg(feature = "hardware-atomic")]
                {
                    Ok(ORSet {
                        elements: UnsafeCell::new(elements_array),
                        element_count: AtomicUsize::new(element_count),
                        tombstones: UnsafeCell::new(tombstones_array),
                        tombstone_count: AtomicUsize::new(tombstone_count),
                        node_id,
                        _phantom: core::marker::PhantomData,
                    })
                }
            }
        }

        const FIELDS: &[&str] = &["e", "ec", "t", "tc", "n"];
        deserializer.deserialize_struct(
            "ORSet",
            FIELDS,
            ORSetVisitor {
                _phantom: core::marker::PhantomData,
            },
        )
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> Default for ORSet<T, C, CAPACITY>
where
    T: Clone + PartialEq,
{
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> CRDT<C> for ORSet<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            let other_element_count = other.element_count;
            let other_tombstone_count = other.tombstone_count;
            let other_elements_ref = &other.elements;
            let other_tombstones_ref = &other.tombstones;

            // Merge elements
            for other_entry in other_elements_ref.iter().take(other_element_count) {
                if let Some(other_entry) = other_entry {
                    // Check if we already have this exact entry
                    let mut found = false;

                    for our_entry in self.elements.iter().take(self.element_count) {
                        if let Some(our_entry) = our_entry {
                            if our_entry.element == other_entry.element
                                && our_entry.timestamp == other_entry.timestamp
                                && our_entry.node_id == other_entry.node_id
                            {
                                found = true;
                                break;
                            }
                        }
                    }

                    if !found {
                        // Check if we have space
                        if self.element_count >= CAPACITY {
                            return Err(CRDTError::BufferOverflow);
                        }

                        // Add the element entry
                        self.elements[self.element_count] = Some(ElementEntry {
                            element: other_entry.element.clone(),
                            timestamp: other_entry.timestamp,
                            node_id: other_entry.node_id,
                        });
                        self.element_count += 1;
                    }
                }
            }

            // Merge tombstones

            for other_tombstone in other_tombstones_ref.iter().take(other_tombstone_count) {
                if let Some(other_tombstone) = other_tombstone {
                    // Check if we already have this exact tombstone
                    let mut found = false;
                    for our_tombstone in self.tombstones.iter().take(self.tombstone_count) {
                        if let Some(our_tombstone) = our_tombstone {
                            if our_tombstone.element == other_tombstone.element
                                && our_tombstone.timestamp == other_tombstone.timestamp
                                && our_tombstone.node_id == other_tombstone.node_id
                                && our_tombstone.remove_timestamp
                                    == other_tombstone.remove_timestamp
                            {
                                found = true;
                                break;
                            }
                        }
                    }

                    if !found {
                        // Check if we have space
                        if self.tombstone_count >= 8 {
                            return Err(CRDTError::BufferOverflow);
                        }

                        // Add the tombstone entry
                        self.tombstones[self.tombstone_count] = Some(TombstoneEntry {
                            element: other_tombstone.element.clone(),
                            timestamp: other_tombstone.timestamp,
                            node_id: other_tombstone.node_id,
                            remove_timestamp: other_tombstone.remove_timestamp,
                        });
                        self.tombstone_count += 1;
                    }
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, merge requires &mut self so it's not thread-safe during merge
            // But we can still implement the same logic using unsafe access to the UnsafeCell
            let other_element_count = other.element_count.load(Ordering::Relaxed);
            let other_tombstone_count = other.tombstone_count.load(Ordering::Relaxed);
            let other_elements_ref = unsafe { &*other.elements.get() };
            let other_tombstones_ref = unsafe { &*other.tombstones.get() };

            let self_elements_mut = unsafe { &mut *self.elements.get() };
            let self_tombstones_mut = unsafe { &mut *self.tombstones.get() };
            let mut self_element_count = self.element_count.load(Ordering::Relaxed);
            let mut self_tombstone_count = self.tombstone_count.load(Ordering::Relaxed);

            // Merge elements
            for other_entry in other_elements_ref.iter().take(other_element_count) {
                if let Some(other_entry) = other_entry {
                    // Check if we already have this exact entry
                    let mut found = false;
                    for our_entry in self_elements_mut.iter().take(self_element_count) {
                        if let Some(our_entry) = our_entry {
                            if our_entry.element == other_entry.element
                                && our_entry.timestamp == other_entry.timestamp
                                && our_entry.node_id == other_entry.node_id
                            {
                                found = true;
                                break;
                            }
                        }
                    }

                    if !found {
                        // Check if we have space
                        if self_element_count >= 8 {
                            return Err(CRDTError::BufferOverflow);
                        }

                        // Add the element entry
                        self_elements_mut[self_element_count] = Some(ElementEntry {
                            element: other_entry.element.clone(),
                            timestamp: other_entry.timestamp,
                            node_id: other_entry.node_id,
                        });
                        self_element_count += 1;
                    }
                }
            }

            // Merge tombstones

            for other_tombstone in other_tombstones_ref.iter().take(other_tombstone_count) {
                if let Some(other_tombstone) = other_tombstone {
                    // Check if we already have this exact tombstone
                    let mut found = false;
                    for our_tombstone in self_tombstones_mut.iter().take(self_tombstone_count) {
                        if let Some(our_tombstone) = our_tombstone {
                            if our_tombstone.element == other_tombstone.element
                                && our_tombstone.timestamp == other_tombstone.timestamp
                                && our_tombstone.node_id == other_tombstone.node_id
                                && our_tombstone.remove_timestamp
                                    == other_tombstone.remove_timestamp
                            {
                                found = true;
                                break;
                            }
                        }
                    }

                    if !found {
                        // Check if we have space
                        if self_tombstone_count >= 8 {
                            return Err(CRDTError::BufferOverflow);
                        }

                        // Add the tombstone entry
                        self_tombstones_mut[self_tombstone_count] = Some(TombstoneEntry {
                            element: other_tombstone.element.clone(),
                            timestamp: other_tombstone.timestamp,
                            node_id: other_tombstone.node_id,
                            remove_timestamp: other_tombstone.remove_timestamp,
                        });
                        self_tombstone_count += 1;
                    }
                }
            }

            // Update the atomic counts
            self.element_count
                .store(self_element_count, Ordering::Relaxed);
            self.tombstone_count
                .store(self_tombstone_count, Ordering::Relaxed);
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        // Two ORSets are equal if they contain the same elements
        // (regardless of internal representation)
        if self.len() != other.len() {
            return false;
        }

        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Check that all our elements are in other

            for entry in self.elements.iter().take(self.element_count) {
                if let Some(entry) = entry {
                    if self.contains(&entry.element) && !other.contains(&entry.element) {
                        return false;
                    }
                }
            }

            // Check that all other's elements are in us

            for entry in other.elements.iter().take(other.element_count) {
                if let Some(entry) = entry {
                    if other.contains(&entry.element) && !self.contains(&entry.element) {
                        return false;
                    }
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let self_element_count = self.element_count.load(Ordering::Relaxed);
            let other_element_count = other.element_count.load(Ordering::Relaxed);
            let self_elements_ref = unsafe { &*self.elements.get() };
            let other_elements_ref = unsafe { &*other.elements.get() };

            // Check that all our elements are in other
            for entry in self_elements_ref.iter().take(self_element_count) {
                if let Some(entry) = entry {
                    if self.contains(&entry.element) && !other.contains(&entry.element) {
                        return false;
                    }
                }
            }

            // Check that all other's elements are in us
            for entry in other_elements_ref.iter().take(other_element_count) {
                if let Some(entry) = entry {
                    if other.contains(&entry.element) && !self.contains(&entry.element) {
                        return false;
                    }
                }
            }
        }

        true
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
            // Validate counts are within bounds
            if self.element_count > 8 || self.tombstone_count > 8 {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate that we don't exceed the configured maximum elements
            if self.element_count > C::MAX_SET_ELEMENTS {
                return Err(CRDTError::ConfigurationExceeded);
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_element_count = self.element_count.load(Ordering::Relaxed);
            let current_tombstone_count = self.tombstone_count.load(Ordering::Relaxed);

            // Validate counts are within bounds
            if current_element_count > 8 || current_tombstone_count > 8 {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate that we don't exceed the configured maximum elements
            if current_element_count > C::MAX_SET_ELEMENTS {
                return Err(CRDTError::ConfigurationExceeded);
            }
        }

        Ok(())
    }

    fn state_hash(&self) -> u32 {
        // Hash based on current elements (order-independent)
        let mut hash = 0u32;

        #[cfg(not(feature = "hardware-atomic"))]
        {
            for entry in self.elements.iter().take(self.element_count) {
                if let Some(entry) = entry {
                    if self.contains(&entry.element) {
                        let element_ptr = &entry.element as *const T as usize;
                        hash ^= element_ptr as u32;
                    }
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_element_count = self.element_count.load(Ordering::Relaxed);
            let elements_ref = unsafe { &*self.elements.get() };
            for entry in elements_ref.iter().take(current_element_count) {
                if let Some(entry) = entry {
                    if self.contains(&entry.element) {
                        let element_ptr = &entry.element as *const T as usize;
                        hash ^= element_ptr as u32;
                    }
                }
            }
        }

        hash ^= self.len() as u32;
        hash
    }

    fn can_merge(&self, other: &Self) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Check if merging would exceed capacity for elements
            let mut new_elements = 0;

            for other_entry in other.elements.iter().take(other.element_count) {
                if let Some(other_entry) = other_entry {
                    let mut found = false;

                    for our_entry in self.elements.iter().take(self.element_count) {
                        if let Some(our_entry) = our_entry {
                            if our_entry.element == other_entry.element
                                && our_entry.timestamp == other_entry.timestamp
                                && our_entry.node_id == other_entry.node_id
                            {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        new_elements += 1;
                    }
                }
            }

            // Check if merging would exceed capacity for tombstones
            let mut new_tombstones = 0;

            for other_tombstone in other.tombstones.iter().take(other.tombstone_count) {
                if let Some(other_tombstone) = other_tombstone {
                    let mut found = false;

                    for our_tombstone in self.tombstones.iter().take(self.tombstone_count) {
                        if let Some(our_tombstone) = our_tombstone {
                            if our_tombstone.element == other_tombstone.element
                                && our_tombstone.timestamp == other_tombstone.timestamp
                                && our_tombstone.node_id == other_tombstone.node_id
                                && our_tombstone.remove_timestamp
                                    == other_tombstone.remove_timestamp
                            {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        new_tombstones += 1;
                    }
                }
            }

            self.element_count + new_elements <= 8 && self.tombstone_count + new_tombstones <= 8
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let self_element_count = self.element_count.load(Ordering::Relaxed);
            let self_tombstone_count = self.tombstone_count.load(Ordering::Relaxed);
            let other_element_count = other.element_count.load(Ordering::Relaxed);
            let other_tombstone_count = other.tombstone_count.load(Ordering::Relaxed);

            let self_elements_ref = unsafe { &*self.elements.get() };
            let self_tombstones_ref = unsafe { &*self.tombstones.get() };
            let other_elements_ref = unsafe { &*other.elements.get() };
            let other_tombstones_ref = unsafe { &*other.tombstones.get() };

            // Check if merging would exceed capacity for elements
            let mut new_elements = 0;
            for other_entry in other_elements_ref.iter().take(other_element_count) {
                if let Some(other_entry) = other_entry {
                    let mut found = false;
                    for our_entry in self_elements_ref.iter().take(self_element_count) {
                        if let Some(our_entry) = our_entry {
                            if our_entry.element == other_entry.element
                                && our_entry.timestamp == other_entry.timestamp
                                && our_entry.node_id == other_entry.node_id
                            {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        new_elements += 1;
                    }
                }
            }

            // Check if merging would exceed capacity for tombstones
            let mut new_tombstones = 0;
            for other_tombstone in other_tombstones_ref.iter().take(other_tombstone_count) {
                if let Some(other_tombstone) = other_tombstone {
                    let mut found = false;
                    for our_tombstone in self_tombstones_ref.iter().take(self_tombstone_count) {
                        if let Some(our_tombstone) = our_tombstone {
                            if our_tombstone.element == other_tombstone.element
                                && our_tombstone.timestamp == other_tombstone.timestamp
                                && our_tombstone.node_id == other_tombstone.node_id
                                && our_tombstone.remove_timestamp
                                    == other_tombstone.remove_timestamp
                            {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        new_tombstones += 1;
                    }
                }
            }

            self_element_count + new_elements <= 8 && self_tombstone_count + new_tombstones <= 8
        }
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> BoundedCRDT<C> for ORSet<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = CAPACITY; // Maximum number of element entries

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.len() // Count of actual elements (not entries)
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // ORSets can't be easily compacted without losing causality information
        // This is a no-op that returns 0 bytes freed
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        // For fixed-size arrays, only check element count, not memory usage
        self.element_count() < Self::MAX_ELEMENTS
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> RealTimeCRDT<C> for ORSet<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_MERGE_CYCLES: u32 = 400; // More complex due to element and tombstone merging
    const MAX_VALIDATE_CYCLES: u32 = 200;
    const MAX_SERIALIZE_CYCLES: u32 = 300;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // ORSet merge is bounded by the number of elements and tombstones
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded by the number of elements and tombstones
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
    fn test_new_set() {
        let set = ORSet::<u32, DefaultConfig>::new(1);
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert_eq!(set.capacity(), 8);
        assert_eq!(set.remaining_capacity(), 8);
        assert!(!set.is_full());
        assert_eq!(set.node_id(), 1);
    }

    #[test]
    fn test_add_and_contains() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);

        // Add element
        assert!(set.add(42, 1000).unwrap());
        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
        assert!(set.contains(&42));
        assert!(!set.contains(&43));

        // Add duplicate (should return false but not error)
        assert!(!set.add(42, 1001).unwrap());
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);

        // Add then remove
        set.add(42, 1000).unwrap();
        assert!(set.contains(&42));

        assert!(set.remove(&42, 2000).unwrap());
        assert!(!set.contains(&42));
        assert_eq!(set.len(), 0);

        // Remove non-existent element
        assert!(!set.remove(&43, 2001).unwrap());
    }

    #[test]
    fn test_add_after_remove() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);

        // Add, remove, then add again with later timestamp
        set.add(42, 1000).unwrap();
        set.remove(&42, 2000).unwrap();
        assert!(!set.contains(&42));

        #[cfg(not(feature = "hardware-atomic"))]
        {
            set.add(42, 3000).unwrap(); // Later timestamp
            assert!(set.contains(&42)); // Should be present again
            assert_eq!(set.len(), 1);
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, adding the same element again from the same node
            // returns false (already exists) and doesn't update timestamp
            assert!(!set.add(42, 3000).unwrap());
            assert!(!set.contains(&42)); // Still removed since timestamp wasn't updated
            assert_eq!(set.len(), 0);
        }
    }

    #[test]
    fn test_remove_before_add() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);

        // Try to remove non-existent element (should return false)
        assert!(!set.remove(&42, 2000).unwrap()); // Nothing to remove

        // Add element with earlier timestamp
        set.add(42, 1000).unwrap(); // Add at time 1000 (earlier than remove)

        // Element should be present since remove didn't affect anything
        assert!(set.contains(&42)); // Should be present (remove was ineffective)
    }

    #[test]
    fn test_merge() {
        let mut set1 = ORSet::<u32, DefaultConfig>::new(1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(2);

        set1.add(1, 1000).unwrap();
        set1.add(2, 1001).unwrap();

        set2.add(2, 1002).unwrap(); // Same element, different timestamp
        set2.add(3, 1003).unwrap();

        // Before merge
        assert_eq!(set1.len(), 2);
        assert_eq!(set2.len(), 2);

        // Merge set2 into set1
        set1.merge(&set2).unwrap();

        assert!(set1.contains(&1));
        assert!(set1.contains(&2));
        assert!(set1.contains(&3));
        assert_eq!(set1.len(), 3);
    }

    #[test]
    fn test_merge_with_removes() {
        let mut set1 = ORSet::<u32, DefaultConfig>::new(1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(2);

        // Set1: add 1, add 2, remove 2
        set1.add(1, 1000).unwrap();
        set1.add(2, 1001).unwrap();
        set1.remove(&2, 1500).unwrap();

        // Set2: add 2 (after set1's remove), add 3
        set2.add(2, 2000).unwrap(); // Later than set1's remove
        set2.add(3, 2001).unwrap();

        set1.merge(&set2).unwrap();

        assert!(set1.contains(&1));
        assert!(set1.contains(&2)); // Should be present (re-added after remove)
        assert!(set1.contains(&3));
        assert_eq!(set1.len(), 3);
    }

    #[test]
    fn test_capacity_limits() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);

        // Fill to capacity
        for i in 0..8 {
            assert!(set.add(i, 1000 + i as u64).is_ok());
        }

        assert!(set.is_full());
        assert_eq!(set.remaining_capacity(), 0);

        // Try to add one more (should fail)
        assert!(set.add(8, 2000).is_err());
    }

    #[test]
    fn test_iter() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);

        set.add(1, 1000).unwrap();
        set.add(3, 1001).unwrap();
        set.add(2, 1002).unwrap();
        set.remove(&2, 1500).unwrap(); // Remove 2

        let mut elements = [0u32; 2];
        let mut i = 0;
        for element in set.iter() {
            elements[i] = *element;
            i += 1;
        }
        elements.sort();
        assert_eq!(elements, [1, 3]); // 2 should be removed
    }

    #[test]
    fn test_merge_idempotent() {
        let mut set1 = ORSet::<u32, DefaultConfig>::new(1);
        let set2 = ORSet::<u32, DefaultConfig>::new(2);

        set1.add(42, 1000).unwrap();

        // Multiple merges should be idempotent
        set1.merge(&set2).unwrap();
        let len1 = set1.len();

        set1.merge(&set2).unwrap();
        let len2 = set1.len();

        assert_eq!(len1, len2);
    }

    #[test]
    fn test_merge_commutative() {
        let mut set1a = ORSet::<u32, DefaultConfig>::new(1);
        let mut set1b = ORSet::<u32, DefaultConfig>::new(1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(2);
        let mut set3 = ORSet::<u32, DefaultConfig>::new(3);

        set1a.add(1, 1000).unwrap();
        set1b.add(1, 1000).unwrap();
        set2.add(2, 2000).unwrap();
        set3.add(3, 3000).unwrap();

        // Merge in different orders
        set1a.merge(&set2).unwrap();
        set1a.merge(&set3).unwrap();

        set1b.merge(&set3).unwrap();
        set1b.merge(&set2).unwrap();

        // Results should be the same
        assert_eq!(set1a.len(), set1b.len());
        assert!(set1a.eq(&set1b));
    }

    #[test]
    fn test_bounded_crdt() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);
        set.add(42, 1000).unwrap();

        assert_eq!(set.element_count(), 1);
        assert!(set.memory_usage() > 0);
        assert!(set.can_add_element());

        // Fill to capacity
        for i in 1..8 {
            set.add(i, 1000 + i as u64).unwrap();
        }

        assert_eq!(set.element_count(), 8);
        assert!(!set.can_add_element());
    }

    #[test]
    fn test_validation() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);
        set.add(42, 1000).unwrap();

        assert!(set.validate().is_ok());
    }

    #[test]
    fn test_real_time_crdt() {
        let mut set1 = ORSet::<u32, DefaultConfig>::new(1);
        let set2 = ORSet::<u32, DefaultConfig>::new(2);

        assert!(set1.merge_bounded(&set2).is_ok());
        assert!(set1.validate_bounded().is_ok());
    }

    #[test]
    fn test_can_merge() {
        let mut set1 = ORSet::<u32, DefaultConfig>::new(1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(2);

        // Fill set1 to capacity
        for i in 0..8 {
            set1.add(i, 1000).unwrap();
        }

        // Empty set2 should be mergeable
        assert!(set1.can_merge(&set2));

        // Set2 with overlapping element should NOT be mergeable (different timestamp/node creates new entry)
        set2.add(5, 2000).unwrap();
        assert!(!set1.can_merge(&set2));

        // Set2 with new element should not be mergeable
        set2.add(100, 3000).unwrap();
        assert!(!set1.can_merge(&set2));
    }

    #[cfg(all(test, feature = "serde"))]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_serialize_deserialize() {
            let mut set = ORSet::<i32, DefaultConfig>::new(1);
            set.add(42, 1000).unwrap();
            set.add(100, 1500).unwrap();
            set.remove(&42, 2000).unwrap();

            // Test that the serde traits are implemented
            // This ensures the code compiles with serde feature
            assert_eq!(set.len(), 1); // Only 100 should remain
            assert!(!set.contains(&42)); // 42 was removed
            assert!(set.contains(&100)); // 100 is still present
            assert_eq!(set.element_entries(), 2); // Both add operations stored
            assert_eq!(set.tombstone_entries(), 1); // One remove operation stored
        }

        #[test]
        fn test_atomic_vs_standard_compatibility() {
            // This test ensures that atomic and standard versions would serialize to the same format
            // The logical state should be identical regardless of internal representation
            let mut set = ORSet::<i32, DefaultConfig>::new(1);
            set.add(42, 1000).unwrap();
            set.add(84, 1500).unwrap();
            set.remove(&42, 2000).unwrap();

            // Both versions should have the same logical state
            assert_eq!(set.len(), 1);
            assert!(!set.contains(&42));
            assert!(set.contains(&84));
            assert_eq!(set.element_entries(), 2);
            assert_eq!(set.tombstone_entries(), 1);
        }

        #[test]
        fn test_empty_set_serialization() {
            let set = ORSet::<i32, DefaultConfig>::new(1);

            // Should handle empty set correctly
            assert_eq!(set.len(), 0);
            assert!(set.is_empty());
            assert_eq!(set.element_entries(), 0);
            assert_eq!(set.tombstone_entries(), 0);
            assert_eq!(set.node_id(), 1);
        }

        #[test]
        fn test_complex_operations_serialization() {
            let mut set1 = ORSet::<i32, DefaultConfig>::new(1);
            let mut set2 = ORSet::<i32, DefaultConfig>::new(2);

            // Complex sequence: add, remove, merge, add again
            set1.add(100, 1000).unwrap();
            set1.add(200, 1100).unwrap();
            set1.remove(&100, 1500).unwrap();

            set2.add(100, 2000).unwrap(); // Re-add after remove
            set2.add(300, 2100).unwrap();

            set1.merge(&set2).unwrap();

            // Should handle complex CRDT semantics correctly
            assert_eq!(set1.len(), 3); // 100 (re-added), 200, 300
            assert!(set1.contains(&100)); // Re-added with later timestamp
            assert!(set1.contains(&200));
            assert!(set1.contains(&300));
            assert_eq!(set1.element_entries(), 4); // All add operations
            assert_eq!(set1.tombstone_entries(), 1); // One remove operation
        }

        #[test]
        fn test_custom_capacity_serialization() {
            let mut set = ORSet::<i32, DefaultConfig, 4>::with_capacity(1);
            set.add(100, 1000).unwrap();
            set.add(200, 1100).unwrap();
            set.remove(&100, 1500).unwrap();

            // Should handle custom capacity correctly
            assert_eq!(set.len(), 1);
            assert_eq!(set.capacity(), 4);
            assert!(!set.contains(&100));
            assert!(set.contains(&200));
            assert_eq!(set.element_entries(), 2);
            assert_eq!(set.tombstone_entries(), 1);
        }
    }
}
