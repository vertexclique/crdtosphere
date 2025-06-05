//! Sensor Network for IoT Systems
//!
//! This module implements CRDTs for distributed IoT sensor data coordination,
//! enabling aggregation and synchronization of sensor readings across networks.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

/// IoT sensor types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SensorType {
    /// Temperature sensor
    Temperature = 1,
    /// Humidity sensor
    Humidity = 2,
    /// Pressure sensor
    Pressure = 3,
    /// Light/illuminance sensor
    Light = 4,
    /// Motion/PIR sensor
    Motion = 5,
    /// Air quality sensor
    AirQuality = 6,
    /// Sound/noise level sensor
    Sound = 7,
    /// Proximity sensor
    Proximity = 8,
    /// Accelerometer
    Accelerometer = 9,
    /// GPS/location sensor
    GPS = 10,
    /// Generic analog sensor
    Analog = 11,
    /// Generic digital sensor
    Digital = 12,
}

impl SensorType {
    /// Returns true if this sensor type provides continuous readings
    pub fn is_continuous(&self) -> bool {
        matches!(
            self,
            SensorType::Temperature
                | SensorType::Humidity
                | SensorType::Pressure
                | SensorType::Light
                | SensorType::AirQuality
                | SensorType::Sound
                | SensorType::Analog
        )
    }

    /// Returns true if this sensor type provides event-based readings
    pub fn is_event_based(&self) -> bool {
        matches!(
            self,
            SensorType::Motion | SensorType::Proximity | SensorType::Digital
        )
    }

    /// Returns typical update interval in milliseconds
    pub fn typical_interval_ms(&self) -> u32 {
        match self {
            SensorType::Temperature | SensorType::Humidity => 30000, // 30 seconds
            SensorType::Pressure => 60000,                           // 1 minute
            SensorType::Light => 10000,                              // 10 seconds
            SensorType::Motion => 1000,                              // 1 second (when active)
            SensorType::AirQuality => 60000,                         // 1 minute
            SensorType::Sound => 5000,                               // 5 seconds
            SensorType::Proximity => 500,                            // 500ms
            SensorType::Accelerometer => 100,                        // 100ms
            SensorType::GPS => 30000,                                // 30 seconds
            SensorType::Analog => 5000,                              // 5 seconds
            SensorType::Digital => 1000,                             // 1 second
        }
    }
}

/// Reading quality indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ReadingQuality {
    /// Poor quality reading (high noise, low confidence)
    Poor = 1,
    /// Fair quality reading
    Fair = 2,
    /// Good quality reading
    Good = 3,
    /// Excellent quality reading (low noise, high confidence)
    Excellent = 4,
}

impl ReadingQuality {
    /// Returns confidence weight for this quality level
    pub fn confidence_weight(&self) -> f32 {
        match self {
            ReadingQuality::Poor => 0.25,
            ReadingQuality::Fair => 0.5,
            ReadingQuality::Good => 0.75,
            ReadingQuality::Excellent => 1.0,
        }
    }

    /// Returns true if this quality is acceptable for critical decisions
    pub fn is_acceptable(&self) -> bool {
        matches!(self, ReadingQuality::Good | ReadingQuality::Excellent)
    }
}

/// Individual sensor reading
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SensorReading {
    /// Sensor that produced this reading
    pub sensor_id: NodeId,
    /// Type of sensor
    pub sensor_type: SensorType,
    /// Reading value (scaled and encoded as i32)
    pub value: i32,
    /// Reading quality
    pub quality: ReadingQuality,
    /// Timestamp when reading was taken
    pub timestamp: CompactTimestamp,
    /// Location/zone identifier (optional)
    pub location_id: u16,
    /// Battery level of sensor (0-255)
    pub battery_level: u8,
    /// Signal strength when reading was transmitted (0-255)
    pub signal_strength: u8,
}

impl SensorReading {
    /// Creates a new sensor reading
    pub fn new(
        sensor_id: NodeId,
        sensor_type: SensorType,
        value: i32,
        quality: ReadingQuality,
        timestamp: u64,
        location_id: u16,
    ) -> Self {
        Self {
            sensor_id,
            sensor_type,
            value,
            quality,
            timestamp: CompactTimestamp::new(timestamp),
            location_id,
            battery_level: 255,   // Unknown/AC powered
            signal_strength: 255, // Unknown/wired
        }
    }

    /// Creates a reading with battery and signal info
    pub fn with_vitals(
        sensor_id: NodeId,
        sensor_type: SensorType,
        value: i32,
        quality: ReadingQuality,
        timestamp: u64,
        location_id: u16,
        battery_level: u8,
        signal_strength: u8,
    ) -> Self {
        Self {
            sensor_id,
            sensor_type,
            value,
            quality,
            timestamp: CompactTimestamp::new(timestamp),
            location_id,
            battery_level,
            signal_strength,
        }
    }

    /// Returns true if this reading should override another
    pub fn should_override(&self, other: &SensorReading) -> bool {
        // Same sensor - newer timestamp wins
        if self.sensor_id == other.sensor_id {
            return self.timestamp > other.timestamp;
        }

        // Different sensors - higher quality wins, then newer timestamp
        if self.quality > other.quality {
            return true;
        }

        if self.quality == other.quality {
            return self.timestamp > other.timestamp;
        }

        false
    }

    /// Returns true if reading is stale
    pub fn is_stale(&self, current_time: u64, max_age_ms: u64) -> bool {
        current_time > self.timestamp.as_u64() + max_age_ms
    }

    /// Returns weighted value based on quality
    pub fn weighted_value(&self) -> f32 {
        self.value as f32 * self.quality.confidence_weight()
    }
}

/// IoT Sensor Network CRDT
///
/// This CRDT manages distributed sensor data coordination across IoT networks,
/// enabling aggregation and synchronization of sensor readings.
///
/// # Type Parameters
/// - `C`: Memory configuration
///
/// # Features
/// - Multi-sensor data aggregation
/// - Quality-based reading prioritization
/// - Location-based sensor grouping
/// - Automatic stale data cleanup
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::iot::{SensorNetwork, SensorReading, SensorType, ReadingQuality};
///
/// // Create sensor network
/// let mut network = SensorNetwork::<DefaultConfig>::new(1); // Gateway ID 1
///
/// // Add temperature reading
/// network.add_reading(
///     42,                        // sensor ID
///     SensorType::Temperature,
///     2350,                      // 23.5°C (scaled by 100)
///     ReadingQuality::Good,
///     1000,                      // timestamp
///     1                          // location ID
/// )?;
///
/// // Get latest temperature readings
/// let temp_readings = network.readings_by_type(SensorType::Temperature);
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct SensorNetwork<C: MemoryConfig> {
    /// Array of sensor readings
    readings: [Option<SensorReading>; 128], // Support up to 128 readings
    /// Number of readings currently stored
    reading_count: usize,
    /// This gateway's ID
    local_gateway_id: NodeId,
    /// Last update timestamp
    last_update: CompactTimestamp,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<C: MemoryConfig> SensorNetwork<C> {
    /// Creates a new sensor network
    ///
    /// # Arguments
    /// * `gateway_id` - The ID of this gateway/controller
    ///
    /// # Returns
    /// A new sensor network CRDT
    pub fn new(gateway_id: NodeId) -> Self {
        Self {
            readings: [const { None }; 128],
            reading_count: 0,
            local_gateway_id: gateway_id,
            last_update: CompactTimestamp::new(0),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Adds a sensor reading
    ///
    /// # Arguments
    /// * `sensor_id` - Sensor identifier
    /// * `sensor_type` - Type of sensor
    /// * `value` - Reading value
    /// * `quality` - Reading quality
    /// * `timestamp` - Reading timestamp
    /// * `location_id` - Location/zone identifier
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn add_reading(
        &mut self,
        sensor_id: NodeId,
        sensor_type: SensorType,
        value: i32,
        quality: ReadingQuality,
        timestamp: u64,
        location_id: u16,
    ) -> CRDTResult<()> {
        let reading = SensorReading::new(
            sensor_id,
            sensor_type,
            value,
            quality,
            timestamp,
            location_id,
        );
        self.add_sensor_reading(reading)?;
        self.last_update = CompactTimestamp::new(timestamp);
        Ok(())
    }

    /// Adds a sensor reading with battery and signal info
    ///
    /// # Arguments
    /// * `sensor_id` - Sensor identifier
    /// * `sensor_type` - Type of sensor
    /// * `value` - Reading value
    /// * `quality` - Reading quality
    /// * `timestamp` - Reading timestamp
    /// * `location_id` - Location/zone identifier
    /// * `battery_level` - Battery level (0-255)
    /// * `signal_strength` - Signal strength (0-255)
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn add_reading_with_vitals(
        &mut self,
        sensor_id: NodeId,
        sensor_type: SensorType,
        value: i32,
        quality: ReadingQuality,
        timestamp: u64,
        location_id: u16,
        battery_level: u8,
        signal_strength: u8,
    ) -> CRDTResult<()> {
        let reading = SensorReading::with_vitals(
            sensor_id,
            sensor_type,
            value,
            quality,
            timestamp,
            location_id,
            battery_level,
            signal_strength,
        );
        self.add_sensor_reading(reading)?;
        self.last_update = CompactTimestamp::new(timestamp);
        Ok(())
    }

    /// Gets all sensor readings
    ///
    /// # Returns
    /// Iterator over sensor readings
    pub fn all_readings(&self) -> impl Iterator<Item = &SensorReading> {
        self.readings.iter().filter_map(|r| r.as_ref())
    }

    /// Gets readings by sensor type
    ///
    /// # Arguments
    /// * `sensor_type` - Type of sensor to filter by
    ///
    /// # Returns
    /// Iterator over readings of the specified type
    pub fn readings_by_type(
        &self,
        sensor_type: SensorType,
    ) -> impl Iterator<Item = &SensorReading> {
        self.all_readings()
            .filter(move |r| r.sensor_type == sensor_type)
    }

    /// Gets readings by location
    ///
    /// # Arguments
    /// * `location_id` - Location to filter by
    ///
    /// # Returns
    /// Iterator over readings from the specified location
    pub fn readings_by_location(&self, location_id: u16) -> impl Iterator<Item = &SensorReading> {
        self.all_readings()
            .filter(move |r| r.location_id == location_id)
    }

    /// Gets readings by sensor ID
    ///
    /// # Arguments
    /// * `sensor_id` - Sensor to filter by
    ///
    /// # Returns
    /// Iterator over readings from the specified sensor
    pub fn readings_by_sensor(&self, sensor_id: NodeId) -> impl Iterator<Item = &SensorReading> {
        self.all_readings()
            .filter(move |r| r.sensor_id == sensor_id)
    }

    /// Gets readings with acceptable quality
    ///
    /// # Returns
    /// Iterator over high-quality readings
    pub fn quality_readings(&self) -> impl Iterator<Item = &SensorReading> {
        self.all_readings().filter(|r| r.quality.is_acceptable())
    }

    /// Gets latest reading for a sensor type and location
    ///
    /// # Arguments
    /// * `sensor_type` - Type of sensor
    /// * `location_id` - Location identifier
    ///
    /// # Returns
    /// Latest reading if available
    pub fn latest_reading(
        &self,
        sensor_type: SensorType,
        location_id: u16,
    ) -> Option<&SensorReading> {
        self.readings_by_type(sensor_type)
            .filter(|r| r.location_id == location_id)
            .max_by_key(|r| r.timestamp.as_u64())
    }

    /// Calculates average value for a sensor type and location
    ///
    /// # Arguments
    /// * `sensor_type` - Type of sensor
    /// * `location_id` - Location identifier
    /// * `max_age_ms` - Maximum age of readings to include
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    /// Average value if readings are available
    pub fn average_value(
        &self,
        sensor_type: SensorType,
        location_id: u16,
        max_age_ms: u64,
        current_time: u64,
    ) -> Option<f32> {
        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;
        let mut count = 0;

        for reading in self
            .readings_by_type(sensor_type)
            .filter(|r| r.location_id == location_id)
            .filter(|r| !r.is_stale(current_time, max_age_ms))
            .filter(|r| r.quality.is_acceptable())
        {
            sum += reading.weighted_value();
            weight_sum += reading.quality.confidence_weight();
            count += 1;
        }

        if count > 0 && weight_sum > 0.0 {
            Some(sum / weight_sum)
        } else {
            None
        }
    }

    /// Gets sensors with low battery
    ///
    /// # Arguments
    /// * `threshold` - Battery level threshold (0-255)
    ///
    /// # Returns
    /// Iterator over readings from sensors with low battery
    pub fn low_battery_sensors(&self, threshold: u8) -> impl Iterator<Item = &SensorReading> {
        self.all_readings()
            .filter(move |r| r.battery_level < threshold && r.battery_level < 255)
    }

    /// Gets sensors with weak signal
    ///
    /// # Arguments
    /// * `threshold` - Signal strength threshold (0-255)
    ///
    /// # Returns
    /// Iterator over readings from sensors with weak signal
    pub fn weak_signal_sensors(&self, threshold: u8) -> impl Iterator<Item = &SensorReading> {
        self.all_readings()
            .filter(move |r| r.signal_strength < threshold && r.signal_strength < 255)
    }

    /// Gets the number of readings
    ///
    /// # Returns
    /// Number of readings
    pub fn reading_count(&self) -> usize {
        self.reading_count
    }

    /// Removes stale readings
    ///
    /// # Arguments
    /// * `current_time` - Current timestamp
    /// * `max_age_ms` - Maximum age in milliseconds
    ///
    /// # Returns
    /// Number of readings removed
    pub fn cleanup_stale_readings(&mut self, current_time: u64, max_age_ms: u64) -> usize {
        let mut removed = 0;

        for i in 0..128 {
            if let Some(reading) = &self.readings[i] {
                if reading.is_stale(current_time, max_age_ms) {
                    self.readings[i] = None;
                    self.reading_count -= 1;
                    removed += 1;
                }
            }
        }

        // Compact the array
        self.compact_readings();

        removed
    }

    /// Adds a sensor reading to the network
    fn add_sensor_reading(&mut self, reading: SensorReading) -> CRDTResult<()> {
        // Check for existing reading from same sensor
        for i in 0..128 {
            if let Some(ref mut existing) = self.readings[i] {
                if existing.sensor_id == reading.sensor_id
                    && existing.sensor_type == reading.sensor_type
                {
                    // Update if new reading should override
                    if reading.should_override(existing) {
                        *existing = reading;
                    }
                    return Ok(());
                }
            } else {
                // Empty slot - add new reading
                self.readings[i] = Some(reading);
                self.reading_count += 1;
                return Ok(());
            }
        }

        // If no space, try to replace oldest poor quality reading
        self.make_space_for_reading(reading)
    }

    /// Makes space for a new reading by replacing old poor quality readings
    fn make_space_for_reading(&mut self, new_reading: SensorReading) -> CRDTResult<()> {
        // Find oldest poor quality reading to replace
        let mut oldest_idx = None;
        let mut oldest_time = u64::MAX;

        for (i, reading_opt) in self.readings.iter().enumerate() {
            if let Some(reading) = reading_opt {
                if reading.quality <= ReadingQuality::Fair
                    && reading.timestamp.as_u64() < oldest_time
                {
                    oldest_time = reading.timestamp.as_u64();
                    oldest_idx = Some(i);
                }
            }
        }

        if let Some(idx) = oldest_idx {
            self.readings[idx] = Some(new_reading);
            Ok(())
        } else {
            Err(CRDTError::BufferOverflow)
        }
    }

    /// Compacts the readings array
    fn compact_readings(&mut self) {
        let mut write_idx = 0;

        for read_idx in 0..128 {
            if let Some(reading) = self.readings[read_idx] {
                if write_idx != read_idx {
                    self.readings[write_idx] = Some(reading);
                    self.readings[read_idx] = None;
                }
                write_idx += 1;
            }
        }
    }

    /// Validates sensor network data
    ///
    /// # Returns
    /// Ok(()) if valid, error otherwise
    pub fn validate_network(&self) -> CRDTResult<()> {
        // Check sensor IDs are valid
        for reading in self.all_readings() {
            if reading.sensor_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        Ok(())
    }
}

impl<C: MemoryConfig> CRDT<C> for SensorNetwork<C> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Merge all readings from other
        for reading in other.all_readings() {
            self.add_sensor_reading(*reading)?;
        }

        // Update timestamp to latest
        if other.last_update > self.last_update {
            self.last_update = other.last_update;
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        if self.reading_count != other.reading_count {
            return false;
        }

        // Check that all readings match
        for reading in self.all_readings() {
            let mut found = false;
            for other_reading in other.all_readings() {
                if reading.sensor_id == other_reading.sensor_id
                    && reading.timestamp == other_reading.timestamp
                    && reading == other_reading
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
        self.validate_network()
    }

    fn state_hash(&self) -> u32 {
        let mut hash = self.local_gateway_id as u32;
        for reading in self.all_readings() {
            hash ^= (reading.sensor_id as u32)
                ^ (reading.timestamp.as_u64() as u32)
                ^ (reading.value as u32);
        }
        hash ^= self.reading_count as u32;
        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // Can always merge sensor networks (space is made by removing old readings)
        true
    }
}

impl<C: MemoryConfig> BoundedCRDT<C> for SensorNetwork<C> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 128; // Maximum readings

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.reading_count
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        self.compact_readings();
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        self.reading_count < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig> RealTimeCRDT<C> for SensorNetwork<C> {
    const MAX_MERGE_CYCLES: u32 = 300; // Bounded by number of readings
    const MAX_VALIDATE_CYCLES: u32 = 150;
    const MAX_SERIALIZE_CYCLES: u32 = 200;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Sensor network merge is bounded
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
    fn test_sensor_type_properties() {
        assert!(SensorType::Temperature.is_continuous());
        assert!(!SensorType::Temperature.is_event_based());

        assert!(SensorType::Motion.is_event_based());
        assert!(!SensorType::Motion.is_continuous());

        assert!(SensorType::Temperature.typical_interval_ms() > 1000);
    }

    #[test]
    fn test_reading_quality_properties() {
        assert!(
            ReadingQuality::Excellent.confidence_weight()
                > ReadingQuality::Poor.confidence_weight()
        );
        assert!(ReadingQuality::Good.is_acceptable());
        assert!(!ReadingQuality::Poor.is_acceptable());
    }

    #[test]
    fn test_sensor_reading_creation() {
        let reading = SensorReading::new(
            42,
            SensorType::Temperature,
            2350, // 23.5°C
            ReadingQuality::Good,
            1000,
            1,
        );

        assert_eq!(reading.sensor_id, 42);
        assert_eq!(reading.sensor_type, SensorType::Temperature);
        assert_eq!(reading.value, 2350);
        assert_eq!(reading.quality, ReadingQuality::Good);
        assert_eq!(reading.location_id, 1);
    }

    #[test]
    fn test_sensor_reading_override() {
        let reading1 = SensorReading::new(
            42,
            SensorType::Temperature,
            2300,
            ReadingQuality::Good,
            1000,
            1,
        );
        let reading2 = SensorReading::new(
            42,
            SensorType::Temperature,
            2350,
            ReadingQuality::Good,
            1001,
            1,
        );
        let reading3 = SensorReading::new(
            43,
            SensorType::Temperature,
            2400,
            ReadingQuality::Excellent,
            999,
            1,
        );

        assert!(reading2.should_override(&reading1)); // Same sensor, newer timestamp
        assert!(reading3.should_override(&reading1)); // Different sensor, higher quality
        assert!(!reading1.should_override(&reading2)); // Same sensor, older timestamp
    }

    #[test]
    fn test_sensor_network_creation() {
        let network = SensorNetwork::<DefaultConfig>::new(1);

        assert_eq!(network.reading_count(), 0);
        assert_eq!(network.local_gateway_id, 1);
    }

    #[test]
    fn test_sensor_reading_addition() {
        let mut network = SensorNetwork::<DefaultConfig>::new(1);

        // Add temperature reading
        network
            .add_reading(
                42,
                SensorType::Temperature,
                2350,
                ReadingQuality::Good,
                1000,
                1,
            )
            .unwrap();

        assert_eq!(network.reading_count(), 1);

        // Add humidity reading
        network
            .add_reading_with_vitals(
                43,
                SensorType::Humidity,
                6500, // 65%
                ReadingQuality::Excellent,
                1001,
                1,
                80,  // Battery level
                200, // Signal strength
            )
            .unwrap();

        assert_eq!(network.reading_count(), 2);
    }

    #[test]
    fn test_sensor_network_queries() {
        let mut network = SensorNetwork::<DefaultConfig>::new(1);

        // Add multiple readings
        network
            .add_reading(
                42,
                SensorType::Temperature,
                2350,
                ReadingQuality::Good,
                1000,
                1,
            )
            .unwrap();
        network
            .add_reading(
                43,
                SensorType::Temperature,
                2400,
                ReadingQuality::Excellent,
                1001,
                2,
            )
            .unwrap();
        network
            .add_reading(
                44,
                SensorType::Humidity,
                6500,
                ReadingQuality::Good,
                1002,
                1,
            )
            .unwrap();
        network
            .add_reading_with_vitals(
                45,
                SensorType::Motion,
                1,
                ReadingQuality::Fair,
                1003,
                1,
                20,
                100,
            )
            .unwrap();

        // Test queries
        assert_eq!(network.readings_by_type(SensorType::Temperature).count(), 2);
        assert_eq!(network.readings_by_location(1).count(), 3);
        assert_eq!(network.readings_by_sensor(42).count(), 1);
        assert_eq!(network.quality_readings().count(), 3); // Good and Excellent only

        // Test latest reading
        let latest_temp = network.latest_reading(SensorType::Temperature, 2).unwrap();
        assert_eq!(latest_temp.sensor_id, 43);

        // Test low battery sensors
        assert_eq!(network.low_battery_sensors(50).count(), 1); // Sensor 45 has 20% battery
    }

    #[test]
    fn test_sensor_network_average() {
        let mut network = SensorNetwork::<DefaultConfig>::new(1);

        // Add temperature readings from same location
        network
            .add_reading(
                42,
                SensorType::Temperature,
                2300,
                ReadingQuality::Good,
                1000,
                1,
            )
            .unwrap();
        network
            .add_reading(
                43,
                SensorType::Temperature,
                2400,
                ReadingQuality::Excellent,
                1001,
                1,
            )
            .unwrap();
        network
            .add_reading(
                44,
                SensorType::Temperature,
                2350,
                ReadingQuality::Good,
                1002,
                1,
            )
            .unwrap();

        // Calculate average (should be weighted by quality)
        let avg = network
            .average_value(SensorType::Temperature, 1, 10000, 2000)
            .unwrap();

        // Expected calculation:
        // Reading 1: 2300 * 0.75 = 1725
        // Reading 2: 2400 * 1.0 = 2400
        // Reading 3: 2350 * 0.75 = 1762.5
        // Sum = 5887.5, Weight sum = 2.5
        // Average = 5887.5 / 2.5 = 2355
        assert!((avg - 2355.0).abs() < 1.0);
    }

    #[test]
    fn test_sensor_network_merge() {
        let mut network1 = SensorNetwork::<DefaultConfig>::new(1);
        let mut network2 = SensorNetwork::<DefaultConfig>::new(2);

        // Add different readings to each network
        network1
            .add_reading(
                42,
                SensorType::Temperature,
                2300,
                ReadingQuality::Good,
                1000,
                1,
            )
            .unwrap();
        network2
            .add_reading(
                43,
                SensorType::Humidity,
                6500,
                ReadingQuality::Excellent,
                1001,
                1,
            )
            .unwrap();

        // Merge
        network1.merge(&network2).unwrap();

        // Should have both readings
        assert_eq!(network1.reading_count(), 2);
        assert_eq!(
            network1.readings_by_type(SensorType::Temperature).count(),
            1
        );
        assert_eq!(network1.readings_by_type(SensorType::Humidity).count(), 1);
    }

    #[test]
    fn test_stale_reading_cleanup() {
        let mut network = SensorNetwork::<DefaultConfig>::new(1);

        // Add reading
        network
            .add_reading(
                42,
                SensorType::Temperature,
                2300,
                ReadingQuality::Good,
                1000,
                1,
            )
            .unwrap();
        assert_eq!(network.reading_count(), 1);

        // Clean up after timeout
        let removed = network.cleanup_stale_readings(10000, 5000); // 5 second timeout
        assert_eq!(removed, 1);
        assert_eq!(network.reading_count(), 0);
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut network = SensorNetwork::<DefaultConfig>::new(1);

        assert_eq!(network.element_count(), 0);
        assert!(network.can_add_element());

        network
            .add_reading(
                42,
                SensorType::Temperature,
                2300,
                ReadingQuality::Good,
                1000,
                1,
            )
            .unwrap();
        assert_eq!(network.element_count(), 1);
        assert!(network.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut network1 = SensorNetwork::<DefaultConfig>::new(1);
        let network2 = SensorNetwork::<DefaultConfig>::new(2);

        assert!(network1.merge_bounded(&network2).is_ok());
        assert!(network1.validate_bounded().is_ok());
    }
}
