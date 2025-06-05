//! Sensor Fusion CRDTs for Automotive Applications
//!
//! This module implements CRDTs for multi-sensor data fusion with reliability
//! weighting and automotive-specific sensor coordination patterns.

use crate::automotive::safety::{ASILLevel, SafetyLevel};
use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

/// Sensor reliability levels for automotive applications
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ReliabilityLevel {
    /// Low reliability sensor (e.g., single point sensor)
    Low = 1,
    /// Medium reliability sensor (e.g., redundant sensor)
    Medium = 2,
    /// High reliability sensor (e.g., safety-critical redundant)
    High = 3,
    /// Ultra-high reliability (e.g., triple redundant safety sensor)
    UltraHigh = 4,
}

impl ReliabilityLevel {
    /// Returns the weight factor for this reliability level
    pub fn weight(&self) -> f32 {
        match self {
            ReliabilityLevel::Low => 1.0,
            ReliabilityLevel::Medium => 2.0,
            ReliabilityLevel::High => 4.0,
            ReliabilityLevel::UltraHigh => 8.0,
        }
    }

    /// Returns true if this reliability level is suitable for safety-critical applications
    pub fn is_safety_suitable(&self) -> bool {
        *self >= ReliabilityLevel::High
    }
}

/// Individual sensor reading with metadata
#[derive(Debug, Clone, Copy)]
pub struct SensorReading<T> {
    /// Sensor value
    pub value: T,
    /// Timestamp when reading was taken
    pub timestamp: CompactTimestamp,
    /// Node ID of the sensor
    pub node_id: NodeId,
    /// Reliability level of this sensor
    pub reliability: ReliabilityLevel,
    /// Safety level of this sensor
    pub safety_level: SafetyLevel,
}

impl<T> SensorReading<T> {
    /// Creates a new sensor reading
    pub fn new(
        value: T,
        timestamp: u64,
        node_id: NodeId,
        reliability: ReliabilityLevel,
        safety_level: SafetyLevel,
    ) -> Self {
        Self {
            value,
            timestamp: CompactTimestamp::new(timestamp),
            node_id,
            reliability,
            safety_level,
        }
    }

    /// Returns the effective weight of this reading
    pub fn effective_weight(&self) -> f32 {
        let reliability_weight = self.reliability.weight();
        let safety_weight = match self.safety_level {
            SafetyLevel::Automotive(ASILLevel::QM) => 1.0,
            SafetyLevel::Automotive(ASILLevel::AsilA) => 1.5,
            SafetyLevel::Automotive(ASILLevel::AsilB) => 2.0,
            SafetyLevel::Automotive(ASILLevel::AsilC) => 3.0,
            SafetyLevel::Automotive(ASILLevel::AsilD) => 4.0,
            _ => 1.0,
        };
        reliability_weight * safety_weight
    }
}

/// Multi-sensor fusion CRDT for automotive applications
///
/// This CRDT aggregates sensor readings from multiple sources with
/// reliability weighting and safety-level prioritization.
///
/// # Type Parameters
/// - `T`: The sensor value type (must support arithmetic operations)
/// - `C`: Memory configuration
///
/// # Features
/// - Reliability-weighted averaging
/// - Safety-level prioritization
/// - Outlier detection and rejection
/// - Temporal consistency checking
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::automotive::{SensorFusion, SensorReading, ReliabilityLevel, SafetyLevel, ASILLevel};
///
/// // Create sensor fusion for temperature readings
/// let mut temp_fusion = SensorFusion::<f32, DefaultConfig>::new(1);
///
/// // Add readings from different sensors
/// let reading1 = SensorReading::new(
///     23.5, 1000, 1,
///     ReliabilityLevel::High,
///     SafetyLevel::automotive(ASILLevel::AsilC)
/// );
/// temp_fusion.add_reading(reading1)?;
///
/// let reading2 = SensorReading::new(
///     24.1, 1001, 2,
///     ReliabilityLevel::Medium,
///     SafetyLevel::automotive(ASILLevel::AsilB)
/// );
/// temp_fusion.add_reading(reading2)?;
///
/// // Get fused result
/// let fused_temp = temp_fusion.fused_value();
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct SensorFusion<T, C: MemoryConfig> {
    /// Fused sensor readings
    readings: [Option<SensorReading<T>>; 8],
    /// Current count of readings
    reading_count: usize,
    /// Node ID for this fusion unit
    #[allow(dead_code)]
    node_id: NodeId,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<T, C: MemoryConfig> SensorFusion<T, C>
where
    T: Clone + PartialEq + Copy,
{
    /// Creates a new sensor fusion CRDT
    ///
    /// # Arguments
    /// * `node_id` - The ID of this node
    ///
    /// # Returns
    /// A new empty sensor fusion CRDT
    pub fn new(node_id: NodeId) -> Self {
        Self {
            readings: [const { None }; 8],
            reading_count: 0,
            node_id,
            _phantom: core::marker::PhantomData,
        }
    }

    /// Adds a new sensor reading
    ///
    /// # Arguments
    /// * `reading` - The sensor reading to add
    ///
    /// # Returns
    /// Ok(()) if successful, error if fusion is full
    pub fn add_reading(&mut self, reading: SensorReading<T>) -> CRDTResult<()> {
        // Check if we already have a reading from this node
        for i in 0..self.reading_count {
            if let Some(ref mut existing) = self.readings[i] {
                if existing.node_id == reading.node_id {
                    // Update if newer timestamp or higher safety level
                    let should_update = reading.safety_level > existing.safety_level
                        || (reading.safety_level == existing.safety_level
                            && reading.timestamp > existing.timestamp);

                    if should_update {
                        *existing = reading;
                    }
                    return Ok(());
                }
            }
        }

        // Add new reading if we have space
        if self.reading_count >= 8 {
            return Err(CRDTError::BufferOverflow);
        }

        self.readings[self.reading_count] = Some(reading);
        self.reading_count += 1;
        Ok(())
    }

    /// Gets all current sensor readings
    ///
    /// # Returns
    /// Iterator over current sensor readings
    pub fn readings(&self) -> impl Iterator<Item = &SensorReading<T>> {
        self.readings
            .iter()
            .take(self.reading_count)
            .filter_map(|r| r.as_ref())
    }

    /// Returns the number of sensor readings
    ///
    /// # Returns
    /// Count of sensor readings
    pub fn reading_count(&self) -> usize {
        self.reading_count
    }

    /// Checks if the fusion has any readings
    ///
    /// # Returns
    /// true if no readings are present
    pub fn is_empty(&self) -> bool {
        self.reading_count == 0
    }

    /// Gets the highest safety level among all readings
    ///
    /// # Returns
    /// The highest safety level, or None if no readings
    pub fn max_safety_level(&self) -> Option<SafetyLevel> {
        self.readings().map(|r| r.safety_level).max()
    }

    /// Filters readings by minimum safety level
    ///
    /// # Arguments
    /// * `min_safety` - Minimum safety level to include
    ///
    /// # Returns
    /// Iterator over readings meeting the safety requirement
    pub fn safety_filtered_readings(
        &self,
        min_safety: SafetyLevel,
    ) -> impl Iterator<Item = &SensorReading<T>> {
        self.readings()
            .filter(move |r| r.safety_level >= min_safety)
    }

    /// Validates sensor readings for consistency
    ///
    /// # Returns
    /// Ok(()) if readings are consistent, error otherwise
    pub fn validate_readings(&self) -> CRDTResult<()> {
        // Check for valid node IDs
        for reading in self.readings() {
            if reading.node_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        // Check for temporal consistency (readings should be reasonably recent)
        if let Some(latest) = self.readings().map(|r| r.timestamp).max() {
            for reading in self.readings() {
                let time_diff = latest.as_u64().saturating_sub(reading.timestamp.as_u64());
                if time_diff > 10000 {
                    // 10 second threshold
                    return Err(CRDTError::InvalidState);
                }
            }
        }

        Ok(())
    }
}

// Numeric operations for sensor fusion
impl<C: MemoryConfig> SensorFusion<f32, C> {
    /// Computes the reliability-weighted average of all readings
    ///
    /// # Returns
    /// The fused sensor value, or None if no readings
    pub fn fused_value(&self) -> Option<f32> {
        if self.reading_count == 0 {
            return None;
        }

        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for reading in self.readings() {
            let weight = reading.effective_weight();
            weighted_sum += reading.value * weight;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            Some(weighted_sum / total_weight)
        } else {
            None
        }
    }

    /// Computes the safety-critical fused value (ASIL-C and above only)
    ///
    /// # Returns
    /// The safety-critical fused value, or None if no safety-critical readings
    pub fn safety_critical_value(&self) -> Option<f32> {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        let mut has_safety_readings = false;

        for reading in self.safety_filtered_readings(SafetyLevel::automotive(ASILLevel::AsilC)) {
            has_safety_readings = true;
            let weight = reading.effective_weight();
            weighted_sum += reading.value * weight;
            total_weight += weight;
        }

        if has_safety_readings && total_weight > 0.0 {
            Some(weighted_sum / total_weight)
        } else {
            None
        }
    }

    /// Computes the variance of sensor readings
    ///
    /// # Returns
    /// The variance of readings, or None if insufficient data
    pub fn variance(&self) -> Option<f32> {
        if self.reading_count < 2 {
            return None;
        }

        let mean = self.fused_value()?;
        let mut variance_sum = 0.0;
        let mut total_weight = 0.0;

        for reading in self.readings() {
            let weight = reading.effective_weight();
            let diff = reading.value - mean;
            variance_sum += weight * diff * diff;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            Some(variance_sum / total_weight)
        } else {
            None
        }
    }

    /// Detects outlier readings based on statistical analysis
    ///
    /// # Arguments
    /// * `threshold` - Standard deviation threshold for outlier detection
    ///
    /// # Returns
    /// Vector of node IDs with outlier readings
    pub fn detect_outliers(&self, threshold: f32) -> [Option<NodeId>; 8] {
        let mean = match self.fused_value() {
            Some(m) => m,
            None => return [const { None }; 8],
        };

        let variance = match self.variance() {
            Some(v) => v,
            None => return [const { None }; 8],
        };

        // Simple sqrt approximation for no_std
        let std_dev = {
            let mut x = variance;
            if x == 0.0 {
                0.0
            } else {
                // Newton's method for sqrt approximation
                for _ in 0..10 {
                    x = 0.5 * (x + variance / x);
                }
                x
            }
        };

        let mut outliers = [const { None }; 8];
        let mut outlier_count = 0;

        for reading in self.readings() {
            if outlier_count >= 8 {
                break;
            }
            let deviation = (reading.value - mean).abs();
            if deviation > threshold * std_dev {
                outliers[outlier_count] = Some(reading.node_id);
                outlier_count += 1;
            }
        }

        outliers
    }
}

impl<T, C: MemoryConfig> CRDT<C> for SensorFusion<T, C>
where
    T: Clone + PartialEq + Copy + core::fmt::Debug,
{
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Merge all readings from other
        for reading in other.readings() {
            self.add_reading(*reading)?;
        }
        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        if self.reading_count != other.reading_count {
            return false;
        }

        // Check that all readings match (order doesn't matter)
        for reading in self.readings() {
            let mut found = false;
            for other_reading in other.readings() {
                if reading.node_id == other_reading.node_id
                    && reading.timestamp == other_reading.timestamp
                    && reading.value == other_reading.value
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
        self.validate_readings()
    }

    fn state_hash(&self) -> u32 {
        let mut hash = 0u32;
        for reading in self.readings() {
            let value_ptr = &reading.value as *const T as usize;
            hash ^=
                (value_ptr as u32) ^ (reading.timestamp.as_u64() as u32) ^ (reading.node_id as u32);
        }
        hash ^= self.reading_count as u32;
        hash
    }

    fn can_merge(&self, other: &Self) -> bool {
        // Check if merging would exceed capacity
        let mut new_nodes = 0;
        for other_reading in other.readings() {
            let mut found = false;
            for our_reading in self.readings() {
                if our_reading.node_id == other_reading.node_id {
                    found = true;
                    break;
                }
            }
            if !found {
                new_nodes += 1;
            }
        }

        self.reading_count + new_nodes <= 8
    }
}

impl<T, C: MemoryConfig> BoundedCRDT<C> for SensorFusion<T, C>
where
    T: Clone + PartialEq + Copy + core::fmt::Debug,
{
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 8; // Maximum number of sensor readings

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.reading_count
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // Remove oldest readings if we're at capacity
        // This is a simple compaction strategy
        Ok(0) // No compaction for now
    }

    fn can_add_element(&self) -> bool {
        self.reading_count < Self::MAX_ELEMENTS
    }
}

impl<T, C: MemoryConfig> RealTimeCRDT<C> for SensorFusion<T, C>
where
    T: Clone + PartialEq + Copy + core::fmt::Debug,
{
    const MAX_MERGE_CYCLES: u32 = 200; // Bounded by number of readings
    const MAX_VALIDATE_CYCLES: u32 = 100;
    const MAX_SERIALIZE_CYCLES: u32 = 150;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Sensor fusion merge is bounded by the number of readings
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded by the number of readings
        self.validate()
    }

    fn remaining_budget(&self) -> Option<u32> {
        // For automotive systems, we don't track budget
        None
    }

    fn set_budget(&mut self, _cycles: u32) {
        // For automotive systems, we don't limit budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::DefaultConfig;

    #[test]
    fn test_reliability_level_weights() {
        assert_eq!(ReliabilityLevel::Low.weight(), 1.0);
        assert_eq!(ReliabilityLevel::Medium.weight(), 2.0);
        assert_eq!(ReliabilityLevel::High.weight(), 4.0);
        assert_eq!(ReliabilityLevel::UltraHigh.weight(), 8.0);

        assert!(ReliabilityLevel::High.is_safety_suitable());
        assert!(!ReliabilityLevel::Low.is_safety_suitable());
    }

    #[test]
    fn test_sensor_reading_creation() {
        let reading = SensorReading::new(
            25.0,
            1000,
            1,
            ReliabilityLevel::High,
            SafetyLevel::automotive(ASILLevel::AsilC),
        );

        assert_eq!(reading.value, 25.0);
        assert_eq!(reading.node_id, 1);
        assert!(reading.effective_weight() > 4.0); // High reliability * ASIL-C
    }

    #[test]
    fn test_sensor_fusion_creation() {
        let fusion = SensorFusion::<f32, DefaultConfig>::new(1);
        assert!(fusion.is_empty());
        assert_eq!(fusion.reading_count(), 0);
        assert_eq!(fusion.fused_value(), None);
    }

    #[test]
    fn test_add_sensor_readings() {
        let mut fusion = SensorFusion::<f32, DefaultConfig>::new(1);

        let reading1 = SensorReading::new(
            23.0,
            1000,
            1,
            ReliabilityLevel::High,
            SafetyLevel::automotive(ASILLevel::AsilC),
        );

        let reading2 = SensorReading::new(
            25.0,
            1001,
            2,
            ReliabilityLevel::Medium,
            SafetyLevel::automotive(ASILLevel::AsilB),
        );

        assert!(fusion.add_reading(reading1).is_ok());
        assert!(fusion.add_reading(reading2).is_ok());

        assert_eq!(fusion.reading_count(), 2);
        assert!(!fusion.is_empty());
    }

    #[test]
    fn test_weighted_fusion() {
        let mut fusion = SensorFusion::<f32, DefaultConfig>::new(1);

        // High reliability, high safety reading
        let reading1 = SensorReading::new(
            20.0,
            1000,
            1,
            ReliabilityLevel::High,
            SafetyLevel::automotive(ASILLevel::AsilD),
        );

        // Low reliability, low safety reading
        let reading2 = SensorReading::new(
            30.0,
            1001,
            2,
            ReliabilityLevel::Low,
            SafetyLevel::automotive(ASILLevel::QM),
        );

        fusion.add_reading(reading1).unwrap();
        fusion.add_reading(reading2).unwrap();

        let fused = fusion.fused_value().unwrap();
        // Should be closer to 20.0 due to higher weight
        assert!(fused < 25.0);
    }

    #[test]
    fn test_safety_critical_filtering() {
        let mut fusion = SensorFusion::<f32, DefaultConfig>::new(1);

        let reading1 = SensorReading::new(
            20.0,
            1000,
            1,
            ReliabilityLevel::High,
            SafetyLevel::automotive(ASILLevel::AsilD),
        );

        let reading2 = SensorReading::new(
            30.0,
            1001,
            2,
            ReliabilityLevel::Low,
            SafetyLevel::automotive(ASILLevel::QM),
        );

        fusion.add_reading(reading1).unwrap();
        fusion.add_reading(reading2).unwrap();

        let safety_critical = fusion.safety_critical_value().unwrap();
        assert_eq!(safety_critical, 20.0); // Only ASIL-D reading

        let max_safety = fusion.max_safety_level().unwrap();
        assert_eq!(max_safety, SafetyLevel::automotive(ASILLevel::AsilD));
    }

    #[test]
    fn test_outlier_detection() {
        let mut fusion = SensorFusion::<f32, DefaultConfig>::new(1);

        // Normal readings
        fusion
            .add_reading(SensorReading::new(
                20.0,
                1000,
                1,
                ReliabilityLevel::High,
                SafetyLevel::automotive(ASILLevel::AsilC),
            ))
            .unwrap();

        fusion
            .add_reading(SensorReading::new(
                21.0,
                1001,
                2,
                ReliabilityLevel::High,
                SafetyLevel::automotive(ASILLevel::AsilC),
            ))
            .unwrap();

        // Outlier reading
        fusion
            .add_reading(SensorReading::new(
                100.0,
                1002,
                3,
                ReliabilityLevel::Low,
                SafetyLevel::automotive(ASILLevel::QM),
            ))
            .unwrap();

        let outliers = fusion.detect_outliers(2.0);
        assert!(outliers.contains(&Some(3))); // Node 3 should be detected as outlier
    }

    #[test]
    fn test_sensor_fusion_merge() {
        let mut fusion1 = SensorFusion::<f32, DefaultConfig>::new(1);
        let mut fusion2 = SensorFusion::<f32, DefaultConfig>::new(2);

        fusion1
            .add_reading(SensorReading::new(
                20.0,
                1000,
                1,
                ReliabilityLevel::High,
                SafetyLevel::automotive(ASILLevel::AsilC),
            ))
            .unwrap();

        fusion2
            .add_reading(SensorReading::new(
                25.0,
                1001,
                2,
                ReliabilityLevel::Medium,
                SafetyLevel::automotive(ASILLevel::AsilB),
            ))
            .unwrap();

        fusion1.merge(&fusion2).unwrap();
        assert_eq!(fusion1.reading_count(), 2);
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut fusion = SensorFusion::<f32, DefaultConfig>::new(1);

        assert_eq!(fusion.element_count(), 0);
        assert!(fusion.can_add_element());

        fusion
            .add_reading(SensorReading::new(
                20.0,
                1000,
                1,
                ReliabilityLevel::High,
                SafetyLevel::automotive(ASILLevel::AsilC),
            ))
            .unwrap();

        assert_eq!(fusion.element_count(), 1);
        assert!(fusion.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut fusion1 = SensorFusion::<f32, DefaultConfig>::new(1);
        let fusion2 = SensorFusion::<f32, DefaultConfig>::new(2);

        assert!(fusion1.merge_bounded(&fusion2).is_ok());
        assert!(fusion1.validate_bounded().is_ok());
    }
}
