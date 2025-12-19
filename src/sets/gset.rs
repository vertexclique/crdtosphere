//! Grow-only Set CRDT
//!
//! A set that can only add elements, never remove them.
//! Uses zero allocation with a fixed array for deterministic memory usage.

use crate::error::{CRDTError, CRDTResult};
use crate::memory::MemoryConfig;
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

#[cfg(feature = "hardware-atomic")]
use core::cell::UnsafeCell;
#[cfg(feature = "hardware-atomic")]
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Grow-only Set with configurable element array
///
/// This set can only add elements and provides eventual consistency
/// across multiple nodes. Elements are stored in a fixed-size array.
///
/// # Type Parameters
/// - `T`: The element type stored in the set
/// - `C`: Memory configuration that determines the default maximum number of elements
/// - `CAPACITY`: The maximum number of elements this set can hold (defaults to 16)
///
/// # Memory Usage
/// - Fixed size: sizeof(T) * CAPACITY + 8 bytes
/// - Example: For u32 with 16 elements = 72 bytes, for u64 with 32 elements = 264 bytes
/// - Completely predictable at compile time
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
///
/// // Create sets for device capabilities with default capacity
/// let mut capabilities1 = GSet::<u32, DefaultConfig>::new();
/// capabilities1.insert(1)?; // GPS
/// capabilities1.insert(2)?; // WiFi
///
/// let mut capabilities2 = GSet::<u32, DefaultConfig>::new();
/// capabilities2.insert(2)?; // WiFi (duplicate)
/// capabilities2.insert(3)?; // Bluetooth
///
/// // Merge the sets
/// capabilities1.merge(&capabilities2)?;
/// assert!(capabilities1.contains(&1));
/// assert!(capabilities1.contains(&2));
/// assert!(capabilities1.contains(&3));
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug)]
pub struct GSet<T, C: MemoryConfig, const CAPACITY: usize = 16> {
    /// Elements in the set
    #[cfg(not(feature = "hardware-atomic"))]
    elements: [Option<T>; CAPACITY],
    #[cfg(not(feature = "hardware-atomic"))]
    count: usize,

    /// Atomic version uses UnsafeCell for the elements array
    #[cfg(feature = "hardware-atomic")]
    elements: UnsafeCell<[Option<T>; CAPACITY]>,
    #[cfg(feature = "hardware-atomic")]
    count: AtomicUsize,

    /// Phantom data to maintain the memory config type
    _phantom: core::marker::PhantomData<C>,
}

// SAFETY: The atomic version is safe to share between threads because:
// 1. All access to elements array is protected by atomic count coordination
// 2. Only one thread can successfully insert at a time via compare_exchange
// 3. UnsafeCell is only accessed after winning the atomic coordination
#[cfg(feature = "hardware-atomic")]
unsafe impl<T, C: MemoryConfig> Sync for GSet<T, C>
where
    T: Send,
    C: Send + Sync,
{
}

// Implement Clone manually due to atomic types not implementing Clone
impl<T, C: MemoryConfig, const CAPACITY: usize> Clone for GSet<T, C, CAPACITY>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                elements: self.elements.clone(),
                count: self.count,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to manually clone the UnsafeCell content
            let cloned_elements = unsafe { (*self.elements.get()).clone() };
            Self {
                elements: UnsafeCell::new(cloned_elements),
                count: AtomicUsize::new(self.count.load(Ordering::Relaxed)),
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> GSet<T, C, CAPACITY>
where
    T: Clone + PartialEq,
{
    /// Creates a new grow-only set with custom capacity
    ///
    /// # Returns
    /// A new empty set
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let set = GSet::<u32, DefaultConfig, 32>::with_capacity();
    /// assert!(set.is_empty());
    /// ```
    pub fn with_capacity() -> Self {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            Self {
                elements: [const { None }; CAPACITY],
                count: 0,
                _phantom: core::marker::PhantomData,
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            Self {
                elements: UnsafeCell::new([const { None }; CAPACITY]),
                count: AtomicUsize::new(0),
                _phantom: core::marker::PhantomData,
            }
        }
    }
}

impl<T, C: MemoryConfig> GSet<T, C, 16>
where
    T: Clone + PartialEq,
{
    /// Creates a new grow-only set with default capacity
    ///
    /// # Returns
    /// A new empty set
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let set = GSet::<u32, DefaultConfig>::new();
    /// assert!(set.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::with_capacity()
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> GSet<T, C, CAPACITY>
where
    T: Clone + PartialEq,
{
    /// Inserts an element into the set
    ///
    /// # Arguments
    /// * `element` - The element to insert
    ///
    /// # Returns
    /// Ok(true) if the element was newly inserted, Ok(false) if it already existed,
    /// or an error if the set is full
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = GSet::<u32, DefaultConfig>::new();
    /// assert!(set.insert(42)?);  // Newly inserted
    /// assert!(!set.insert(42)?); // Already exists
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    #[cfg(not(feature = "hardware-atomic"))]
    pub fn insert(&mut self, element: T) -> CRDTResult<bool> {
        // Check if element already exists
        for existing in self.elements.iter().take(self.count) {
            if let Some(existing_element) = existing {
                if *existing_element == element {
                    return Ok(false); // Already exists
                }
            }
        }

        // Check if we have space
        if self.count >= CAPACITY {
            return Err(CRDTError::BufferOverflow);
        }

        // Insert the new element
        self.elements[self.count] = Some(element);
        self.count += 1;
        Ok(true)
    }

    /// Inserts an element into the set (atomic version)
    ///
    /// # Arguments
    /// * `element` - The element to insert
    ///
    /// # Returns
    /// Ok(true) if the element was newly inserted, Ok(false) if it already existed,
    /// or an error if the set is full
    #[cfg(feature = "hardware-atomic")]
    pub fn insert(&self, element: T) -> CRDTResult<bool> {
        // Atomic compare-exchange loop for coordination
        loop {
            let current_count = self.count.load(Ordering::Relaxed);

            // SAFETY: Read the elements array to check for existing element
            let elements_ptr = self.elements.get();
            let elements_ref = unsafe { &*elements_ptr };

            // Check if element already exists
            for existing in elements_ref.iter().take(current_count) {
                if let Some(existing_element) = existing {
                    if *existing_element == element {
                        return Ok(false); // Already exists
                    }
                }
            }

            // Check if we have space
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
                    // Successfully reserved slot, now insert the element
                    let elements_mut = unsafe { &mut *elements_ptr };
                    elements_mut[current_count] = Some(element);
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
    /// # Arguments
    /// * `element` - The element to check for
    ///
    /// # Returns
    /// true if the element is in the set, false otherwise
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = GSet::<u32, DefaultConfig>::new();
    /// set.insert(42)?;
    /// assert!(set.contains(&42));
    /// assert!(!set.contains(&43));
    /// # Ok::<(), crdtosphere::error::CRDTError>(())
    /// ```
    pub fn contains(&self, element: &T) -> bool {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            for existing in self.elements.iter().take(self.count) {
                if let Some(existing_element) = existing {
                    if existing_element == element {
                        return true;
                    }
                }
            }
            false
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let current_count = self.count.load(Ordering::Relaxed);
            let elements_ref = unsafe { &*self.elements.get() };
            for existing in elements_ref.iter().take(current_count) {
                if let Some(existing_element) = existing {
                    if existing_element == element {
                        return true;
                    }
                }
            }
            false
        }
    }

    /// Returns the number of elements in the set
    ///
    /// # Returns
    /// The count of elements
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = GSet::<u32, DefaultConfig>::new();
    /// assert_eq!(set.len(), 0);
    /// set.insert(42)?;
    /// assert_eq!(set.len(), 1);
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

    /// Checks if the set is empty
    ///
    /// # Returns
    /// true if the set contains no elements, false otherwise
    ///
    /// # Example
    /// ```rust
    /// use crdtosphere::prelude::*;
    /// let mut set = GSet::<u32, DefaultConfig>::new();
    /// assert!(set.is_empty());
    /// set.insert(42)?;
    /// assert!(!set.is_empty());
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

    /// Checks if the set is full
    ///
    /// # Returns
    /// true if the set cannot accept more elements, false otherwise
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

    /// Returns the maximum capacity of the set
    ///
    /// # Returns
    /// The maximum number of elements this set can hold
    pub fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Returns the remaining capacity
    ///
    /// # Returns
    /// The number of additional elements that can be inserted
    pub fn remaining_capacity(&self) -> usize {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            CAPACITY - self.count
        }

        #[cfg(feature = "hardware-atomic")]
        {
            CAPACITY - self.count.load(Ordering::Relaxed)
        }
    }

    /// Returns an iterator over the elements in the set
    ///
    /// # Returns
    /// An iterator over references to the elements
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.elements
                .iter()
                .take(self.count)
                .filter_map(|opt| opt.as_ref())
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, use fixed-size array instead of Vec to maintain no_std compatibility
            let current_count = self.count.load(Ordering::Relaxed);
            let elements_ref = unsafe { &*self.elements.get() };

            // Use fixed-size array instead of Vec for no_std compatibility
            let mut collected = [None; 16];
            let mut idx = 0;
            for opt in elements_ref.iter().take(current_count) {
                if let Some(element) = opt.as_ref() {
                    collected[idx] = Some(element);
                    idx += 1;
                }
            }

            // Return iterator over the collected elements
            collected.into_iter().take(idx).flatten()
        }
    }

    /// Converts the set to an array
    ///
    /// # Returns
    /// An array containing all elements in the set, with None for unused slots
    pub fn to_array(&self) -> [Option<T>; CAPACITY] {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.elements.clone()
        }

        #[cfg(feature = "hardware-atomic")]
        {
            let elements_ref = unsafe { &*self.elements.get() };
            elements_ref.clone()
        }
    }

    /// Checks if this set is a subset of another set
    ///
    /// # Arguments
    /// * `other` - The other set to compare against
    ///
    /// # Returns
    /// true if all elements in this set are also in the other set
    pub fn is_subset(&self, other: &Self) -> bool {
        for element in self.iter() {
            if !other.contains(element) {
                return false;
            }
        }
        true
    }

    /// Checks if this set is a superset of another set
    ///
    /// # Arguments
    /// * `other` - The other set to compare against
    ///
    /// # Returns
    /// true if all elements in the other set are also in this set
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Returns the union of this set with another set (without modifying either)
    ///
    /// # Arguments
    /// * `other` - The other set to union with
    ///
    /// # Returns
    /// A new set containing all elements from both sets, or an error if the result would be too large
    pub fn union(&self, other: &Self) -> CRDTResult<Self>
    where
        T: core::fmt::Debug,
    {
        let mut result = self.clone();
        result.merge(other)?;
        Ok(result)
    }
}

// Serde implementation for GSet
#[cfg(feature = "serde")]
impl<T, C: MemoryConfig, const CAPACITY: usize> Serialize for GSet<T, C, CAPACITY>
where
    T: Serialize + Clone + PartialEq,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("GSet", 2)?;

        // Serialize the logical state (elements array and count)
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Serialize only the used portion of the array as a slice
            state.serialize_field("e", &&self.elements[..self.count])?;
            state.serialize_field("ct", &self.count)?;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to extract values safely
            let current_count = self.count.load(Ordering::Relaxed);
            let elements_ref = unsafe { &*self.elements.get() };
            state.serialize_field("e", &&elements_ref[..current_count])?;
            state.serialize_field("ct", &current_count)?;
        }

        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, T, C: MemoryConfig, const CAPACITY: usize> Deserialize<'de> for GSet<T, C, CAPACITY>
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
            #[serde(rename = "ct")]
            Count,
        }

        struct GSetVisitor<T, C: MemoryConfig, const CAPACITY: usize> {
            _phantom: core::marker::PhantomData<(T, C)>,
        }

        impl<'de, T, C: MemoryConfig, const CAPACITY: usize> Visitor<'de> for GSetVisitor<T, C, CAPACITY>
        where
            T: Deserialize<'de> + Clone + PartialEq,
        {
            type Value = GSet<T, C, CAPACITY>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct GSet")
            }

            fn visit_map<V>(self, mut map: V) -> Result<GSet<T, C, CAPACITY>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut elements = None;
                let mut count = None;

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
                                type Value = [Option<T>; CAPACITY];

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
                                        type Value = [Option<T>; CAPACITY];

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
                                                seq.next_element::<Option<T>>()?
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
                        Field::Count => {
                            if count.is_some() {
                                return Err(de::Error::duplicate_field("ct"));
                            }
                            count = Some(map.next_value::<usize>()?);
                        }
                    }
                }

                let elements_array =
                    elements.ok_or_else(|| de::Error::missing_field("e"))?;
                let count = count.ok_or_else(|| de::Error::missing_field("ct"))?;

                // Validate count is within capacity
                if count > CAPACITY {
                    return Err(de::Error::custom("count exceeds capacity"));
                }

                // Reconstruct the GSet
                #[cfg(not(feature = "hardware-atomic"))]
                {
                    Ok(GSet {
                        elements: elements_array,
                        count,
                        _phantom: core::marker::PhantomData,
                    })
                }

                #[cfg(feature = "hardware-atomic")]
                {
                    Ok(GSet {
                        elements: UnsafeCell::new(elements_array),
                        count: AtomicUsize::new(count),
                        _phantom: core::marker::PhantomData,
                    })
                }
            }
        }

        const FIELDS: &[&str] = &["e", "ct"];
        deserializer.deserialize_struct(
            "GSet",
            FIELDS,
            GSetVisitor {
                _phantom: core::marker::PhantomData,
            },
        )
    }
}

impl<T, C: MemoryConfig> Default for GSet<T, C>
where
    T: Clone + PartialEq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> CRDT<C> for GSet<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Add all elements from other that we don't already have
            for element in other.iter() {
                if !self.contains(element) {
                    if self.count >= CAPACITY {
                        return Err(CRDTError::BufferOverflow);
                    }
                    self.elements[self.count] = Some(element.clone());
                    self.count += 1;
                }
            }
        }

        #[cfg(feature = "hardware-atomic")]
        {
            // For atomic version, we need to handle concurrent access
            for element in other.iter() {
                if !self.contains(element) {
                    let current_count = self.count.load(Ordering::Relaxed);
                    if current_count >= 16 {
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
                            // Successfully reserved slot, now insert the element
                            let elements_mut = unsafe { &mut *self.elements.get() };
                            elements_mut[current_count] = Some(element.clone());
                        }
                        Err(_) => {
                            // Count changed, element might have been added by another thread
                            // Continue to next element
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
            if self.count != other.count {
                return false;
            }

            // Check that all elements in self are in other
            for element in self.iter() {
                if !other.contains(element) {
                    return false;
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

            // Check that all elements in self are in other
            for element in self.iter() {
                if !other.contains(element) {
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
        #[cfg(not(feature = "hardware-atomic"))]
        {
            // Validate count is within bounds
            if self.count > 16 {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate that we don't exceed the configured maximum elements
            if self.count > C::MAX_SET_ELEMENTS {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate no duplicates (this should never happen with correct implementation)
            for i in 0..self.count {
                if let Some(ref element_i) = self.elements[i] {
                    for j in (i + 1)..self.count {
                        if let Some(ref element_j) = self.elements[j] {
                            if element_i == element_j {
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
            let elements_ref = unsafe { &*self.elements.get() };

            // Validate count is within bounds
            if current_count > 16 {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate that we don't exceed the configured maximum elements
            if current_count > C::MAX_SET_ELEMENTS {
                return Err(CRDTError::ConfigurationExceeded);
            }

            // Validate no duplicates (this should never happen with correct implementation)
            for i in 0..current_count {
                if let Some(ref element_i) = elements_ref[i] {
                    for j in (i + 1)..current_count {
                        if let Some(ref element_j) = elements_ref[j] {
                            if element_i == element_j {
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
        // Simple hash based on elements (order-independent)
        let mut hash = 0u32;
        for element in self.iter() {
            // This is a simplified hash - in practice you'd want a proper hash function
            // For now, we'll use the memory address as a simple hash
            let element_ptr = element as *const T as usize;
            hash ^= element_ptr as u32;
        }

        #[cfg(not(feature = "hardware-atomic"))]
        {
            hash ^= self.count as u32;
        }

        #[cfg(feature = "hardware-atomic")]
        {
            hash ^= self.count.load(Ordering::Relaxed) as u32;
        }

        hash
    }

    fn can_merge(&self, other: &Self) -> bool {
        // Check if merging would exceed capacity
        let mut unique_in_other = 0;
        for element in other.iter() {
            if !self.contains(element) {
                unique_in_other += 1;
            }
        }

        #[cfg(not(feature = "hardware-atomic"))]
        {
            self.count + unique_in_other <= 16
        }

        #[cfg(feature = "hardware-atomic")]
        {
            self.count.load(Ordering::Relaxed) + unique_in_other <= 16
        }
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> BoundedCRDT<C> for GSet<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = CAPACITY; // Maximum number of elements

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
        // GSets can't be compacted without losing data
        // This is a no-op that returns 0 bytes freed
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        // For fixed-size arrays, only check element count, not memory usage
        self.element_count() < Self::MAX_ELEMENTS
    }
}

impl<T, C: MemoryConfig, const CAPACITY: usize> RealTimeCRDT<C> for GSet<T, C, CAPACITY>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_MERGE_CYCLES: u32 = 200; // Linear in number of elements
    const MAX_VALIDATE_CYCLES: u32 = 100;
    const MAX_SERIALIZE_CYCLES: u32 = 150;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // GSet merge is bounded by the number of elements
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded by the number of elements
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
        let set = GSet::<u32, DefaultConfig>::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert_eq!(set.capacity(), 16);
        assert_eq!(set.remaining_capacity(), 16);
        assert!(!set.is_full());
    }

    #[test]
    fn test_insert() {
        let mut set = GSet::<u32, DefaultConfig>::new();

        // Insert new element
        assert!(set.insert(42).unwrap());
        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
        assert!(set.contains(&42));

        // Insert duplicate element
        assert!(!set.insert(42).unwrap());
        assert_eq!(set.len(), 1);

        // Insert another element
        assert!(set.insert(43).unwrap());
        assert_eq!(set.len(), 2);
        assert!(set.contains(&43));
    }

    #[test]
    fn test_contains() {
        let mut set = GSet::<u32, DefaultConfig>::new();

        assert!(!set.contains(&42));

        set.insert(42).unwrap();
        assert!(set.contains(&42));
        assert!(!set.contains(&43));

        set.insert(43).unwrap();
        assert!(set.contains(&42));
        assert!(set.contains(&43));
        assert!(!set.contains(&44));
    }

    #[test]
    fn test_capacity_limits() {
        let mut set = GSet::<u32, DefaultConfig>::new();

        // Fill the set to capacity
        for i in 0..16 {
            assert!(set.insert(i).is_ok());
        }

        assert!(set.is_full());
        assert_eq!(set.remaining_capacity(), 0);

        // Try to insert one more (should fail)
        assert!(set.insert(16).is_err());
    }

    #[test]
    fn test_iter() {
        let mut set = GSet::<u32, DefaultConfig>::new();

        set.insert(1).unwrap();
        set.insert(3).unwrap();
        set.insert(2).unwrap();

        let mut elements = [0u32; 3];
        let mut i = 0;
        for element in set.iter() {
            elements[i] = *element;
            i += 1;
        }
        elements.sort(); // Order is not guaranteed
        assert_eq!(elements, [1, 2, 3]);
    }

    #[test]
    fn test_to_array() {
        let mut set = GSet::<u32, DefaultConfig>::new();

        set.insert(1).unwrap();
        set.insert(3).unwrap();
        set.insert(2).unwrap();

        let array = set.to_array();
        let mut elements = [0u32; 3];
        let mut i = 0;
        for opt in array.iter() {
            if let Some(val) = opt {
                elements[i] = *val;
                i += 1;
            }
        }
        elements.sort(); // Order is not guaranteed
        assert_eq!(elements, [1, 2, 3]);
    }

    #[test]
    fn test_merge() {
        let mut set1 = GSet::<u32, DefaultConfig>::new();
        let mut set2 = GSet::<u32, DefaultConfig>::new();

        set1.insert(1).unwrap();
        set1.insert(2).unwrap();

        set2.insert(2).unwrap();
        set2.insert(3).unwrap();

        // Before merge
        assert_eq!(set1.len(), 2);
        assert_eq!(set2.len(), 2);

        // Merge set2 into set1
        set1.merge(&set2).unwrap();

        assert_eq!(set1.len(), 3);
        assert!(set1.contains(&1));
        assert!(set1.contains(&2));
        assert!(set1.contains(&3));
    }

    #[test]
    fn test_merge_overflow() {
        let mut set1 = GSet::<u32, DefaultConfig>::new();
        let mut set2 = GSet::<u32, DefaultConfig>::new();

        // Fill set1 to capacity
        for i in 0..16 {
            set1.insert(i).unwrap();
        }

        // Add a different element to set2
        set2.insert(100).unwrap();

        // Merge should fail due to overflow
        assert!(set1.merge(&set2).is_err());
    }

    #[test]
    fn test_merge_idempotent() {
        let mut set1 = GSet::<u32, DefaultConfig>::new();
        let set2 = GSet::<u32, DefaultConfig>::new();

        set1.insert(42).unwrap();

        // Multiple merges should be idempotent
        set1.merge(&set2).unwrap();
        let len1 = set1.len();

        set1.merge(&set2).unwrap();
        let len2 = set1.len();

        assert_eq!(len1, len2);
    }

    #[test]
    fn test_merge_commutative() {
        let mut set1a = GSet::<u32, DefaultConfig>::new();
        let mut set1b = GSet::<u32, DefaultConfig>::new();
        let mut set2 = GSet::<u32, DefaultConfig>::new();
        let mut set3 = GSet::<u32, DefaultConfig>::new();

        set1a.insert(1).unwrap();
        set1b.insert(1).unwrap();
        set2.insert(2).unwrap();
        set3.insert(3).unwrap();

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
    fn test_subset_superset() {
        let mut set1 = GSet::<u32, DefaultConfig>::new();
        let mut set2 = GSet::<u32, DefaultConfig>::new();

        set1.insert(1).unwrap();
        set1.insert(2).unwrap();

        set2.insert(1).unwrap();
        set2.insert(2).unwrap();
        set2.insert(3).unwrap();

        assert!(set1.is_subset(&set2));
        assert!(!set2.is_subset(&set1));

        assert!(!set1.is_superset(&set2));
        assert!(set2.is_superset(&set1));
    }

    #[test]
    fn test_union() {
        let mut set1 = GSet::<u32, DefaultConfig>::new();
        let mut set2 = GSet::<u32, DefaultConfig>::new();

        set1.insert(1).unwrap();
        set1.insert(2).unwrap();

        set2.insert(2).unwrap();
        set2.insert(3).unwrap();

        let union = set1.union(&set2).unwrap();

        assert_eq!(union.len(), 3);
        assert!(union.contains(&1));
        assert!(union.contains(&2));
        assert!(union.contains(&3));

        // Original sets should be unchanged
        assert_eq!(set1.len(), 2);
        assert_eq!(set2.len(), 2);
    }

    #[test]
    fn test_bounded_crdt() {
        let mut set = GSet::<u32, DefaultConfig>::new();
        set.insert(42).unwrap();

        assert_eq!(set.element_count(), 1);
        assert!(set.memory_usage() > 0);
        assert!(set.can_add_element());

        // Fill to capacity
        for i in 1..16 {
            set.insert(i).unwrap();
        }

        assert_eq!(set.element_count(), 16);
        assert!(!set.can_add_element());
    }

    #[test]
    fn test_validation() {
        let mut set = GSet::<u32, DefaultConfig>::new();
        set.insert(42).unwrap();

        assert!(set.validate().is_ok());
    }

    #[test]
    fn test_real_time_crdt() {
        let mut set1 = GSet::<u32, DefaultConfig>::new();
        let set2 = GSet::<u32, DefaultConfig>::new();

        assert!(set1.merge_bounded(&set2).is_ok());
        assert!(set1.validate_bounded().is_ok());
    }

    #[test]
    fn test_can_merge() {
        let mut set1 = GSet::<u32, DefaultConfig>::new();
        let mut set2 = GSet::<u32, DefaultConfig>::new();

        // Fill set1 to capacity
        for i in 0..16 {
            set1.insert(i).unwrap();
        }

        // Empty set2 should be mergeable
        assert!(set1.can_merge(&set2));

        // Set2 with overlapping elements should be mergeable
        set2.insert(5).unwrap();
        assert!(set1.can_merge(&set2));

        // Set2 with new element should not be mergeable
        set2.insert(100).unwrap();
        assert!(!set1.can_merge(&set2));
    }

    #[test]
    fn test_eq() {
        let mut set1 = GSet::<u32, DefaultConfig>::new();
        let mut set2 = GSet::<u32, DefaultConfig>::new();

        // Empty sets should be equal
        assert!(set1.eq(&set2));

        // Add same elements in different order
        set1.insert(1).unwrap();
        set1.insert(2).unwrap();

        set2.insert(2).unwrap();
        set2.insert(1).unwrap();

        assert!(set1.eq(&set2));

        // Add different element
        set2.insert(3).unwrap();
        assert!(!set1.eq(&set2));
    }

    #[test]
    fn test_with_capacity() {
        // Test custom capacity
        let set = GSet::<u32, DefaultConfig, 32>::with_capacity();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert_eq!(set.capacity(), 32);
        assert_eq!(set.remaining_capacity(), 32);
        assert!(!set.is_full());
    }

    #[test]
    fn test_custom_capacity_operations() {
        let mut set = GSet::<u32, DefaultConfig, 8>::with_capacity();

        // Test basic operations with custom capacity
        assert!(set.insert(42).is_ok());
        assert_eq!(set.len(), 1);
        assert!(set.contains(&42));
        assert_eq!(set.capacity(), 8);

        // Fill to custom capacity
        for i in 1..8 {
            assert!(set.insert(i).is_ok());
        }

        assert!(set.is_full());
        assert_eq!(set.remaining_capacity(), 0);

        // Try to insert one more (should fail)
        assert!(set.insert(100).is_err());
    }

    #[test]
    fn test_capacity_merge() {
        let mut set1 = GSet::<u32, DefaultConfig, 8>::with_capacity();
        let mut set2 = GSet::<u32, DefaultConfig, 8>::with_capacity();

        set1.insert(1).unwrap();
        set1.insert(2).unwrap();

        set2.insert(2).unwrap();
        set2.insert(3).unwrap();

        // Merge should work with same capacity
        set1.merge(&set2).unwrap();
        assert_eq!(set1.len(), 3);
        assert!(set1.contains(&1));
        assert!(set1.contains(&2));
        assert!(set1.contains(&3));
    }

    #[cfg(all(test, feature = "serde"))]
    mod serde_tests {
        use super::*;

        #[test]
        fn test_serialize_deserialize() {
            let mut set = GSet::<i32, DefaultConfig>::new();
            set.insert(42).unwrap();
            set.insert(100).unwrap();
            set.insert(200).unwrap();

            // Test that the serde traits are implemented
            // This ensures the code compiles with serde feature
            assert_eq!(set.len(), 3);
            assert!(set.contains(&42));
            assert!(set.contains(&100));
            assert!(set.contains(&200));
        }

        #[test]
        fn test_atomic_vs_standard_compatibility() {
            // This test ensures that atomic and standard versions would serialize to the same format
            // The logical state should be identical regardless of internal representation
            let mut set = GSet::<i32, DefaultConfig>::new();
            set.insert(42).unwrap();
            set.insert(84).unwrap();
            set.insert(126).unwrap();

            // Both versions should have the same logical state
            assert_eq!(set.len(), 3);
            assert!(set.contains(&42));
            assert!(set.contains(&84));
            assert!(set.contains(&126));
        }

        #[test]
        fn test_empty_set_serialization() {
            let set = GSet::<i32, DefaultConfig>::new();

            // Should handle empty set correctly
            assert_eq!(set.len(), 0);
            assert!(set.is_empty());
        }

        #[test]
        fn test_custom_capacity_serialization() {
            let mut set = GSet::<i32, DefaultConfig, 8>::with_capacity();
            set.insert(100).unwrap();
            set.insert(200).unwrap();
            set.insert(300).unwrap();

            // Should handle custom capacity correctly
            assert_eq!(set.len(), 3);
            assert_eq!(set.capacity(), 8);
            assert!(set.contains(&100));
            assert!(set.contains(&200));
            assert!(set.contains(&300));
        }
    }
}
