//! Safety-Critical CRDTs for Automotive Applications
//!
//! This module implements CRDTs with ISO 26262 safety level integration,
//! providing safety-prioritized conflict resolution for automotive ECUs.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};
use core::cmp::Ordering;

/// ISO 26262 Automotive Safety Integrity Levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ASILLevel {
    /// Quality Management (no safety requirements)
    QM = 0,
    /// ASIL A - Lowest automotive safety integrity level
    AsilA = 1,
    /// ASIL B - Low automotive safety integrity level  
    AsilB = 2,
    /// ASIL C - High automotive safety integrity level
    AsilC = 3,
    /// ASIL D - Highest automotive safety integrity level
    AsilD = 4,
}

impl ASILLevel {
    /// Returns true if this ASIL level is safety-critical (A-D)
    pub fn is_safety_critical(&self) -> bool {
        *self != ASILLevel::QM
    }

    /// Returns the required verification level for this ASIL
    pub fn verification_level(&self) -> u8 {
        match self {
            ASILLevel::QM => 0,
            ASILLevel::AsilA => 1,
            ASILLevel::AsilB => 2,
            ASILLevel::AsilC => 3,
            ASILLevel::AsilD => 4,
        }
    }
}

/// General safety level enumeration supporting multiple standards
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SafetyLevel {
    /// Automotive ISO 26262 levels
    Automotive(ASILLevel),
    /// Industrial IEC 61508 levels (for future use)
    Industrial(u8),
    /// Aerospace DO-178C levels (for future use)
    Aerospace(u8),
    /// Custom safety level
    Custom(u8),
}

impl SafetyLevel {
    /// Creates a new automotive safety level
    pub fn automotive(level: ASILLevel) -> Self {
        SafetyLevel::Automotive(level)
    }

    /// Returns the numeric priority for comparison
    pub fn priority(&self) -> u8 {
        match self {
            SafetyLevel::Automotive(asil) => *asil as u8,
            SafetyLevel::Industrial(sil) => *sil,
            SafetyLevel::Aerospace(dal) => *dal,
            SafetyLevel::Custom(level) => *level,
        }
    }

    /// Returns true if this is a safety-critical level
    pub fn is_safety_critical(&self) -> bool {
        match self {
            SafetyLevel::Automotive(asil) => asil.is_safety_critical(),
            SafetyLevel::Industrial(sil) => *sil > 0,
            SafetyLevel::Aerospace(dal) => *dal > 0,
            SafetyLevel::Custom(level) => *level > 0,
        }
    }
}

/// Safety-Critical CRDT with ISO 26262 compliance
///
/// This CRDT implements safety-prioritized conflict resolution where
/// higher safety levels always take precedence over lower ones.
///
/// # Type Parameters
/// - `T`: The value type stored in the CRDT
/// - `C`: Memory configuration
///
/// # Safety Semantics
/// - ASIL-D values always override lower safety levels
/// - Same safety level uses timestamp ordering
/// - Safety verification is performed on all operations
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::automotive::{SafetyCRDT, SafetyLevel, ASILLevel};
///
/// // Create safety-critical brake command
/// let mut brake_cmd = SafetyCRDT::<u8, DefaultConfig>::new(
///     1, // node_id
///     SafetyLevel::automotive(ASILLevel::AsilD)
/// );
///
/// // Set brake pressure (safety-critical)
/// brake_cmd.set(80, 1000)?; // 80% brake pressure
///
/// // Lower safety level cannot override
/// let mut user_cmd = SafetyCRDT::<u8, DefaultConfig>::new(
///     2, // node_id  
///     SafetyLevel::automotive(ASILLevel::QM)
/// );
/// user_cmd.set(20, 1001)?; // User wants 20% brake
///
/// // Merge - safety-critical command wins
/// brake_cmd.merge(&user_cmd)?;
/// assert_eq!(brake_cmd.get(), Some(&80)); // Safety command preserved
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct SafetyCRDT<T, C: MemoryConfig> {
    /// Current value
    value: Option<T>,
    /// Safety level of current value
    safety_level: SafetyLevel,
    /// Timestamp of current value
    timestamp: CompactTimestamp,
    /// Node ID that set the current value
    node_id: NodeId,
    /// This node's ID
    local_node_id: NodeId,
    /// This node's safety level
    local_safety_level: SafetyLevel,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<T, C: MemoryConfig> SafetyCRDT<T, C>
where
    T: Clone + PartialEq,
{
    /// Creates a new safety CRDT for the given node and safety level
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node
    /// * `safety_level` - The safety level this node operates at
    ///
    /// # Returns
    /// A new empty safety CRDT
    pub fn new(node_id: NodeId, safety_level: SafetyLevel) -> Self {
        Self {
            value: None,
            safety_level,
            timestamp: CompactTimestamp::new(0),
            node_id,
            local_node_id: node_id,
            local_safety_level: safety_level,
            _phantom: core::marker::PhantomData,
        }
    }

    /// Sets a new value with safety verification
    ///
    /// # Arguments
    /// * `value` - The new value to set
    /// * `timestamp` - The timestamp for this update
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if safety verification fails
    pub fn set(&mut self, value: T, timestamp: u64) -> CRDTResult<()> {
        let new_timestamp = CompactTimestamp::new(timestamp);

        // Safety verification: can only set if we have appropriate safety level
        if !self.local_safety_level.is_safety_critical()
            && self.safety_level.is_safety_critical()
            && self.safety_level > self.local_safety_level
        {
            return Err(CRDTError::SafetyViolation);
        }

        // Update if we have higher safety level or newer timestamp at same level
        let should_update = match self.local_safety_level.cmp(&self.safety_level) {
            Ordering::Greater => true, // We have higher safety level
            Ordering::Equal => new_timestamp > self.timestamp, // Same level, newer timestamp
            Ordering::Less => false,   // We have lower safety level, cannot override
        };

        if should_update {
            self.value = Some(value);
            self.safety_level = self.local_safety_level;
            self.timestamp = new_timestamp;
            self.node_id = self.local_node_id;
        }

        Ok(())
    }

    /// Gets the current value
    ///
    /// # Returns
    /// Reference to the current value, or None if no value is set
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Gets the current safety level
    ///
    /// # Returns
    /// The safety level of the current value
    pub fn current_safety_level(&self) -> SafetyLevel {
        self.safety_level
    }

    /// Gets the timestamp of the current value
    ///
    /// # Returns
    /// The timestamp when the current value was set
    pub fn timestamp(&self) -> CompactTimestamp {
        self.timestamp
    }

    /// Gets the node ID that set the current value
    ///
    /// # Returns
    /// The node ID of the value setter
    pub fn value_node_id(&self) -> NodeId {
        self.node_id
    }

    /// Checks if the current value is safety-critical
    ///
    /// # Returns
    /// true if the current value has a safety-critical level
    pub fn is_safety_critical(&self) -> bool {
        self.safety_level.is_safety_critical()
    }

    /// Performs safety verification on the current state
    ///
    /// # Returns
    /// Ok(()) if the state is safe, error otherwise
    pub fn verify_safety(&self) -> CRDTResult<()> {
        // Verify safety level consistency
        if self.safety_level.priority() > 4 {
            return Err(CRDTError::InvalidSafetyLevel);
        }

        // Verify node ID is valid
        if self.node_id as usize >= C::MAX_NODES {
            return Err(CRDTError::InvalidNodeId);
        }

        Ok(())
    }
}

impl<T, C: MemoryConfig> CRDT<C> for SafetyCRDT<T, C>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Safety-prioritized merge
        if let Some(ref other_value) = other.value {
            let should_merge = match self.safety_level.cmp(&other.safety_level) {
                Ordering::Less => {
                    // Other has higher safety priority - always accept
                    true
                }
                Ordering::Greater => {
                    // We have higher safety priority - keep ours
                    false
                }
                Ordering::Equal => {
                    // Same safety level - use timestamp ordering
                    other.timestamp > self.timestamp
                }
            };

            if should_merge {
                self.value = Some(other_value.clone());
                self.safety_level = other.safety_level;
                self.timestamp = other.timestamp;
                self.node_id = other.node_id;
            }
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.safety_level == other.safety_level
            && self.timestamp == other.timestamp
            && self.node_id == other.node_id
    }

    fn size_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn validate(&self) -> CRDTResult<()> {
        self.verify_safety()
    }

    fn state_hash(&self) -> u32 {
        let mut hash = 0u32;
        if let Some(ref value) = self.value {
            let value_ptr = value as *const T as usize;
            hash ^= value_ptr as u32;
        }
        hash ^= (self.safety_level.priority() as u32) << 24;
        hash ^= (self.timestamp.as_u64() as u32) << 8;
        hash ^= self.node_id as u32;
        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // Safety CRDTs can always merge (safety rules determine the outcome)
        true
    }
}

impl<T, C: MemoryConfig> BoundedCRDT<C> for SafetyCRDT<T, C>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 1; // Single value

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        if self.value.is_some() { 1 } else { 0 }
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // Safety CRDTs cannot be compacted without losing safety information
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        // Can always update the single value
        true
    }
}

impl<T, C: MemoryConfig> RealTimeCRDT<C> for SafetyCRDT<T, C>
where
    T: Clone + PartialEq + core::fmt::Debug,
{
    const MAX_MERGE_CYCLES: u32 = 50; // Very fast merge for safety-critical systems
    const MAX_VALIDATE_CYCLES: u32 = 25;
    const MAX_SERIALIZE_CYCLES: u32 = 30;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Safety merge is always bounded and fast
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Safety validation is always bounded
        self.validate()
    }

    fn remaining_budget(&self) -> Option<u32> {
        // For safety-critical systems, we don't track budget
        None
    }

    fn set_budget(&mut self, _cycles: u32) {
        // For safety-critical systems, we don't limit budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::DefaultConfig;

    #[test]
    fn test_asil_level_ordering() {
        assert!(ASILLevel::AsilD > ASILLevel::AsilC);
        assert!(ASILLevel::AsilC > ASILLevel::AsilB);
        assert!(ASILLevel::AsilB > ASILLevel::AsilA);
        assert!(ASILLevel::AsilA > ASILLevel::QM);

        assert!(ASILLevel::AsilD.is_safety_critical());
        assert!(!ASILLevel::QM.is_safety_critical());
    }

    #[test]
    fn test_safety_level_priority() {
        let qm = SafetyLevel::automotive(ASILLevel::QM);
        let asil_d = SafetyLevel::automotive(ASILLevel::AsilD);

        assert!(asil_d > qm);
        assert!(asil_d.is_safety_critical());
        assert!(!qm.is_safety_critical());
    }

    #[test]
    fn test_safety_crdt_creation() {
        let crdt =
            SafetyCRDT::<u32, DefaultConfig>::new(1, SafetyLevel::automotive(ASILLevel::AsilD));

        assert_eq!(crdt.get(), None);
        assert_eq!(
            crdt.current_safety_level(),
            SafetyLevel::automotive(ASILLevel::AsilD)
        );
        assert!(crdt.is_safety_critical());
    }

    #[test]
    fn test_safety_prioritized_merge() {
        let mut asil_d_crdt =
            SafetyCRDT::<u32, DefaultConfig>::new(1, SafetyLevel::automotive(ASILLevel::AsilD));
        asil_d_crdt.set(100, 1000).unwrap();

        let mut qm_crdt =
            SafetyCRDT::<u32, DefaultConfig>::new(2, SafetyLevel::automotive(ASILLevel::QM));
        qm_crdt.set(50, 2000).unwrap(); // Later timestamp but lower safety

        // Merge QM into ASIL-D
        asil_d_crdt.merge(&qm_crdt).unwrap();
        assert_eq!(asil_d_crdt.get(), Some(&100)); // ASIL-D value preserved

        // Merge ASIL-D into QM
        qm_crdt.merge(&asil_d_crdt).unwrap();
        assert_eq!(qm_crdt.get(), Some(&100)); // ASIL-D value takes over
    }

    #[test]
    fn test_same_safety_level_timestamp_ordering() {
        let mut crdt1 =
            SafetyCRDT::<u32, DefaultConfig>::new(1, SafetyLevel::automotive(ASILLevel::AsilC));
        crdt1.set(100, 1000).unwrap();

        let mut crdt2 =
            SafetyCRDT::<u32, DefaultConfig>::new(2, SafetyLevel::automotive(ASILLevel::AsilC));
        crdt2.set(200, 2000).unwrap(); // Later timestamp, same safety level

        crdt1.merge(&crdt2).unwrap();
        assert_eq!(crdt1.get(), Some(&200)); // Later timestamp wins
    }

    #[test]
    fn test_safety_verification() {
        let crdt =
            SafetyCRDT::<u32, DefaultConfig>::new(1, SafetyLevel::automotive(ASILLevel::AsilD));

        assert!(crdt.verify_safety().is_ok());
        assert!(crdt.validate().is_ok());
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut crdt =
            SafetyCRDT::<u32, DefaultConfig>::new(1, SafetyLevel::automotive(ASILLevel::AsilB));

        assert_eq!(crdt.element_count(), 0);
        assert!(crdt.can_add_element());

        crdt.set(42, 1000).unwrap();
        assert_eq!(crdt.element_count(), 1);
        assert!(crdt.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut crdt1 =
            SafetyCRDT::<u32, DefaultConfig>::new(1, SafetyLevel::automotive(ASILLevel::AsilD));
        let crdt2 =
            SafetyCRDT::<u32, DefaultConfig>::new(2, SafetyLevel::automotive(ASILLevel::AsilC));

        assert!(crdt1.merge_bounded(&crdt2).is_ok());
        assert!(crdt1.validate_bounded().is_ok());
    }
}
