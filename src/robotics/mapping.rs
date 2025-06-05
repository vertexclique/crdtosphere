//! Shared Mapping for Multi-Robot Systems
//!
//! This module implements CRDTs for collaborative mapping and spatial data
//! sharing between robots, enabling distributed SLAM and environment mapping.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

/// Map point types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum MapPointType {
    /// Free space (navigable)
    Free = 0,
    /// Obstacle (not navigable)
    Obstacle = 1,
    /// Unknown space (not yet explored)
    Unknown = 2,
    /// Landmark (recognizable feature)
    Landmark = 3,
    /// Goal location
    Goal = 4,
    /// Charging station
    ChargingStation = 5,
}

impl MapPointType {
    /// Returns true if this point type is navigable
    pub fn is_navigable(&self) -> bool {
        matches!(
            self,
            MapPointType::Free | MapPointType::Goal | MapPointType::ChargingStation
        )
    }

    /// Returns true if this is a special point of interest
    pub fn is_poi(&self) -> bool {
        matches!(
            self,
            MapPointType::Landmark | MapPointType::Goal | MapPointType::ChargingStation
        )
    }
}

/// Individual map point with metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapPoint {
    /// X coordinate (millimeters)
    pub x: i32,
    /// Y coordinate (millimeters)
    pub y: i32,
    /// Point type
    pub point_type: MapPointType,
    /// Confidence level (0-255)
    pub confidence: u8,
    /// Timestamp when point was observed
    pub timestamp: CompactTimestamp,
    /// Robot that observed this point
    pub observer_id: NodeId,
}

impl MapPoint {
    /// Creates a new map point
    pub fn new(
        x: i32,
        y: i32,
        point_type: MapPointType,
        confidence: u8,
        timestamp: u64,
        observer_id: NodeId,
    ) -> Self {
        Self {
            x,
            y,
            point_type,
            confidence,
            timestamp: CompactTimestamp::new(timestamp),
            observer_id,
        }
    }

    /// Returns the grid key for this point (for spatial indexing)
    pub fn grid_key(&self, grid_size: i32) -> (i32, i32) {
        (self.x / grid_size, self.y / grid_size)
    }

    /// Calculates distance squared to another point
    pub fn distance_squared(&self, other: &MapPoint) -> u64 {
        let dx = (self.x - other.x) as i64;
        let dy = (self.y - other.y) as i64;
        (dx * dx + dy * dy) as u64
    }

    /// Returns true if this point should override another based on confidence and timestamp
    pub fn should_override(&self, other: &MapPoint) -> bool {
        // Higher confidence wins
        if self.confidence > other.confidence {
            return true;
        }

        // Same confidence - newer timestamp wins
        if self.confidence == other.confidence {
            return self.timestamp > other.timestamp;
        }

        false
    }
}

/// Map data container for efficient storage
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapData {
    /// Compressed map point data
    pub point: MapPoint,
    /// Hash for quick comparison
    pub hash: u32,
}

impl MapData {
    /// Creates new map data
    pub fn new(point: MapPoint) -> Self {
        let hash = Self::compute_hash(&point);
        Self { point, hash }
    }

    /// Computes hash for a map point
    fn compute_hash(point: &MapPoint) -> u32 {
        let mut hash = 0u32;
        hash ^= point.x as u32;
        hash ^= (point.y as u32) << 16;
        hash ^= (point.point_type as u32) << 8;
        hash ^= point.confidence as u32;
        hash
    }
}

/// Shared mapping CRDT for collaborative SLAM
///
/// This CRDT manages distributed mapping data between robots,
/// enabling collaborative SLAM and environment understanding.
///
/// # Type Parameters
/// - `C`: Memory configuration
///
/// # Features
/// - Collaborative mapping
/// - Confidence-based conflict resolution
/// - Spatial indexing for efficient queries
/// - Point of interest tracking
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::robotics::{SharedMap, MapPoint, MapPointType};
///
/// // Create shared map
/// let mut map = SharedMap::<DefaultConfig>::new(1);
///
/// // Add obstacle observation
/// map.add_observation(
///     1000, 2000, // position (1m, 2m)
///     MapPointType::Obstacle,
///     200, // high confidence
///     1000 // timestamp
/// )?;
///
/// // Query nearby points
/// let nearby = map.points_near(1000, 2000, 500_000); // 500mm radius
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct SharedMap<C: MemoryConfig> {
    /// Array of map data points
    points: [Option<MapData>; 64], // Support up to 64 map points
    /// Number of points currently stored
    point_count: usize,
    /// This robot's ID
    local_robot_id: NodeId,
    /// Last update timestamp
    last_update: CompactTimestamp,
    /// Grid size for spatial indexing (millimeters)
    grid_size: i32,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<C: MemoryConfig> SharedMap<C> {
    /// Creates a new shared map
    ///
    /// # Arguments
    /// * `robot_id` - The ID of this robot
    ///
    /// # Returns
    /// A new shared map CRDT
    pub fn new(robot_id: NodeId) -> Self {
        Self {
            points: [const { None }; 64],
            point_count: 0,
            local_robot_id: robot_id,
            last_update: CompactTimestamp::new(0),
            grid_size: 100, // 10cm grid cells
            _phantom: core::marker::PhantomData,
        }
    }

    /// Creates a new shared map with custom grid size
    ///
    /// # Arguments
    /// * `robot_id` - The ID of this robot
    /// * `grid_size` - Grid cell size in millimeters
    ///
    /// # Returns
    /// A new shared map CRDT
    pub fn with_grid_size(robot_id: NodeId, grid_size: i32) -> Self {
        Self {
            points: [const { None }; 64],
            point_count: 0,
            local_robot_id: robot_id,
            last_update: CompactTimestamp::new(0),
            grid_size,
            _phantom: core::marker::PhantomData,
        }
    }

    /// Adds a map observation
    ///
    /// # Arguments
    /// * `x` - X coordinate in millimeters
    /// * `y` - Y coordinate in millimeters
    /// * `point_type` - Type of map point
    /// * `confidence` - Confidence level (0-255)
    /// * `timestamp` - Observation timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn add_observation(
        &mut self,
        x: i32,
        y: i32,
        point_type: MapPointType,
        confidence: u8,
        timestamp: u64,
    ) -> CRDTResult<()> {
        let point = MapPoint::new(x, y, point_type, confidence, timestamp, self.local_robot_id);
        let map_data = MapData::new(point);

        self.add_map_data(map_data)?;
        self.last_update = CompactTimestamp::new(timestamp);
        Ok(())
    }

    /// Gets all map points
    ///
    /// # Returns
    /// Iterator over map points
    pub fn all_points(&self) -> impl Iterator<Item = &MapPoint> {
        self.points
            .iter()
            .filter_map(|d| d.as_ref())
            .map(|d| &d.point)
    }

    /// Gets points of a specific type
    ///
    /// # Arguments
    /// * `point_type` - Type of points to get
    ///
    /// # Returns
    /// Iterator over points of the specified type
    pub fn points_by_type(&self, point_type: MapPointType) -> impl Iterator<Item = &MapPoint> {
        self.all_points()
            .filter(move |p| p.point_type == point_type)
    }

    /// Gets navigable points
    ///
    /// # Returns
    /// Iterator over navigable points
    pub fn navigable_points(&self) -> impl Iterator<Item = &MapPoint> {
        self.all_points().filter(|p| p.point_type.is_navigable())
    }

    /// Gets points of interest
    ///
    /// # Returns
    /// Iterator over points of interest
    pub fn points_of_interest(&self) -> impl Iterator<Item = &MapPoint> {
        self.all_points().filter(|p| p.point_type.is_poi())
    }

    /// Gets points near a location
    ///
    /// # Arguments
    /// * `x` - X coordinate in millimeters
    /// * `y` - Y coordinate in millimeters
    /// * `max_distance_squared` - Maximum distance squared
    ///
    /// # Returns
    /// Iterator over nearby points
    pub fn points_near(
        &self,
        x: i32,
        y: i32,
        max_distance_squared: u64,
    ) -> impl Iterator<Item = &MapPoint> {
        let target = MapPoint::new(x, y, MapPointType::Unknown, 0, 0, 0);
        self.all_points()
            .filter(move |p| p.distance_squared(&target) <= max_distance_squared)
    }

    /// Gets obstacles near a location
    ///
    /// # Arguments
    /// * `x` - X coordinate in millimeters
    /// * `y` - Y coordinate in millimeters
    /// * `max_distance_squared` - Maximum distance squared
    ///
    /// # Returns
    /// Iterator over nearby obstacles
    pub fn obstacles_near(
        &self,
        x: i32,
        y: i32,
        max_distance_squared: u64,
    ) -> impl Iterator<Item = &MapPoint> {
        self.points_near(x, y, max_distance_squared)
            .filter(|p| p.point_type == MapPointType::Obstacle)
    }

    /// Finds the nearest point of interest
    ///
    /// # Arguments
    /// * `x` - X coordinate in millimeters
    /// * `y` - Y coordinate in millimeters
    ///
    /// # Returns
    /// Nearest point of interest if any exist
    pub fn nearest_poi(&self, x: i32, y: i32) -> Option<&MapPoint> {
        let target = MapPoint::new(x, y, MapPointType::Unknown, 0, 0, 0);
        self.points_of_interest()
            .min_by_key(|p| p.distance_squared(&target))
    }

    /// Gets the number of map points
    ///
    /// # Returns
    /// Number of points
    pub fn point_count(&self) -> usize {
        self.point_count
    }

    /// Gets the grid size
    ///
    /// # Returns
    /// Grid size in millimeters
    pub fn grid_size(&self) -> i32 {
        self.grid_size
    }

    /// Checks if a location is likely navigable
    ///
    /// # Arguments
    /// * `x` - X coordinate in millimeters
    /// * `y` - Y coordinate in millimeters
    /// * `safety_radius` - Safety radius to check for obstacles
    ///
    /// # Returns
    /// true if location appears navigable
    pub fn is_navigable(&self, x: i32, y: i32, safety_radius: u64) -> bool {
        // Check for nearby obstacles
        let obstacle_count = self.obstacles_near(x, y, safety_radius).count();
        obstacle_count == 0
    }

    /// Adds map data to the CRDT
    fn add_map_data(&mut self, map_data: MapData) -> CRDTResult<()> {
        // Check for existing point at same location
        for i in 0..64 {
            if let Some(ref mut existing) = self.points[i] {
                let same_location =
                    existing.point.x == map_data.point.x && existing.point.y == map_data.point.y;

                if same_location {
                    // Update if new point should override
                    if map_data.point.should_override(&existing.point) {
                        *existing = map_data;
                    }
                    return Ok(());
                }
            } else {
                // Empty slot - add new point
                self.points[i] = Some(map_data);
                self.point_count += 1;
                return Ok(());
            }
        }

        // If no space, try to replace lowest confidence point
        self.make_space_for_point(map_data)
    }

    /// Makes space for a new point by replacing low-confidence points
    fn make_space_for_point(&mut self, new_data: MapData) -> CRDTResult<()> {
        // Find lowest confidence point to replace
        let mut lowest_idx = None;
        let mut lowest_confidence = u8::MAX;

        for (i, data_opt) in self.points.iter().enumerate() {
            if let Some(data) = data_opt {
                if data.point.confidence < lowest_confidence {
                    lowest_confidence = data.point.confidence;
                    lowest_idx = Some(i);
                }
            }
        }

        if let Some(idx) = lowest_idx {
            if new_data.point.confidence > lowest_confidence {
                self.points[idx] = Some(new_data);
                return Ok(());
            }
        }

        Err(CRDTError::BufferOverflow)
    }

    /// Validates map data
    ///
    /// # Returns
    /// Ok(()) if valid, error otherwise
    pub fn validate_map(&self) -> CRDTResult<()> {
        // Check observer IDs are valid
        for point in self.all_points() {
            if point.observer_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        Ok(())
    }
}

impl<C: MemoryConfig> CRDT<C> for SharedMap<C> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Merge all map data from other
        for data in other.points.iter().filter_map(|d| d.as_ref()) {
            self.add_map_data(*data)?;
        }

        // Update timestamp to latest
        if other.last_update > self.last_update {
            self.last_update = other.last_update;
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        if self.point_count != other.point_count {
            return false;
        }

        // Check that all points match
        for data in self.points.iter().filter_map(|d| d.as_ref()) {
            let mut found = false;
            for other_data in other.points.iter().filter_map(|d| d.as_ref()) {
                if data.hash == other_data.hash && data.point == other_data.point {
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
        self.validate_map()
    }

    fn state_hash(&self) -> u32 {
        let mut hash = self.local_robot_id as u32;
        for data in self.points.iter().filter_map(|d| d.as_ref()) {
            hash ^= data.hash;
        }
        hash ^= self.point_count as u32;
        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // Can always merge maps (space is made by replacing low-confidence points)
        true
    }
}

impl<C: MemoryConfig> BoundedCRDT<C> for SharedMap<C> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 64; // Maximum map points

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.point_count
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // Could implement cleanup of low-confidence points
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        self.point_count < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig> RealTimeCRDT<C> for SharedMap<C> {
    const MAX_MERGE_CYCLES: u32 = 200; // Bounded by number of points
    const MAX_VALIDATE_CYCLES: u32 = 100;
    const MAX_SERIALIZE_CYCLES: u32 = 150;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Map merge is bounded
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
    fn test_map_point_type_properties() {
        assert!(MapPointType::Free.is_navigable());
        assert!(MapPointType::Goal.is_navigable());
        assert!(!MapPointType::Obstacle.is_navigable());

        assert!(MapPointType::Landmark.is_poi());
        assert!(MapPointType::ChargingStation.is_poi());
        assert!(!MapPointType::Free.is_poi());
    }

    #[test]
    fn test_map_point_creation() {
        let point = MapPoint::new(1000, 2000, MapPointType::Obstacle, 200, 1000, 1);

        assert_eq!(point.x, 1000);
        assert_eq!(point.y, 2000);
        assert_eq!(point.point_type, MapPointType::Obstacle);
        assert_eq!(point.confidence, 200);
        assert_eq!(point.observer_id, 1);

        let grid_key = point.grid_key(100);
        assert_eq!(grid_key, (10, 20)); // 1000/100, 2000/100
    }

    #[test]
    fn test_map_point_distance() {
        let point1 = MapPoint::new(0, 0, MapPointType::Free, 100, 1000, 1);
        let point2 = MapPoint::new(3, 4, MapPointType::Free, 100, 1000, 1);

        assert_eq!(point1.distance_squared(&point2), 25); // 3^2 + 4^2 = 25
    }

    #[test]
    fn test_map_point_override() {
        let point1 = MapPoint::new(0, 0, MapPointType::Free, 100, 1000, 1);
        let point2 = MapPoint::new(0, 0, MapPointType::Obstacle, 150, 1001, 2); // Higher confidence
        let point3 = MapPoint::new(0, 0, MapPointType::Unknown, 100, 1002, 3); // Same confidence, newer

        assert!(point2.should_override(&point1)); // Higher confidence
        assert!(point3.should_override(&point1)); // Same confidence, newer timestamp
        assert!(!point1.should_override(&point2)); // Lower confidence
    }

    #[test]
    fn test_shared_map_creation() {
        let map = SharedMap::<DefaultConfig>::new(1);

        assert_eq!(map.point_count(), 0);
        assert_eq!(map.grid_size(), 100);

        let custom_map = SharedMap::<DefaultConfig>::with_grid_size(1, 50);
        assert_eq!(custom_map.grid_size(), 50);
    }

    #[test]
    fn test_map_observations() {
        let mut map = SharedMap::<DefaultConfig>::new(1);

        // Add obstacle
        map.add_observation(1000, 2000, MapPointType::Obstacle, 200, 1000)
            .unwrap();

        // Add landmark
        map.add_observation(1500, 2500, MapPointType::Landmark, 180, 1001)
            .unwrap();

        // Add free space
        map.add_observation(500, 1500, MapPointType::Free, 150, 1002)
            .unwrap();

        assert_eq!(map.point_count(), 3);

        // Test queries
        let obstacles_count = map.points_by_type(MapPointType::Obstacle).count();
        assert_eq!(obstacles_count, 1);
        let obstacle = map.points_by_type(MapPointType::Obstacle).next().unwrap();
        assert_eq!(obstacle.x, 1000);

        let pois_count = map.points_of_interest().count();
        assert_eq!(pois_count, 1);
        let poi = map.points_of_interest().next().unwrap();
        assert_eq!(poi.point_type, MapPointType::Landmark);

        let navigable_count = map.navigable_points().count();
        assert_eq!(navigable_count, 1);
        let navigable = map.navigable_points().next().unwrap();
        assert_eq!(navigable.point_type, MapPointType::Free);
    }

    #[test]
    fn test_spatial_queries() {
        let mut map = SharedMap::<DefaultConfig>::new(1);

        // Add points at different locations
        map.add_observation(0, 0, MapPointType::Free, 100, 1000)
            .unwrap();
        map.add_observation(100, 0, MapPointType::Obstacle, 150, 1001)
            .unwrap();
        map.add_observation(1000, 1000, MapPointType::Landmark, 200, 1002)
            .unwrap();

        // Test nearby points
        let nearby_count = map.points_near(50, 0, 10000).count(); // 100mm radius squared
        assert_eq!(nearby_count, 2); // First two points

        // Test obstacles near
        let obstacles_count = map.obstacles_near(50, 0, 10000).count();
        assert_eq!(obstacles_count, 1);
        let obstacle = map.obstacles_near(50, 0, 10000).next().unwrap();
        assert_eq!(obstacle.point_type, MapPointType::Obstacle);

        // Test nearest POI
        let nearest_poi = map.nearest_poi(500, 500).unwrap();
        assert_eq!(nearest_poi.point_type, MapPointType::Landmark);

        // Test navigability
        assert!(map.is_navigable(0, 0, 1000)); // Near free space
        assert!(!map.is_navigable(100, 0, 1000)); // Near obstacle
    }

    #[test]
    fn test_map_point_override_behavior() {
        let mut map = SharedMap::<DefaultConfig>::new(1);

        // Add initial observation
        map.add_observation(1000, 1000, MapPointType::Unknown, 100, 1000)
            .unwrap();
        assert_eq!(map.point_count(), 1);

        // Add higher confidence observation at same location
        map.add_observation(1000, 1000, MapPointType::Obstacle, 200, 1001)
            .unwrap();
        assert_eq!(map.point_count(), 1); // Should replace, not add

        let point = map.all_points().next().unwrap();
        assert_eq!(point.point_type, MapPointType::Obstacle);
        assert_eq!(point.confidence, 200);
    }

    #[test]
    fn test_shared_map_merge() {
        let mut map1 = SharedMap::<DefaultConfig>::new(1);
        let mut map2 = SharedMap::<DefaultConfig>::new(2);

        // Add different observations to each map
        map1.add_observation(0, 0, MapPointType::Free, 100, 1000)
            .unwrap();
        map2.add_observation(1000, 1000, MapPointType::Obstacle, 150, 1001)
            .unwrap();

        // Merge
        map1.merge(&map2).unwrap();

        // Should have both points
        assert_eq!(map1.point_count(), 2);
        assert!(map1.points_by_type(MapPointType::Free).next().is_some());
        assert!(map1.points_by_type(MapPointType::Obstacle).next().is_some());
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut map = SharedMap::<DefaultConfig>::new(1);

        assert_eq!(map.element_count(), 0);
        assert!(map.can_add_element());

        map.add_observation(0, 0, MapPointType::Free, 100, 1000)
            .unwrap();
        assert_eq!(map.element_count(), 1);
        assert!(map.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut map1 = SharedMap::<DefaultConfig>::new(1);
        let map2 = SharedMap::<DefaultConfig>::new(2);

        assert!(map1.merge_bounded(&map2).is_ok());
        assert!(map1.validate_bounded().is_ok());
    }
}
