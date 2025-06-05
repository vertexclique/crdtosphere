//! Equipment Registry for Industrial Systems
//!
//! This module implements CRDTs for distributed industrial equipment management,
//! enabling coordination of equipment states and maintenance across systems.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

/// Industrial equipment operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum EquipmentStatus {
    /// Equipment is offline/powered down
    Offline = 0,
    /// Equipment is starting up
    Starting = 1,
    /// Equipment is idle/ready
    Idle = 2,
    /// Equipment is running/operational
    Running = 3,
    /// Equipment is stopping
    Stopping = 4,
    /// Equipment has a warning
    Warning = 5,
    /// Equipment has an error
    Error = 6,
    /// Equipment is in emergency stop
    Emergency = 7,
    /// Equipment is in maintenance mode
    Maintenance = 8,
}

impl EquipmentStatus {
    /// Returns true if equipment is operational
    pub fn is_operational(&self) -> bool {
        matches!(self, EquipmentStatus::Idle | EquipmentStatus::Running)
    }

    /// Returns true if equipment requires attention
    pub fn requires_attention(&self) -> bool {
        matches!(self, EquipmentStatus::Error | EquipmentStatus::Emergency)
    }

    /// Returns true if equipment can be started
    pub fn can_start(&self) -> bool {
        matches!(self, EquipmentStatus::Offline | EquipmentStatus::Idle)
    }

    /// Returns true if equipment can be stopped
    pub fn can_stop(&self) -> bool {
        matches!(
            self,
            EquipmentStatus::Running | EquipmentStatus::Warning | EquipmentStatus::Error
        )
    }
}

/// Equipment maintenance states
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum MaintenanceState {
    /// No maintenance required
    None = 0,
    /// Preventive maintenance due soon
    PreventiveDue = 1,
    /// Preventive maintenance overdue
    PreventiveOverdue = 2,
    /// Corrective maintenance required
    CorrectiveRequired = 3,
    /// Emergency maintenance required
    EmergencyRequired = 4,
    /// Maintenance in progress
    InProgress = 5,
    /// Maintenance completed
    Completed = 6,
}

impl MaintenanceState {
    /// Returns true if maintenance is required
    pub fn requires_maintenance(&self) -> bool {
        matches!(
            self,
            MaintenanceState::PreventiveOverdue
                | MaintenanceState::CorrectiveRequired
                | MaintenanceState::EmergencyRequired
        )
    }

    /// Returns true if maintenance is urgent
    pub fn is_urgent(&self) -> bool {
        matches!(
            self,
            MaintenanceState::EmergencyRequired | MaintenanceState::CorrectiveRequired
        )
    }

    /// Returns priority level (higher number = higher priority)
    pub fn priority_level(&self) -> u8 {
        match self {
            MaintenanceState::None => 0,
            MaintenanceState::Completed => 0,
            MaintenanceState::PreventiveDue => 1,
            MaintenanceState::InProgress => 2,
            MaintenanceState::PreventiveOverdue => 3,
            MaintenanceState::CorrectiveRequired => 4,
            MaintenanceState::EmergencyRequired => 5,
        }
    }
}

/// Individual equipment information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipmentInfo {
    /// Equipment unique identifier
    pub equipment_id: NodeId,
    /// Equipment type/category (encoded as u16)
    pub equipment_type: u16,
    /// Current operational status
    pub status: EquipmentStatus,
    /// Current maintenance state
    pub maintenance_state: MaintenanceState,
    /// Operating hours (scaled)
    pub operating_hours: u32,
    /// Cycle count (for cyclic equipment)
    pub cycle_count: u32,
    /// Last maintenance timestamp
    pub last_maintenance: CompactTimestamp,
    /// Next maintenance due timestamp
    pub next_maintenance_due: CompactTimestamp,
    /// Last update timestamp
    pub last_update: CompactTimestamp,
    /// Controller that manages this equipment
    pub controller_id: NodeId,
}

impl EquipmentInfo {
    /// Creates new equipment info
    pub fn new(
        equipment_id: NodeId,
        equipment_type: u16,
        controller_id: NodeId,
        timestamp: u64,
    ) -> Self {
        Self {
            equipment_id,
            equipment_type,
            status: EquipmentStatus::Offline,
            maintenance_state: MaintenanceState::None,
            operating_hours: 0,
            cycle_count: 0,
            last_maintenance: CompactTimestamp::new(timestamp),
            next_maintenance_due: CompactTimestamp::new(timestamp + 86400000), // 24 hours default
            last_update: CompactTimestamp::new(timestamp),
            controller_id,
        }
    }

    /// Updates equipment status
    pub fn update_status(&mut self, status: EquipmentStatus, timestamp: u64) {
        self.status = status;
        self.last_update = CompactTimestamp::new(timestamp);
    }

    /// Updates maintenance state
    pub fn update_maintenance(&mut self, state: MaintenanceState, timestamp: u64) {
        self.maintenance_state = state;
        self.last_update = CompactTimestamp::new(timestamp);

        // If maintenance completed, update last maintenance time
        if state == MaintenanceState::Completed {
            self.last_maintenance = CompactTimestamp::new(timestamp);
            // Set next maintenance due (example: 30 days)
            self.next_maintenance_due = CompactTimestamp::new(timestamp + 30 * 86400000);
        }
    }

    /// Updates operating metrics
    pub fn update_metrics(&mut self, operating_hours: u32, cycle_count: u32, timestamp: u64) {
        self.operating_hours = operating_hours;
        self.cycle_count = cycle_count;
        self.last_update = CompactTimestamp::new(timestamp);
    }

    /// Sets next maintenance due time
    pub fn set_maintenance_due(&mut self, due_timestamp: u64, current_timestamp: u64) {
        self.next_maintenance_due = CompactTimestamp::new(due_timestamp);
        self.last_update = CompactTimestamp::new(current_timestamp);
    }

    /// Returns true if this equipment info should override another
    pub fn should_override(&self, other: &EquipmentInfo) -> bool {
        // More recent updates win
        self.last_update > other.last_update
    }

    /// Returns true if maintenance is overdue
    pub fn is_maintenance_overdue(&self, current_time: u64) -> bool {
        current_time > self.next_maintenance_due.as_u64()
    }

    /// Returns time until next maintenance (negative if overdue)
    pub fn time_until_maintenance(&self, current_time: u64) -> i64 {
        self.next_maintenance_due.as_u64() as i64 - current_time as i64
    }
}

/// Industrial Equipment Registry CRDT
///
/// This CRDT manages distributed equipment state coordination across industrial systems,
/// enabling synchronization of equipment status and maintenance scheduling.
///
/// # Type Parameters
/// - `C`: Memory configuration
///
/// # Features
/// - Equipment status tracking
/// - Maintenance state coordination
/// - Operating metrics monitoring
/// - Maintenance scheduling
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::industrial::{EquipmentRegistry, EquipmentStatus, MaintenanceState};
///
/// // Create equipment registry
/// let mut registry = EquipmentRegistry::<DefaultConfig>::new(1); // Controller ID 1
///
/// // Register new equipment
/// registry.register_equipment(
///     42,     // equipment ID
///     0x2001, // equipment type (motor)
///     1000    // timestamp
/// )?;
///
/// // Update equipment status
/// registry.update_equipment_status(42, EquipmentStatus::Running, 1001)?;
///
/// // Schedule maintenance
/// registry.schedule_maintenance(42, 1000 + 86400000, 1002)?; // 24 hours from now
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct EquipmentRegistry<C: MemoryConfig> {
    /// Array of equipment information
    equipment: [Option<EquipmentInfo>; 64], // Support up to 64 equipment items
    /// Number of equipment items currently registered
    equipment_count: usize,
    /// This controller's ID
    local_controller_id: NodeId,
    /// Last update timestamp
    last_update: CompactTimestamp,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<C: MemoryConfig> EquipmentRegistry<C> {
    /// Creates a new equipment registry
    ///
    /// # Arguments
    /// * `controller_id` - The ID of this controller
    ///
    /// # Returns
    /// A new equipment registry CRDT
    pub fn new(controller_id: NodeId) -> Self {
        Self {
            equipment: [const { None }; 64],
            equipment_count: 0,
            local_controller_id: controller_id,
            last_update: CompactTimestamp::new(0),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Registers new equipment
    ///
    /// # Arguments
    /// * `equipment_id` - Equipment identifier
    /// * `equipment_type` - Equipment type/category
    /// * `timestamp` - Registration timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn register_equipment(
        &mut self,
        equipment_id: NodeId,
        equipment_type: u16,
        timestamp: u64,
    ) -> CRDTResult<()> {
        let equipment_info = EquipmentInfo::new(
            equipment_id,
            equipment_type,
            self.local_controller_id,
            timestamp,
        );
        self.add_equipment_info(equipment_info)?;
        self.last_update = CompactTimestamp::new(timestamp);
        Ok(())
    }

    /// Updates equipment status
    ///
    /// # Arguments
    /// * `equipment_id` - Equipment to update
    /// * `status` - New status
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_equipment_status(
        &mut self,
        equipment_id: NodeId,
        status: EquipmentStatus,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(equipment) = self.find_equipment_mut(equipment_id) {
            equipment.update_status(status, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Updates equipment maintenance state
    ///
    /// # Arguments
    /// * `equipment_id` - Equipment to update
    /// * `state` - New maintenance state
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_maintenance_state(
        &mut self,
        equipment_id: NodeId,
        state: MaintenanceState,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(equipment) = self.find_equipment_mut(equipment_id) {
            equipment.update_maintenance(state, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Updates equipment operating metrics
    ///
    /// # Arguments
    /// * `equipment_id` - Equipment to update
    /// * `operating_hours` - Total operating hours
    /// * `cycle_count` - Total cycle count
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_equipment_metrics(
        &mut self,
        equipment_id: NodeId,
        operating_hours: u32,
        cycle_count: u32,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(equipment) = self.find_equipment_mut(equipment_id) {
            equipment.update_metrics(operating_hours, cycle_count, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Schedules maintenance for equipment
    ///
    /// # Arguments
    /// * `equipment_id` - Equipment to schedule maintenance for
    /// * `due_timestamp` - When maintenance is due
    /// * `timestamp` - Scheduling timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn schedule_maintenance(
        &mut self,
        equipment_id: NodeId,
        due_timestamp: u64,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(equipment) = self.find_equipment_mut(equipment_id) {
            equipment.set_maintenance_due(due_timestamp, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Gets all equipment
    ///
    /// # Returns
    /// Iterator over equipment info
    pub fn all_equipment(&self) -> impl Iterator<Item = &EquipmentInfo> {
        self.equipment.iter().filter_map(|e| e.as_ref())
    }

    /// Gets equipment by status
    ///
    /// # Arguments
    /// * `status` - Status to filter by
    ///
    /// # Returns
    /// Iterator over equipment with the specified status
    pub fn equipment_by_status(
        &self,
        status: EquipmentStatus,
    ) -> impl Iterator<Item = &EquipmentInfo> {
        self.all_equipment().filter(move |e| e.status == status)
    }

    /// Gets running equipment
    ///
    /// # Returns
    /// Iterator over running equipment
    pub fn running_equipment(&self) -> impl Iterator<Item = &EquipmentInfo> {
        self.equipment_by_status(EquipmentStatus::Running)
    }

    /// Gets equipment requiring attention
    ///
    /// # Returns
    /// Iterator over equipment with errors or emergency states
    pub fn equipment_requiring_attention(&self) -> impl Iterator<Item = &EquipmentInfo> {
        self.all_equipment()
            .filter(|e| e.status.requires_attention())
    }

    /// Gets equipment by maintenance state
    ///
    /// # Arguments
    /// * `state` - Maintenance state to filter by
    ///
    /// # Returns
    /// Iterator over equipment with the specified maintenance state
    pub fn equipment_by_maintenance_state(
        &self,
        state: MaintenanceState,
    ) -> impl Iterator<Item = &EquipmentInfo> {
        self.all_equipment()
            .filter(move |e| e.maintenance_state == state)
    }

    /// Gets equipment requiring maintenance
    ///
    /// # Returns
    /// Iterator over equipment requiring maintenance
    pub fn equipment_requiring_maintenance(&self) -> impl Iterator<Item = &EquipmentInfo> {
        self.all_equipment()
            .filter(|e| e.maintenance_state.requires_maintenance())
    }

    /// Gets equipment with overdue maintenance
    ///
    /// # Arguments
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    /// Iterator over equipment with overdue maintenance
    pub fn equipment_with_overdue_maintenance(
        &self,
        current_time: u64,
    ) -> impl Iterator<Item = &EquipmentInfo> {
        self.all_equipment()
            .filter(move |e| e.is_maintenance_overdue(current_time))
    }

    /// Gets equipment by type
    ///
    /// # Arguments
    /// * `equipment_type` - Equipment type to filter by
    ///
    /// # Returns
    /// Iterator over equipment of the specified type
    pub fn equipment_by_type(&self, equipment_type: u16) -> impl Iterator<Item = &EquipmentInfo> {
        self.all_equipment()
            .filter(move |e| e.equipment_type == equipment_type)
    }

    /// Gets equipment by controller
    ///
    /// # Arguments
    /// * `controller_id` - Controller ID to filter by
    ///
    /// # Returns
    /// Iterator over equipment managed by the controller
    pub fn equipment_by_controller(
        &self,
        controller_id: NodeId,
    ) -> impl Iterator<Item = &EquipmentInfo> {
        self.all_equipment()
            .filter(move |e| e.controller_id == controller_id)
    }

    /// Gets equipment by ID
    ///
    /// # Arguments
    /// * `equipment_id` - Equipment ID to look up
    ///
    /// # Returns
    /// Equipment info if found
    pub fn get_equipment(&self, equipment_id: NodeId) -> Option<&EquipmentInfo> {
        self.all_equipment()
            .find(|e| e.equipment_id == equipment_id)
    }

    /// Gets the number of equipment items
    ///
    /// # Returns
    /// Number of equipment items
    pub fn equipment_count(&self) -> usize {
        self.equipment_count
    }

    /// Emergency stops all equipment
    ///
    /// # Arguments
    /// * `timestamp` - Emergency stop timestamp
    ///
    /// # Returns
    /// Number of equipment items stopped
    pub fn emergency_stop_all(&mut self, timestamp: u64) -> usize {
        let mut stopped = 0;

        for i in 0..64 {
            if let Some(ref mut equipment) = self.equipment[i] {
                if equipment.status.is_operational() {
                    equipment.update_status(EquipmentStatus::Emergency, timestamp);
                    stopped += 1;
                }
            }
        }

        if stopped > 0 {
            self.last_update = CompactTimestamp::new(timestamp);
        }

        stopped
    }

    /// Finds equipment by ID (mutable)
    fn find_equipment_mut(&mut self, equipment_id: NodeId) -> Option<&mut EquipmentInfo> {
        for equipment_opt in &mut self.equipment {
            if let Some(equipment) = equipment_opt {
                if equipment.equipment_id == equipment_id {
                    return Some(equipment);
                }
            }
        }
        None
    }

    /// Adds equipment info to the registry
    fn add_equipment_info(&mut self, equipment_info: EquipmentInfo) -> CRDTResult<()> {
        // Check for existing equipment
        for i in 0..64 {
            if let Some(ref mut existing) = self.equipment[i] {
                if existing.equipment_id == equipment_info.equipment_id {
                    // Update if new info should override
                    if equipment_info.should_override(existing) {
                        *existing = equipment_info;
                    }
                    return Ok(());
                }
            } else {
                // Empty slot - add new equipment
                self.equipment[i] = Some(equipment_info);
                self.equipment_count += 1;
                return Ok(());
            }
        }

        // If no space, try to replace oldest offline equipment
        self.make_space_for_equipment(equipment_info)
    }

    /// Makes space for new equipment by replacing old offline equipment
    fn make_space_for_equipment(&mut self, new_equipment: EquipmentInfo) -> CRDTResult<()> {
        // Find oldest offline equipment to replace
        let mut oldest_idx = None;
        let mut oldest_time = u64::MAX;

        for (i, equipment_opt) in self.equipment.iter().enumerate() {
            if let Some(equipment) = equipment_opt {
                if equipment.status == EquipmentStatus::Offline
                    && equipment.last_update.as_u64() < oldest_time
                {
                    oldest_time = equipment.last_update.as_u64();
                    oldest_idx = Some(i);
                }
            }
        }

        if let Some(idx) = oldest_idx {
            self.equipment[idx] = Some(new_equipment);
            Ok(())
        } else {
            Err(CRDTError::BufferOverflow)
        }
    }

    /// Validates equipment registry data
    ///
    /// # Returns
    /// Ok(()) if valid, error otherwise
    pub fn validate_registry(&self) -> CRDTResult<()> {
        // Check equipment IDs are valid
        for equipment in self.all_equipment() {
            if equipment.equipment_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
            if equipment.controller_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        Ok(())
    }
}

impl<C: MemoryConfig> CRDT<C> for EquipmentRegistry<C> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Merge all equipment from other
        for equipment in other.all_equipment() {
            self.add_equipment_info(*equipment)?;
        }

        // Update timestamp to latest
        if other.last_update > self.last_update {
            self.last_update = other.last_update;
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        if self.equipment_count != other.equipment_count {
            return false;
        }

        // Check that all equipment matches
        for equipment in self.all_equipment() {
            let mut found = false;
            for other_equipment in other.all_equipment() {
                if equipment.equipment_id == other_equipment.equipment_id
                    && equipment == other_equipment
                {
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }

        true
    }

    fn size_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn validate(&self) -> CRDTResult<()> {
        self.validate_registry()
    }

    fn state_hash(&self) -> u32 {
        let mut hash = self.local_controller_id as u32;
        for equipment in self.all_equipment() {
            hash ^= (equipment.equipment_id as u32)
                ^ (equipment.last_update.as_u64() as u32)
                ^ (equipment.status as u32);
        }
        hash ^= self.equipment_count as u32;
        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // Can always merge equipment registries (space is made by removing old equipment)
        true
    }
}

impl<C: MemoryConfig> BoundedCRDT<C> for EquipmentRegistry<C> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 64; // Maximum equipment items

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.equipment_count
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // Could implement cleanup of old offline equipment
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        self.equipment_count < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig> RealTimeCRDT<C> for EquipmentRegistry<C> {
    const MAX_MERGE_CYCLES: u32 = 200; // Bounded by number of equipment items
    const MAX_VALIDATE_CYCLES: u32 = 100;
    const MAX_SERIALIZE_CYCLES: u32 = 150;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Equipment registry merge is bounded
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded
        self.validate()
    }

    fn remaining_budget(&self) -> Option<u32> {
        // For industrial systems, we don't track budget
        None
    }

    fn set_budget(&mut self, _cycles: u32) {
        // For industrial systems, we don't limit budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::DefaultConfig;

    #[test]
    fn test_equipment_status_properties() {
        assert!(EquipmentStatus::Running.is_operational());
        assert!(EquipmentStatus::Idle.is_operational());
        assert!(!EquipmentStatus::Offline.is_operational());

        assert!(EquipmentStatus::Error.requires_attention());
        assert!(EquipmentStatus::Emergency.requires_attention());
        assert!(!EquipmentStatus::Running.requires_attention());

        assert!(EquipmentStatus::Offline.can_start());
        assert!(EquipmentStatus::Idle.can_start());
        assert!(!EquipmentStatus::Running.can_start());

        assert!(EquipmentStatus::Running.can_stop());
        assert!(EquipmentStatus::Error.can_stop());
        assert!(!EquipmentStatus::Offline.can_stop());
    }

    #[test]
    fn test_maintenance_state_properties() {
        assert!(MaintenanceState::CorrectiveRequired.requires_maintenance());
        assert!(MaintenanceState::EmergencyRequired.requires_maintenance());
        assert!(!MaintenanceState::None.requires_maintenance());

        assert!(MaintenanceState::EmergencyRequired.is_urgent());
        assert!(MaintenanceState::CorrectiveRequired.is_urgent());
        assert!(!MaintenanceState::PreventiveDue.is_urgent());

        assert!(
            MaintenanceState::EmergencyRequired.priority_level()
                > MaintenanceState::CorrectiveRequired.priority_level()
        );
        assert!(
            MaintenanceState::CorrectiveRequired.priority_level()
                > MaintenanceState::PreventiveDue.priority_level()
        );
    }

    #[test]
    fn test_equipment_info_creation() {
        let equipment = EquipmentInfo::new(42, 0x2001, 1, 1000);

        assert_eq!(equipment.equipment_id, 42);
        assert_eq!(equipment.equipment_type, 0x2001);
        assert_eq!(equipment.controller_id, 1);
        assert_eq!(equipment.status, EquipmentStatus::Offline);
        assert_eq!(equipment.maintenance_state, MaintenanceState::None);
    }

    #[test]
    fn test_equipment_info_updates() {
        let mut equipment = EquipmentInfo::new(42, 0x2001, 1, 1000);

        equipment.update_status(EquipmentStatus::Running, 1001);
        assert_eq!(equipment.status, EquipmentStatus::Running);

        equipment.update_maintenance(MaintenanceState::PreventiveDue, 1002);
        assert_eq!(equipment.maintenance_state, MaintenanceState::PreventiveDue);

        equipment.update_metrics(1000, 500, 1003);
        assert_eq!(equipment.operating_hours, 1000);
        assert_eq!(equipment.cycle_count, 500);

        // Test maintenance completion
        equipment.update_maintenance(MaintenanceState::Completed, 1004);
        assert_eq!(equipment.last_maintenance.as_u64(), 1004);
        assert!(equipment.next_maintenance_due.as_u64() > 1004);
    }

    #[test]
    fn test_maintenance_timing() {
        let mut equipment = EquipmentInfo::new(42, 0x2001, 1, 1000);
        equipment.set_maintenance_due(2000, 1000);

        assert!(!equipment.is_maintenance_overdue(1500)); // Not overdue yet
        assert!(equipment.is_maintenance_overdue(2500)); // Overdue

        assert_eq!(equipment.time_until_maintenance(1500), 500); // 500ms until due
        assert_eq!(equipment.time_until_maintenance(2500), -500); // 500ms overdue
    }

    #[test]
    fn test_equipment_registry_creation() {
        let registry = EquipmentRegistry::<DefaultConfig>::new(1);

        assert_eq!(registry.equipment_count(), 0);
        assert_eq!(registry.local_controller_id, 1);
    }

    #[test]
    fn test_equipment_registration_and_updates() {
        let mut registry = EquipmentRegistry::<DefaultConfig>::new(1);

        // Register equipment
        registry.register_equipment(42, 0x2001, 1000).unwrap();
        assert_eq!(registry.equipment_count(), 1);

        // Update status
        registry
            .update_equipment_status(42, EquipmentStatus::Running, 1001)
            .unwrap();

        // Update maintenance state
        registry
            .update_maintenance_state(42, MaintenanceState::PreventiveDue, 1002)
            .unwrap();

        // Update metrics
        registry
            .update_equipment_metrics(42, 1000, 500, 1003)
            .unwrap();

        // Schedule maintenance
        registry.schedule_maintenance(42, 2000, 1004).unwrap();

        let equipment = registry.get_equipment(42).unwrap();
        assert_eq!(equipment.status, EquipmentStatus::Running);
        assert_eq!(equipment.maintenance_state, MaintenanceState::PreventiveDue);
        assert_eq!(equipment.operating_hours, 1000);
        assert_eq!(equipment.cycle_count, 500);
        assert_eq!(equipment.next_maintenance_due.as_u64(), 2000);
    }

    #[test]
    fn test_equipment_registry_queries() {
        let mut registry = EquipmentRegistry::<DefaultConfig>::new(1);

        // Register multiple equipment items
        registry.register_equipment(1, 0x2001, 1000).unwrap(); // Motor
        registry.register_equipment(2, 0x2002, 1001).unwrap(); // Pump
        registry.register_equipment(3, 0x2001, 1002).unwrap(); // Another motor

        // Update states
        registry
            .update_equipment_status(1, EquipmentStatus::Running, 1003)
            .unwrap();
        registry
            .update_equipment_status(2, EquipmentStatus::Error, 1004)
            .unwrap();
        registry
            .update_maintenance_state(3, MaintenanceState::PreventiveDue, 1005)
            .unwrap();

        // Test queries
        assert_eq!(registry.running_equipment().count(), 1);
        assert_eq!(registry.equipment_requiring_attention().count(), 1);
        assert_eq!(registry.equipment_by_type(0x2001).count(), 2); // Two motors
        assert_eq!(registry.equipment_by_controller(1).count(), 3); // All managed by controller 1
        assert_eq!(registry.equipment_requiring_maintenance().count(), 0); // PreventiveDue doesn't require maintenance yet
    }

    #[test]
    fn test_maintenance_scheduling() {
        let mut registry = EquipmentRegistry::<DefaultConfig>::new(1);

        // Register equipment
        registry.register_equipment(42, 0x2001, 1000).unwrap();

        // Schedule maintenance
        registry.schedule_maintenance(42, 2000, 1000).unwrap();

        let equipment = registry.get_equipment(42).unwrap();
        assert_eq!(equipment.next_maintenance_due.as_u64(), 2000);

        // Test overdue maintenance
        assert!(!equipment.is_maintenance_overdue(1500)); // Not overdue yet
        assert!(equipment.is_maintenance_overdue(2500)); // Overdue

        // Test overdue equipment query
        assert_eq!(registry.equipment_with_overdue_maintenance(2500).count(), 1);
    }

    #[test]
    fn test_emergency_stop() {
        let mut registry = EquipmentRegistry::<DefaultConfig>::new(1);

        // Register and start multiple equipment items
        registry.register_equipment(1, 0x2001, 1000).unwrap();
        registry.register_equipment(2, 0x2002, 1001).unwrap();

        registry
            .update_equipment_status(1, EquipmentStatus::Running, 1002)
            .unwrap();
        registry
            .update_equipment_status(2, EquipmentStatus::Idle, 1003)
            .unwrap();

        assert_eq!(registry.running_equipment().count(), 1);

        // Emergency stop all
        let stopped = registry.emergency_stop_all(1004);
        assert_eq!(stopped, 2); // Both operational equipment stopped
        assert_eq!(
            registry
                .equipment_by_status(EquipmentStatus::Emergency)
                .count(),
            2
        );
    }

    #[test]
    fn test_equipment_registry_merge() {
        let mut registry1 = EquipmentRegistry::<DefaultConfig>::new(1);
        let mut registry2 = EquipmentRegistry::<DefaultConfig>::new(2);

        // Add different equipment to each registry
        registry1.register_equipment(1, 0x2001, 1000).unwrap();
        registry2.register_equipment(2, 0x2002, 1001).unwrap();

        // Merge
        registry1.merge(&registry2).unwrap();

        // Should have both equipment items
        assert_eq!(registry1.equipment_count(), 2);
        assert!(registry1.get_equipment(1).is_some());
        assert!(registry1.get_equipment(2).is_some());
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut registry = EquipmentRegistry::<DefaultConfig>::new(1);

        assert_eq!(registry.element_count(), 0);
        assert!(registry.can_add_element());

        registry.register_equipment(42, 0x2001, 1000).unwrap();
        assert_eq!(registry.element_count(), 1);
        assert!(registry.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut registry1 = EquipmentRegistry::<DefaultConfig>::new(1);
        let registry2 = EquipmentRegistry::<DefaultConfig>::new(2);

        assert!(registry1.merge_bounded(&registry2).is_ok());
        assert!(registry1.validate_bounded().is_ok());
    }
}
