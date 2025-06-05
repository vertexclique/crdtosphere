//! Robot Status Coordination for Multi-Robot Systems
//!
//! This module implements CRDTs for sharing robot operational status,
//! position, and health information across distributed robot networks.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

/// Robot operational modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum OperationalMode {
    /// Robot is offline or shutdown
    Offline = 0,
    /// Robot is idle and ready
    Idle = 1,
    /// Robot is actively working
    Active = 2,
    /// Robot is in maintenance mode
    Maintenance = 3,
    /// Robot has encountered an error
    Error = 4,
    /// Robot is in emergency stop
    Emergency = 5,
}

impl OperationalMode {
    /// Returns true if the robot is available for coordination
    pub fn is_available(&self) -> bool {
        matches!(self, OperationalMode::Idle | OperationalMode::Active)
    }

    /// Returns true if this is a critical state
    pub fn is_critical(&self) -> bool {
        matches!(self, OperationalMode::Error | OperationalMode::Emergency)
    }
}

/// Battery level representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum BatteryLevel {
    /// Critical battery level (< 10%)
    Critical = 0,
    /// Low battery level (10-25%)
    Low = 1,
    /// Medium battery level (25-75%)
    Medium = 2,
    /// High battery level (> 75%)
    High = 3,
}

impl BatteryLevel {
    /// Creates a battery level from percentage
    pub fn from_percentage(percent: u8) -> Self {
        match percent {
            0..=9 => BatteryLevel::Critical,
            10..=25 => BatteryLevel::Low,
            26..=75 => BatteryLevel::Medium,
            _ => BatteryLevel::High,
        }
    }

    /// Returns true if battery level is sufficient for operation
    pub fn is_sufficient(&self) -> bool {
        *self >= BatteryLevel::Low
    }
}

/// 3D position representation (fixed-point for deterministic behavior)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position3D {
    /// X coordinate (millimeters)
    pub x: i32,
    /// Y coordinate (millimeters)
    pub y: i32,
    /// Z coordinate (millimeters)
    pub z: i32,
}

impl Position3D {
    /// Creates a new 3D position
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Creates a 2D position (z = 0)
    pub fn new_2d(x: i32, y: i32) -> Self {
        Self { x, y, z: 0 }
    }

    /// Calculates distance to another position (squared to avoid floating point)
    pub fn distance_squared(&self, other: &Position3D) -> u64 {
        let dx = (self.x - other.x) as i64;
        let dy = (self.y - other.y) as i64;
        let dz = (self.z - other.z) as i64;
        (dx * dx + dy * dy + dz * dz) as u64
    }
}

/// Robot status information
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusInfo {
    /// Current operational mode
    pub mode: OperationalMode,
    /// Current position
    pub position: Position3D,
    /// Battery level
    pub battery: BatteryLevel,
    /// Last update timestamp
    pub timestamp: CompactTimestamp,
    /// Robot ID
    pub robot_id: NodeId,
}

impl StatusInfo {
    /// Creates new status information
    pub fn new(
        mode: OperationalMode,
        position: Position3D,
        battery: BatteryLevel,
        timestamp: u64,
        robot_id: NodeId,
    ) -> Self {
        Self {
            mode,
            position,
            battery,
            timestamp: CompactTimestamp::new(timestamp),
            robot_id,
        }
    }

    /// Returns true if this robot is operational
    pub fn is_operational(&self) -> bool {
        self.mode.is_available() && self.battery.is_sufficient()
    }

    /// Returns true if this robot needs attention
    pub fn needs_attention(&self) -> bool {
        self.mode.is_critical() || self.battery == BatteryLevel::Critical
    }
}

/// Multi-robot status coordination CRDT
///
/// This CRDT manages distributed robot status sharing, allowing robots
/// to coordinate based on each other's operational state, position, and health.
///
/// # Type Parameters
/// - `C`: Memory configuration
///
/// # Features
/// - Real-time status sharing
/// - Position-based coordination
/// - Battery level monitoring
/// - Operational mode tracking
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::robotics::{RobotStatus, OperationalMode, BatteryLevel, Position3D};
///
/// // Create robot status coordinator
/// let mut status = RobotStatus::<DefaultConfig>::new(1);
///
/// // Update this robot's status
/// status.update_status(
///     OperationalMode::Active,
///     Position3D::new(1000, 2000, 0), // 1m, 2m position
///     BatteryLevel::High,
///     1000 // timestamp
/// )?;
///
/// // Check if any robots need help
/// let critical_robots = status.robots_needing_attention();
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct RobotStatus<C: MemoryConfig> {
    /// Array of robot status information
    robots: [Option<StatusInfo>; 16], // Support up to 16 robots
    /// Number of robots currently tracked
    robot_count: usize,
    /// This robot's ID
    local_robot_id: NodeId,
    /// Last update timestamp
    last_update: CompactTimestamp,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<C: MemoryConfig> RobotStatus<C> {
    /// Creates a new robot status coordinator
    ///
    /// # Arguments
    /// * `robot_id` - The ID of this robot
    ///
    /// # Returns
    /// A new robot status CRDT
    pub fn new(robot_id: NodeId) -> Self {
        Self {
            robots: [const { None }; 16],
            robot_count: 0,
            local_robot_id: robot_id,
            last_update: CompactTimestamp::new(0),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Updates this robot's status
    ///
    /// # Arguments
    /// * `mode` - Current operational mode
    /// * `position` - Current position
    /// * `battery` - Current battery level
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_status(
        &mut self,
        mode: OperationalMode,
        position: Position3D,
        battery: BatteryLevel,
        timestamp: u64,
    ) -> CRDTResult<()> {
        let status = StatusInfo::new(mode, position, battery, timestamp, self.local_robot_id);
        self.add_or_update_robot(status)?;
        self.last_update = CompactTimestamp::new(timestamp);
        Ok(())
    }

    /// Gets status of a specific robot
    ///
    /// # Arguments
    /// * `robot_id` - Robot to query
    ///
    /// # Returns
    /// Robot status if found, None otherwise
    pub fn get_robot_status(&self, robot_id: NodeId) -> Option<&StatusInfo> {
        self.robots
            .iter()
            .filter_map(|r| r.as_ref())
            .find(|r| r.robot_id == robot_id)
    }

    /// Gets all robot statuses
    ///
    /// # Returns
    /// Iterator over all robot statuses
    pub fn all_robots(&self) -> impl Iterator<Item = &StatusInfo> {
        self.robots.iter().filter_map(|r| r.as_ref())
    }

    /// Gets operational robots
    ///
    /// # Returns
    /// Iterator over operational robots
    pub fn operational_robots(&self) -> impl Iterator<Item = &StatusInfo> {
        self.all_robots().filter(|r| r.is_operational())
    }

    /// Gets robots needing attention
    ///
    /// # Returns
    /// Iterator over robots needing attention
    pub fn robots_needing_attention(&self) -> impl Iterator<Item = &StatusInfo> {
        self.all_robots().filter(|r| r.needs_attention())
    }

    /// Finds nearest robot to a position
    ///
    /// # Arguments
    /// * `position` - Target position
    ///
    /// # Returns
    /// Nearest robot status if any robots exist
    pub fn nearest_robot(&self, position: &Position3D) -> Option<&StatusInfo> {
        self.operational_robots()
            .min_by_key(|r| r.position.distance_squared(position))
    }

    /// Gets robots within a certain distance
    ///
    /// # Arguments
    /// * `center` - Center position
    /// * `max_distance_squared` - Maximum distance squared (to avoid floating point)
    ///
    /// # Returns
    /// Iterator over nearby robots
    pub fn robots_within_distance(
        &self,
        center: &Position3D,
        max_distance_squared: u64,
    ) -> impl Iterator<Item = &StatusInfo> {
        self.operational_robots()
            .filter(move |r| r.position.distance_squared(center) <= max_distance_squared)
    }

    /// Gets the number of tracked robots
    ///
    /// # Returns
    /// Number of robots
    pub fn robot_count(&self) -> usize {
        self.robot_count
    }

    /// Gets the number of operational robots
    ///
    /// # Returns
    /// Number of operational robots
    pub fn operational_count(&self) -> usize {
        self.operational_robots().count()
    }

    /// Adds or updates a robot's status
    fn add_or_update_robot(&mut self, status: StatusInfo) -> CRDTResult<()> {
        // Find existing robot or empty slot
        for i in 0..16 {
            if let Some(ref mut existing) = self.robots[i] {
                if existing.robot_id == status.robot_id {
                    // Update if newer timestamp
                    if status.timestamp > existing.timestamp {
                        *existing = status;
                    }
                    return Ok(());
                }
            } else {
                // Empty slot - add new robot
                self.robots[i] = Some(status);
                self.robot_count += 1;
                return Ok(());
            }
        }

        Err(CRDTError::BufferOverflow)
    }

    /// Validates robot status data
    ///
    /// # Returns
    /// Ok(()) if valid, error otherwise
    pub fn validate_status(&self) -> CRDTResult<()> {
        // Check robot IDs are valid
        for robot in self.all_robots() {
            if robot.robot_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        // Check timestamp consistency
        if let Some(latest) = self.all_robots().map(|r| r.timestamp).max() {
            for robot in self.all_robots() {
                let time_diff = latest.as_u64().saturating_sub(robot.timestamp.as_u64());
                if time_diff > 60000 {
                    // 60 second threshold
                    return Err(CRDTError::InvalidState);
                }
            }
        }

        Ok(())
    }
}

impl<C: MemoryConfig> CRDT<C> for RobotStatus<C> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Merge all robot statuses from other
        for robot in other.all_robots() {
            self.add_or_update_robot(*robot)?;
        }

        // Update timestamp to latest
        if other.last_update > self.last_update {
            self.last_update = other.last_update;
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        if self.robot_count != other.robot_count {
            return false;
        }

        // Check that all robots match
        for robot in self.all_robots() {
            if let Some(other_robot) = other.get_robot_status(robot.robot_id) {
                if robot != other_robot {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    fn size_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn validate(&self) -> CRDTResult<()> {
        self.validate_status()
    }

    fn state_hash(&self) -> u32 {
        let mut hash = self.local_robot_id as u32;
        for robot in self.all_robots() {
            hash ^= (robot.robot_id as u32) ^ (robot.timestamp.as_u64() as u32);
        }
        hash ^= self.robot_count as u32;
        hash
    }

    fn can_merge(&self, other: &Self) -> bool {
        // Check if merging would exceed capacity
        let mut new_robots = 0;
        for other_robot in other.all_robots() {
            if self.get_robot_status(other_robot.robot_id).is_none() {
                new_robots += 1;
            }
        }

        self.robot_count + new_robots <= 16
    }
}

impl<C: MemoryConfig> BoundedCRDT<C> for RobotStatus<C> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 16; // Maximum robots

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.robot_count
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // Could implement cleanup of old/offline robots
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        self.robot_count < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig> RealTimeCRDT<C> for RobotStatus<C> {
    const MAX_MERGE_CYCLES: u32 = 100; // Fast merge for real-time coordination
    const MAX_VALIDATE_CYCLES: u32 = 50;
    const MAX_SERIALIZE_CYCLES: u32 = 75;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Robot status merge is bounded
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded
        self.validate()
    }

    fn remaining_budget(&self) -> Option<u32> {
        // For robotics systems, we don't track budget
        None
    }

    fn set_budget(&mut self, _cycles: u32) {
        // For robotics systems, we don't limit budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::DefaultConfig;

    #[test]
    fn test_operational_mode_properties() {
        assert!(OperationalMode::Active.is_available());
        assert!(OperationalMode::Idle.is_available());
        assert!(!OperationalMode::Offline.is_available());

        assert!(OperationalMode::Emergency.is_critical());
        assert!(OperationalMode::Error.is_critical());
        assert!(!OperationalMode::Active.is_critical());
    }

    #[test]
    fn test_battery_level() {
        assert_eq!(BatteryLevel::from_percentage(5), BatteryLevel::Critical);
        assert_eq!(BatteryLevel::from_percentage(20), BatteryLevel::Low);
        assert_eq!(BatteryLevel::from_percentage(50), BatteryLevel::Medium);
        assert_eq!(BatteryLevel::from_percentage(90), BatteryLevel::High);

        assert!(!BatteryLevel::Critical.is_sufficient());
        assert!(BatteryLevel::Low.is_sufficient());
    }

    #[test]
    fn test_position_3d() {
        let pos1 = Position3D::new(0, 0, 0);
        let pos2 = Position3D::new(3, 4, 0); // 3-4-5 triangle

        assert_eq!(pos1.distance_squared(&pos2), 25); // 3^2 + 4^2 = 25

        let pos2d = Position3D::new_2d(10, 20);
        assert_eq!(pos2d.z, 0);
    }

    #[test]
    fn test_status_info() {
        let status = StatusInfo::new(
            OperationalMode::Active,
            Position3D::new(100, 200, 0),
            BatteryLevel::High,
            1000,
            1,
        );

        assert!(status.is_operational());
        assert!(!status.needs_attention());
        assert_eq!(status.robot_id, 1);
    }

    #[test]
    fn test_robot_status_creation() {
        let status = RobotStatus::<DefaultConfig>::new(1);

        assert_eq!(status.robot_count(), 0);
        assert_eq!(status.operational_count(), 0);
        assert!(status.get_robot_status(1).is_none());
    }

    #[test]
    fn test_status_update_and_query() {
        let mut status = RobotStatus::<DefaultConfig>::new(1);

        // Update this robot's status
        status
            .update_status(
                OperationalMode::Active,
                Position3D::new(1000, 2000, 0),
                BatteryLevel::High,
                1000,
            )
            .unwrap();

        assert_eq!(status.robot_count(), 1);
        assert_eq!(status.operational_count(), 1);

        let robot_status = status.get_robot_status(1).unwrap();
        assert_eq!(robot_status.mode, OperationalMode::Active);
        assert_eq!(robot_status.position.x, 1000);
        assert!(robot_status.is_operational());
    }

    #[test]
    fn test_robot_coordination_queries() {
        let mut status = RobotStatus::<DefaultConfig>::new(1);

        // Add multiple robots
        status
            .add_or_update_robot(StatusInfo::new(
                OperationalMode::Active,
                Position3D::new(0, 0, 0),
                BatteryLevel::High,
                1000,
                1,
            ))
            .unwrap();

        status
            .add_or_update_robot(StatusInfo::new(
                OperationalMode::Error,
                Position3D::new(1000, 0, 0),
                BatteryLevel::Critical,
                1000,
                2,
            ))
            .unwrap();

        status
            .add_or_update_robot(StatusInfo::new(
                OperationalMode::Active,
                Position3D::new(500, 500, 0),
                BatteryLevel::Medium,
                1000,
                3,
            ))
            .unwrap();

        assert_eq!(status.robot_count(), 3);
        assert_eq!(status.operational_count(), 2); // Robots 1 and 3
        assert_eq!(status.robots_needing_attention().count(), 1); // Robot 2

        // Test nearest robot
        let target = Position3D::new(400, 400, 0);
        let nearest = status.nearest_robot(&target).unwrap();
        assert_eq!(nearest.robot_id, 3); // Robot 3 is closest

        // Test robots within distance
        let nearby = status.robots_within_distance(&target, 500_000).count(); // Large distance
        assert_eq!(nearby, 2); // Robots 1 and 3 are operational
    }

    #[test]
    fn test_robot_status_merge() {
        let mut status1 = RobotStatus::<DefaultConfig>::new(1);
        let mut status2 = RobotStatus::<DefaultConfig>::new(2);

        // Add different robots to each
        status1
            .update_status(
                OperationalMode::Active,
                Position3D::new(0, 0, 0),
                BatteryLevel::High,
                1000,
            )
            .unwrap();

        status2
            .update_status(
                OperationalMode::Idle,
                Position3D::new(1000, 1000, 0),
                BatteryLevel::Medium,
                1001,
            )
            .unwrap();

        // Merge
        status1.merge(&status2).unwrap();

        // Should have both robots
        assert_eq!(status1.robot_count(), 2);
        assert!(status1.get_robot_status(1).is_some());
        assert!(status1.get_robot_status(2).is_some());
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut status = RobotStatus::<DefaultConfig>::new(1);

        assert_eq!(status.element_count(), 0);
        assert!(status.can_add_element());

        status
            .update_status(
                OperationalMode::Active,
                Position3D::new(0, 0, 0),
                BatteryLevel::High,
                1000,
            )
            .unwrap();

        assert_eq!(status.element_count(), 1);
        assert!(status.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut status1 = RobotStatus::<DefaultConfig>::new(1);
        let status2 = RobotStatus::<DefaultConfig>::new(2);

        assert!(status1.merge_bounded(&status2).is_ok());
        assert!(status1.validate_bounded().is_ok());
    }
}
