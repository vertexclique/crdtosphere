//! Device Registry for IoT Systems
//!
//! This module implements CRDTs for distributed IoT device management,
//! enabling coordination of device states across IoT networks.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

/// IoT device connection states
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ConnectionState {
    /// Device is offline/disconnected
    Offline = 0,
    /// Device is connecting
    Connecting = 1,
    /// Device is online and responsive
    Online = 2,
    /// Device is in sleep/low-power mode
    Sleeping = 3,
    /// Device has connectivity issues
    Unstable = 4,
    /// Device is in maintenance mode
    Maintenance = 5,
}

impl ConnectionState {
    /// Returns true if device can receive commands
    pub fn can_receive_commands(&self) -> bool {
        matches!(self, ConnectionState::Online | ConnectionState::Unstable)
    }

    /// Returns true if device is considered healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, ConnectionState::Online | ConnectionState::Sleeping)
    }
}

/// IoT device operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum DeviceStatus {
    /// Device is functioning normally
    Normal = 0,
    /// Device has warnings but is operational
    Warning = 1,
    /// Device has errors but is partially functional
    Error = 2,
    /// Device is in critical state
    Critical = 3,
    /// Device has failed
    Failed = 4,
    /// Device is being updated/configured
    Updating = 5,
}

impl DeviceStatus {
    /// Returns true if device requires immediate attention
    pub fn requires_attention(&self) -> bool {
        matches!(self, DeviceStatus::Critical | DeviceStatus::Failed)
    }

    /// Returns true if device is operational
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            DeviceStatus::Normal | DeviceStatus::Warning | DeviceStatus::Updating
        )
    }
}

/// Individual IoT device information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Device unique identifier
    pub device_id: NodeId,
    /// Device type/category (encoded as u16)
    pub device_type: u16,
    /// Current connection state
    pub connection_state: ConnectionState,
    /// Current operational status
    pub device_status: DeviceStatus,
    /// Battery level (0-255, 255 = AC powered)
    pub battery_level: u8,
    /// Signal strength (0-255)
    pub signal_strength: u8,
    /// Last seen timestamp
    pub last_seen: CompactTimestamp,
    /// Last update timestamp
    pub last_update: CompactTimestamp,
    /// Gateway/hub that manages this device
    pub gateway_id: NodeId,
}

impl DeviceInfo {
    /// Creates new device info
    pub fn new(device_id: NodeId, device_type: u16, gateway_id: NodeId, timestamp: u64) -> Self {
        Self {
            device_id,
            device_type,
            connection_state: ConnectionState::Offline,
            device_status: DeviceStatus::Normal,
            battery_level: 255, // Assume AC powered initially
            signal_strength: 0,
            last_seen: CompactTimestamp::new(timestamp),
            last_update: CompactTimestamp::new(timestamp),
            gateway_id,
        }
    }

    /// Updates device connection state
    pub fn update_connection(&mut self, state: ConnectionState, timestamp: u64) {
        self.connection_state = state;
        self.last_update = CompactTimestamp::new(timestamp);
        if state != ConnectionState::Offline {
            self.last_seen = CompactTimestamp::new(timestamp);
        }
    }

    /// Updates device status
    pub fn update_status(&mut self, status: DeviceStatus, timestamp: u64) {
        self.device_status = status;
        self.last_update = CompactTimestamp::new(timestamp);
    }

    /// Updates battery and signal info
    pub fn update_vitals(&mut self, battery: u8, signal: u8, timestamp: u64) {
        self.battery_level = battery;
        self.signal_strength = signal;
        self.last_update = CompactTimestamp::new(timestamp);
        self.last_seen = CompactTimestamp::new(timestamp);
    }

    /// Returns true if this device info should override another
    pub fn should_override(&self, other: &DeviceInfo) -> bool {
        // More recent updates win
        self.last_update > other.last_update
    }

    /// Returns true if device is considered stale
    pub fn is_stale(&self, current_time: u64, timeout_ms: u64) -> bool {
        current_time > self.last_seen.as_u64() + timeout_ms
    }
}

/// IoT Device Registry CRDT
///
/// This CRDT manages distributed device state coordination across IoT networks,
/// enabling gateways and controllers to maintain consistent device inventories.
///
/// # Type Parameters
/// - `C`: Memory configuration
///
/// # Features
/// - Device discovery and registration
/// - Connection state tracking
/// - Battery and signal monitoring
/// - Gateway assignment coordination
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::iot::{DeviceRegistry, DeviceInfo, ConnectionState};
///
/// // Create device registry
/// let mut registry = DeviceRegistry::<DefaultConfig>::new(1); // Gateway ID 1
///
/// // Register new device
/// registry.register_device(
///     42,    // device ID
///     0x1001, // device type (sensor)
///     1000   // timestamp
/// )?;
///
/// // Update device connection
/// registry.update_device_connection(42, ConnectionState::Online, 1001)?;
///
/// // Query online devices
/// let online_count = registry.online_devices().count();
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct DeviceRegistry<C: MemoryConfig> {
    /// Array of registered devices
    devices: [Option<DeviceInfo>; 64], // Support up to 64 devices
    /// Number of devices currently registered
    device_count: usize,
    /// This gateway's ID
    local_gateway_id: NodeId,
    /// Last update timestamp
    last_update: CompactTimestamp,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<C: MemoryConfig> DeviceRegistry<C> {
    /// Creates a new device registry
    ///
    /// # Arguments
    /// * `gateway_id` - The ID of this gateway/controller
    ///
    /// # Returns
    /// A new device registry CRDT
    pub fn new(gateway_id: NodeId) -> Self {
        Self {
            devices: [const { None }; 64],
            device_count: 0,
            local_gateway_id: gateway_id,
            last_update: CompactTimestamp::new(0),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Registers a new device
    ///
    /// # Arguments
    /// * `device_id` - Unique device identifier
    /// * `device_type` - Device type/category
    /// * `timestamp` - Registration timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn register_device(
        &mut self,
        device_id: NodeId,
        device_type: u16,
        timestamp: u64,
    ) -> CRDTResult<()> {
        let device_info = DeviceInfo::new(device_id, device_type, self.local_gateway_id, timestamp);
        self.add_device_info(device_info)?;
        self.last_update = CompactTimestamp::new(timestamp);
        Ok(())
    }

    /// Updates device connection state
    ///
    /// # Arguments
    /// * `device_id` - Device to update
    /// * `state` - New connection state
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_device_connection(
        &mut self,
        device_id: NodeId,
        state: ConnectionState,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(device) = self.find_device_mut(device_id) {
            device.update_connection(state, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Updates device operational status
    ///
    /// # Arguments
    /// * `device_id` - Device to update
    /// * `status` - New operational status
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_device_status(
        &mut self,
        device_id: NodeId,
        status: DeviceStatus,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(device) = self.find_device_mut(device_id) {
            device.update_status(status, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Updates device battery and signal info
    ///
    /// # Arguments
    /// * `device_id` - Device to update
    /// * `battery_level` - Battery level (0-255)
    /// * `signal_strength` - Signal strength (0-255)
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_device_vitals(
        &mut self,
        device_id: NodeId,
        battery_level: u8,
        signal_strength: u8,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(device) = self.find_device_mut(device_id) {
            device.update_vitals(battery_level, signal_strength, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Gets all registered devices
    ///
    /// # Returns
    /// Iterator over device info
    pub fn all_devices(&self) -> impl Iterator<Item = &DeviceInfo> {
        self.devices.iter().filter_map(|d| d.as_ref())
    }

    /// Gets devices by connection state
    ///
    /// # Arguments
    /// * `state` - Connection state to filter by
    ///
    /// # Returns
    /// Iterator over devices with the specified state
    pub fn devices_by_state(&self, state: ConnectionState) -> impl Iterator<Item = &DeviceInfo> {
        self.all_devices()
            .filter(move |d| d.connection_state == state)
    }

    /// Gets online devices
    ///
    /// # Returns
    /// Iterator over online devices
    pub fn online_devices(&self) -> impl Iterator<Item = &DeviceInfo> {
        self.devices_by_state(ConnectionState::Online)
    }

    /// Gets devices requiring attention
    ///
    /// # Returns
    /// Iterator over devices with critical status
    pub fn devices_requiring_attention(&self) -> impl Iterator<Item = &DeviceInfo> {
        self.all_devices()
            .filter(|d| d.device_status.requires_attention())
    }

    /// Gets devices by type
    ///
    /// # Arguments
    /// * `device_type` - Device type to filter by
    ///
    /// # Returns
    /// Iterator over devices of the specified type
    pub fn devices_by_type(&self, device_type: u16) -> impl Iterator<Item = &DeviceInfo> {
        self.all_devices()
            .filter(move |d| d.device_type == device_type)
    }

    /// Gets devices managed by a specific gateway
    ///
    /// # Arguments
    /// * `gateway_id` - Gateway ID to filter by
    ///
    /// # Returns
    /// Iterator over devices managed by the gateway
    pub fn devices_by_gateway(&self, gateway_id: NodeId) -> impl Iterator<Item = &DeviceInfo> {
        self.all_devices()
            .filter(move |d| d.gateway_id == gateway_id)
    }

    /// Gets device info by ID
    ///
    /// # Arguments
    /// * `device_id` - Device ID to look up
    ///
    /// # Returns
    /// Device info if found
    pub fn get_device(&self, device_id: NodeId) -> Option<&DeviceInfo> {
        self.all_devices().find(|d| d.device_id == device_id)
    }

    /// Gets the number of registered devices
    ///
    /// # Returns
    /// Number of devices
    pub fn device_count(&self) -> usize {
        self.device_count
    }

    /// Removes stale devices
    ///
    /// # Arguments
    /// * `current_time` - Current timestamp
    /// * `timeout_ms` - Timeout in milliseconds
    ///
    /// # Returns
    /// Number of devices removed
    pub fn cleanup_stale_devices(&mut self, current_time: u64, timeout_ms: u64) -> usize {
        let mut removed = 0;

        for i in 0..64 {
            if let Some(device) = &self.devices[i] {
                if device.is_stale(current_time, timeout_ms) {
                    self.devices[i] = None;
                    self.device_count -= 1;
                    removed += 1;
                }
            }
        }

        // Compact the array
        self.compact_devices();

        removed
    }

    /// Finds a device by ID (mutable)
    fn find_device_mut(&mut self, device_id: NodeId) -> Option<&mut DeviceInfo> {
        for device_opt in &mut self.devices {
            if let Some(device) = device_opt {
                if device.device_id == device_id {
                    return Some(device);
                }
            }
        }
        None
    }

    /// Adds device info to the registry
    fn add_device_info(&mut self, device_info: DeviceInfo) -> CRDTResult<()> {
        // Check for existing device
        for i in 0..64 {
            if let Some(ref mut existing) = self.devices[i] {
                if existing.device_id == device_info.device_id {
                    // Update if new info should override
                    if device_info.should_override(existing) {
                        *existing = device_info;
                    }
                    return Ok(());
                }
            } else {
                // Empty slot - add new device
                self.devices[i] = Some(device_info);
                self.device_count += 1;
                return Ok(());
            }
        }

        // If no space, try to replace oldest offline device
        self.make_space_for_device(device_info)
    }

    /// Makes space for a new device by replacing old offline devices
    fn make_space_for_device(&mut self, new_device: DeviceInfo) -> CRDTResult<()> {
        // Find oldest offline device to replace
        let mut oldest_idx = None;
        let mut oldest_time = u64::MAX;

        for (i, device_opt) in self.devices.iter().enumerate() {
            if let Some(device) = device_opt {
                if device.connection_state == ConnectionState::Offline
                    && device.last_seen.as_u64() < oldest_time
                {
                    oldest_time = device.last_seen.as_u64();
                    oldest_idx = Some(i);
                }
            }
        }

        if let Some(idx) = oldest_idx {
            self.devices[idx] = Some(new_device);
            Ok(())
        } else {
            Err(CRDTError::BufferOverflow)
        }
    }

    /// Compacts the devices array
    fn compact_devices(&mut self) {
        let mut write_idx = 0;

        for read_idx in 0..64 {
            if let Some(device) = self.devices[read_idx] {
                if write_idx != read_idx {
                    self.devices[write_idx] = Some(device);
                    self.devices[read_idx] = None;
                }
                write_idx += 1;
            }
        }
    }

    /// Validates device registry data
    ///
    /// # Returns
    /// Ok(()) if valid, error otherwise
    pub fn validate_registry(&self) -> CRDTResult<()> {
        // Check device IDs are valid
        for device in self.all_devices() {
            if device.device_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
            if device.gateway_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        Ok(())
    }
}

impl<C: MemoryConfig> CRDT<C> for DeviceRegistry<C> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Merge all devices from other
        for device in other.all_devices() {
            self.add_device_info(*device)?;
        }

        // Update timestamp to latest
        if other.last_update > self.last_update {
            self.last_update = other.last_update;
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        if self.device_count != other.device_count {
            return false;
        }

        // Check that all devices match
        for device in self.all_devices() {
            let mut found = false;
            for other_device in other.all_devices() {
                if device.device_id == other_device.device_id && device == other_device {
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
        let mut hash = self.local_gateway_id as u32;
        for device in self.all_devices() {
            hash ^= (device.device_id as u32) ^ (device.last_update.as_u64() as u32);
        }
        hash ^= self.device_count as u32;
        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // Can always merge device registries (space is made by removing old devices)
        true
    }
}

impl<C: MemoryConfig> BoundedCRDT<C> for DeviceRegistry<C> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 64; // Maximum devices

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.device_count
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        self.compact_devices();
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        self.device_count < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig> RealTimeCRDT<C> for DeviceRegistry<C> {
    const MAX_MERGE_CYCLES: u32 = 200; // Bounded by number of devices
    const MAX_VALIDATE_CYCLES: u32 = 100;
    const MAX_SERIALIZE_CYCLES: u32 = 150;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Device registry merge is bounded
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded
        self.validate()
    }

    fn remaining_budget(&self) -> Option<u32> {
        // For IoT systems, we don't track budget
        None
    }

    fn set_budget(&mut self, _cycles: u32) {
        // For IoT systems, we don't limit budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::DefaultConfig;

    #[test]
    fn test_connection_state_properties() {
        assert!(ConnectionState::Online.can_receive_commands());
        assert!(ConnectionState::Unstable.can_receive_commands());
        assert!(!ConnectionState::Offline.can_receive_commands());

        assert!(ConnectionState::Online.is_healthy());
        assert!(ConnectionState::Sleeping.is_healthy());
        assert!(!ConnectionState::Offline.is_healthy());
    }

    #[test]
    fn test_device_status_properties() {
        assert!(DeviceStatus::Critical.requires_attention());
        assert!(DeviceStatus::Failed.requires_attention());
        assert!(!DeviceStatus::Normal.requires_attention());

        assert!(DeviceStatus::Normal.is_operational());
        assert!(DeviceStatus::Warning.is_operational());
        assert!(!DeviceStatus::Failed.is_operational());
    }

    #[test]
    fn test_device_info_creation() {
        let device = DeviceInfo::new(42, 0x1001, 1, 1000);

        assert_eq!(device.device_id, 42);
        assert_eq!(device.device_type, 0x1001);
        assert_eq!(device.gateway_id, 1);
        assert_eq!(device.connection_state, ConnectionState::Offline);
        assert_eq!(device.device_status, DeviceStatus::Normal);
    }

    #[test]
    fn test_device_info_updates() {
        let mut device = DeviceInfo::new(42, 0x1001, 1, 1000);

        device.update_connection(ConnectionState::Online, 1001);
        assert_eq!(device.connection_state, ConnectionState::Online);
        assert_eq!(device.last_seen.as_u64(), 1001);

        device.update_status(DeviceStatus::Warning, 1002);
        assert_eq!(device.device_status, DeviceStatus::Warning);

        device.update_vitals(80, 200, 1003);
        assert_eq!(device.battery_level, 80);
        assert_eq!(device.signal_strength, 200);
    }

    #[test]
    fn test_device_info_override() {
        let device1 = DeviceInfo::new(42, 0x1001, 1, 1000);
        let mut device2 = DeviceInfo::new(42, 0x1001, 1, 1001);
        device2.update_connection(ConnectionState::Online, 1002);

        assert!(device2.should_override(&device1)); // Newer timestamp
        assert!(!device1.should_override(&device2)); // Older timestamp
    }

    #[test]
    fn test_device_registry_creation() {
        let registry = DeviceRegistry::<DefaultConfig>::new(1);

        assert_eq!(registry.device_count(), 0);
        assert_eq!(registry.local_gateway_id, 1);
    }

    #[test]
    fn test_device_registration_and_updates() {
        let mut registry = DeviceRegistry::<DefaultConfig>::new(1);

        // Register device
        registry.register_device(42, 0x1001, 1000).unwrap();
        assert_eq!(registry.device_count(), 1);

        // Update connection
        registry
            .update_device_connection(42, ConnectionState::Online, 1001)
            .unwrap();

        // Update status
        registry
            .update_device_status(42, DeviceStatus::Warning, 1002)
            .unwrap();

        // Update vitals
        registry.update_device_vitals(42, 75, 180, 1003).unwrap();

        let device = registry.get_device(42).unwrap();
        assert_eq!(device.connection_state, ConnectionState::Online);
        assert_eq!(device.device_status, DeviceStatus::Warning);
        assert_eq!(device.battery_level, 75);
        assert_eq!(device.signal_strength, 180);
    }

    #[test]
    fn test_device_queries() {
        let mut registry = DeviceRegistry::<DefaultConfig>::new(1);

        // Register multiple devices
        registry.register_device(1, 0x1001, 1000).unwrap(); // Sensor
        registry.register_device(2, 0x2001, 1001).unwrap(); // Actuator
        registry.register_device(3, 0x1001, 1002).unwrap(); // Another sensor

        // Update states
        registry
            .update_device_connection(1, ConnectionState::Online, 1003)
            .unwrap();
        registry
            .update_device_connection(2, ConnectionState::Offline, 1004)
            .unwrap();
        registry
            .update_device_status(3, DeviceStatus::Critical, 1005)
            .unwrap();

        // Test queries
        assert_eq!(registry.online_devices().count(), 1);
        assert_eq!(registry.devices_by_type(0x1001).count(), 2); // Two sensors
        assert_eq!(registry.devices_requiring_attention().count(), 1); // Critical device
        assert_eq!(registry.devices_by_gateway(1).count(), 3); // All managed by gateway 1
    }

    #[test]
    fn test_device_registry_merge() {
        let mut registry1 = DeviceRegistry::<DefaultConfig>::new(1);
        let mut registry2 = DeviceRegistry::<DefaultConfig>::new(2);

        // Add different devices to each registry
        registry1.register_device(1, 0x1001, 1000).unwrap();
        registry2.register_device(2, 0x2001, 1001).unwrap();

        // Merge
        registry1.merge(&registry2).unwrap();

        // Should have both devices
        assert_eq!(registry1.device_count(), 2);
        assert!(registry1.get_device(1).is_some());
        assert!(registry1.get_device(2).is_some());
    }

    #[test]
    fn test_stale_device_cleanup() {
        let mut registry = DeviceRegistry::<DefaultConfig>::new(1);

        // Register device and mark as seen
        registry.register_device(42, 0x1001, 1000).unwrap();
        registry
            .update_device_connection(42, ConnectionState::Online, 1000)
            .unwrap();

        assert_eq!(registry.device_count(), 1);

        // Clean up after timeout
        let removed = registry.cleanup_stale_devices(10000, 5000); // 5 second timeout
        assert_eq!(removed, 1);
        assert_eq!(registry.device_count(), 0);
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut registry = DeviceRegistry::<DefaultConfig>::new(1);

        assert_eq!(registry.element_count(), 0);
        assert!(registry.can_add_element());

        registry.register_device(42, 0x1001, 1000).unwrap();
        assert_eq!(registry.element_count(), 1);
        assert!(registry.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut registry1 = DeviceRegistry::<DefaultConfig>::new(1);
        let registry2 = DeviceRegistry::<DefaultConfig>::new(2);

        assert!(registry1.merge_bounded(&registry2).is_ok());
        assert!(registry1.validate_bounded().is_ok());
    }
}
