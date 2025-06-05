//! Sensor Manager for Automotive ECU Network
//!
//! This module implements sensor data processing, fusion, and
//! outlier detection for automotive sensor networks.

use crate::ecu_types::*;
use crdtosphere::automotive::ReliabilityLevel;
use heapless::Vec;

/// Sensor manager for processing and analyzing sensor data
pub struct SensorManager {
    /// This ECU's node ID
    node_id: ECUNodeId,
    /// Temperature reading history for trend analysis
    temperature_history: [Option<TemperatureReading>; 16],
    /// Number of temperature readings stored
    temperature_count: usize,
    /// Outlier detection statistics
    outlier_stats: OutlierStats,
    /// Sensor health monitoring
    sensor_health: SensorHealth,
    /// Last sensor update timestamp
    last_sensor_update: u64,
}

/// Individual temperature reading with metadata
#[derive(Debug, Clone, Copy)]
pub struct TemperatureReading {
    /// Temperature value in Celsius
    pub temperature: f32,
    /// Reliability level of the sensor
    pub reliability: ReliabilityLevel,
    /// Timestamp of the reading
    pub timestamp: u64,
    /// Source node ID
    pub source: ECUNodeId,
    /// Quality score (0.0 to 1.0)
    pub quality: f32,
}

/// Outlier detection statistics
#[derive(Debug, Clone, Copy)]
pub struct OutlierStats {
    /// Total outliers detected
    pub outliers_detected: u64,
    /// Current mean temperature
    pub current_mean: f32,
    /// Current standard deviation
    pub current_std_dev: f32,
    /// Outlier threshold multiplier
    pub outlier_threshold: f32,
    /// Last outlier detection timestamp
    pub last_outlier_time: u64,
}

/// Sensor health monitoring
#[derive(Debug, Clone, Copy)]
pub struct SensorHealth {
    /// Number of valid readings received
    pub valid_readings: u64,
    /// Number of invalid readings received
    pub invalid_readings: u64,
    /// Number of timeout events
    pub timeout_events: u64,
    /// Average reading quality
    pub average_quality: f32,
    /// Last health check timestamp
    pub last_health_check: u64,
    /// Sensor status
    pub status: SensorStatus,
}

/// Sensor status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorStatus {
    /// Sensor is operating normally
    Healthy,
    /// Sensor has degraded performance
    Degraded,
    /// Sensor is producing unreliable data
    Unreliable,
    /// Sensor has failed
    Failed,
    /// Sensor status is unknown
    Unknown,
}

impl Default for OutlierStats {
    fn default() -> Self {
        Self {
            outliers_detected: 0,
            current_mean: 0.0,
            current_std_dev: 0.0,
            outlier_threshold: 2.5, // 2.5 standard deviations
            last_outlier_time: 0,
        }
    }
}

impl Default for SensorHealth {
    fn default() -> Self {
        Self {
            valid_readings: 0,
            invalid_readings: 0,
            timeout_events: 0,
            average_quality: 1.0,
            last_health_check: 0,
            status: SensorStatus::Unknown,
        }
    }
}

impl SensorManager {
    /// Creates a new sensor manager
    pub fn new(node_id: ECUNodeId) -> Self {
        Self {
            node_id,
            temperature_history: [const { None }; 16],
            temperature_count: 0,
            outlier_stats: OutlierStats::default(),
            sensor_health: SensorHealth::default(),
            last_sensor_update: 0,
        }
    }
    
    /// Processes a new temperature reading
    pub fn process_temperature_reading(
        &mut self,
        temperature: f32,
        reliability: ReliabilityLevel,
        timestamp: u64
    ) -> Result<(), ECUError> {
        self.last_sensor_update = timestamp;
        
        // Validate temperature reading
        if !self.is_valid_temperature(temperature) {
            self.sensor_health.invalid_readings += 1;
            return Err(ECUError::SensorError);
        }
        
        // Calculate quality score based on reliability and consistency
        let quality = self.calculate_quality_score(temperature, reliability, timestamp);
        
        // Create temperature reading
        let reading = TemperatureReading {
            temperature,
            reliability,
            timestamp,
            source: self.node_id,
            quality,
        };
        
        // Add to history
        self.add_temperature_reading(reading);
        
        // Update statistics
        self.update_statistics();
        
        // Check for outliers
        if self.is_outlier(temperature) {
            self.outlier_stats.outliers_detected += 1;
            self.outlier_stats.last_outlier_time = timestamp;
            
            // Reduce quality for outlier readings
            if let Some(last_reading) = self.get_last_temperature_reading() {
                let mut updated_reading = *last_reading;
                updated_reading.quality *= 0.5; // Penalize outliers
                self.update_last_reading(updated_reading);
            }
        }
        
        // Update sensor health
        self.update_sensor_health(timestamp);
        
        self.sensor_health.valid_readings += 1;
        Ok(())
    }
    
    /// Validates if a temperature reading is within reasonable bounds
    fn is_valid_temperature(&self, temperature: f32) -> bool {
        // Automotive temperature range: -40°C to +150°C
        temperature >= -40.0 && temperature <= 150.0 && temperature.is_finite()
    }
    
    /// Calculates quality score for a temperature reading
    fn calculate_quality_score(
        &self,
        temperature: f32,
        reliability: ReliabilityLevel,
        timestamp: u64
    ) -> f32 {
        let mut quality = reliability.weight() / 8.0; // Normalize to 0.0-1.0
        
        // Penalize readings that are too different from recent history
        if let Some(recent_mean) = self.get_recent_mean_temperature() {
            let deviation = (temperature - recent_mean).abs();
            if deviation > 10.0 { // More than 10°C difference
                quality *= 0.8;
            }
        }
        
        // Penalize old readings
        if let Some(last_reading) = self.get_last_temperature_reading() {
            let time_diff = timestamp.saturating_sub(last_reading.timestamp);
            if time_diff > 1000 { // More than 1000 cycles old
                quality *= 0.9;
            }
        }
        
        quality.min(1.0).max(0.0)
    }
    
    /// Adds a temperature reading to history
    fn add_temperature_reading(&mut self, reading: TemperatureReading) {
        if self.temperature_count < 16 {
            self.temperature_history[self.temperature_count] = Some(reading);
            self.temperature_count += 1;
        } else {
            // Shift history and add new reading
            for i in 0..15 {
                self.temperature_history[i] = self.temperature_history[i + 1];
            }
            self.temperature_history[15] = Some(reading);
        }
    }
    
    /// Updates the last temperature reading in history
    fn update_last_reading(&mut self, reading: TemperatureReading) {
        if self.temperature_count > 0 {
            let index = (self.temperature_count - 1).min(15);
            self.temperature_history[index] = Some(reading);
        }
    }
    
    /// Gets the most recent temperature reading
    fn get_last_temperature_reading(&self) -> Option<&TemperatureReading> {
        if self.temperature_count > 0 {
            let index = (self.temperature_count - 1).min(15);
            self.temperature_history[index].as_ref()
        } else {
            None
        }
    }
    
    /// Gets the mean temperature from recent readings
    fn get_recent_mean_temperature(&self) -> Option<f32> {
        if self.temperature_count == 0 {
            return None;
        }
        
        let recent_count = self.temperature_count.min(8); // Last 8 readings
        let start_index = if self.temperature_count > 8 {
            self.temperature_count - 8
        } else {
            0
        };
        
        let mut sum = 0.0;
        let mut count = 0;
        
        for i in start_index..self.temperature_count.min(16) {
            if let Some(reading) = &self.temperature_history[i] {
                sum += reading.temperature;
                count += 1;
            }
        }
        
        if count > 0 {
            Some(sum / count as f32)
        } else {
            None
        }
    }
    
    /// Updates statistical measures for outlier detection
    fn update_statistics(&mut self) {
        if self.temperature_count < 2 {
            return;
        }
        
        // Calculate mean
        let mut sum = 0.0;
        let mut count = 0;
        
        for i in 0..self.temperature_count.min(16) {
            if let Some(reading) = &self.temperature_history[i] {
                sum += reading.temperature;
                count += 1;
            }
        }
        
        if count > 0 {
            self.outlier_stats.current_mean = sum / count as f32;
            
            // Calculate standard deviation
            let mut variance_sum = 0.0;
            for i in 0..self.temperature_count.min(16) {
                if let Some(reading) = &self.temperature_history[i] {
                    let diff = reading.temperature - self.outlier_stats.current_mean;
                    variance_sum += diff * diff;
                }
            }
            
            let variance = variance_sum / count as f32;
            self.outlier_stats.current_std_dev = self.sqrt_approximation(variance);
        }
    }
    
    /// Simple square root approximation for no_std
    fn sqrt_approximation(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        
        let mut result = x;
        // Newton's method for square root
        for _ in 0..10 {
            result = 0.5 * (result + x / result);
        }
        result
    }
    
    /// Checks if a temperature reading is an outlier
    fn is_outlier(&self, temperature: f32) -> bool {
        if self.temperature_count < 3 {
            return false; // Need at least 3 readings for outlier detection
        }
        
        let deviation = (temperature - self.outlier_stats.current_mean).abs();
        let threshold = self.outlier_stats.outlier_threshold * self.outlier_stats.current_std_dev;
        
        deviation > threshold
    }
    
    /// Updates sensor health status
    fn update_sensor_health(&mut self, current_time: u64) {
        self.sensor_health.last_health_check = current_time;
        
        // Calculate average quality
        if self.temperature_count > 0 {
            let mut quality_sum = 0.0;
            let mut count = 0;
            
            for i in 0..self.temperature_count.min(16) {
                if let Some(reading) = &self.temperature_history[i] {
                    quality_sum += reading.quality;
                    count += 1;
                }
            }
            
            if count > 0 {
                self.sensor_health.average_quality = quality_sum / count as f32;
            }
        }
        
        // Determine sensor status
        let total_readings = self.sensor_health.valid_readings + self.sensor_health.invalid_readings;
        let error_rate = if total_readings > 0 {
            self.sensor_health.invalid_readings as f32 / total_readings as f32
        } else {
            0.0
        };
        
        self.sensor_health.status = if error_rate > 0.5 {
            SensorStatus::Failed
        } else if error_rate > 0.2 || self.sensor_health.average_quality < 0.3 {
            SensorStatus::Unreliable
        } else if error_rate > 0.1 || self.sensor_health.average_quality < 0.7 {
            SensorStatus::Degraded
        } else {
            SensorStatus::Healthy
        };
    }
    
    /// Checks for sensor timeout
    pub fn check_sensor_timeout(&mut self, current_time: u64, timeout_threshold: u64) -> bool {
        if current_time > self.last_sensor_update + timeout_threshold {
            self.sensor_health.timeout_events += 1;
            self.sensor_health.status = SensorStatus::Failed;
            true
        } else {
            false
        }
    }
    
    /// Gets temperature reading history
    pub fn get_temperature_history(&self) -> &[Option<TemperatureReading>] {
        &self.temperature_history[..self.temperature_count.min(16)]
    }
    
    /// Gets outlier detection statistics
    pub fn get_outlier_stats(&self) -> OutlierStats {
        self.outlier_stats
    }
    
    /// Gets sensor health information
    pub fn get_sensor_health(&self) -> SensorHealth {
        self.sensor_health
    }
    
    /// Gets the current temperature trend
    pub fn get_temperature_trend(&self) -> TemperatureTrend {
        if self.temperature_count < 3 {
            return TemperatureTrend::Unknown;
        }
        
        // Compare recent readings to determine trend
        let recent_count = self.temperature_count.min(5);
        let start_index = if self.temperature_count > 5 {
            self.temperature_count - 5
        } else {
            0
        };
        
        let mut temperatures = Vec::<f32, 5>::new();
        for i in start_index..self.temperature_count.min(16) {
            if let Some(reading) = &self.temperature_history[i] {
                temperatures.push(reading.temperature).ok();
            }
        }
        
        if temperatures.len() < 3 {
            return TemperatureTrend::Unknown;
        }
        
        // Simple trend analysis: compare first half to second half
        let mid = temperatures.len() / 2;
        let first_half_avg: f32 = temperatures[..mid].iter().sum::<f32>() / mid as f32;
        let second_half_avg: f32 = temperatures[mid..].iter().sum::<f32>() / (temperatures.len() - mid) as f32;
        
        let diff = second_half_avg - first_half_avg;
        
        if diff > 2.0 {
            TemperatureTrend::Rising
        } else if diff < -2.0 {
            TemperatureTrend::Falling
        } else {
            TemperatureTrend::Stable
        }
    }
    
    /// Performs sensor diagnostics
    pub fn run_diagnostics(&mut self, current_time: u64) -> SensorDiagnostics {
        // Check for timeout
        let has_timeout = self.check_sensor_timeout(current_time, 1000);
        
        // Check outlier rate
        let outlier_rate = if self.temperature_count > 0 {
            self.outlier_stats.outliers_detected as f32 / self.temperature_count as f32
        } else {
            0.0
        };
        
        // Check quality degradation
        let quality_degraded = self.sensor_health.average_quality < 0.5;
        
        // Check for rapid temperature changes
        let rapid_changes = self.detect_rapid_temperature_changes();
        
        SensorDiagnostics {
            node_id: self.node_id,
            sensor_status: self.sensor_health.status,
            has_timeout,
            outlier_rate,
            quality_degraded,
            rapid_changes,
            temperature_trend: self.get_temperature_trend(),
            last_reading: self.get_last_temperature_reading().copied(),
            diagnostics_timestamp: current_time,
        }
    }
    
    /// Detects rapid temperature changes that might indicate sensor issues
    fn detect_rapid_temperature_changes(&self) -> bool {
        if self.temperature_count < 3 {
            return false;
        }
        
        // Check last 3 readings for rapid changes
        let start_index = if self.temperature_count >= 3 {
            self.temperature_count - 3
        } else {
            0
        };
        
        for i in start_index..self.temperature_count.min(16) - 1 {
            if let (Some(reading1), Some(reading2)) = 
                (&self.temperature_history[i], &self.temperature_history[i + 1]) {
                let change_rate = (reading2.temperature - reading1.temperature).abs();
                let time_diff = reading2.timestamp.saturating_sub(reading1.timestamp);
                
                if time_diff > 0 && change_rate / time_diff as f32 > 0.1 {
                    return true; // More than 0.1°C per cycle
                }
            }
        }
        
        false
    }
}

/// Temperature trend enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemperatureTrend {
    Rising,
    Falling,
    Stable,
    Unknown,
}

/// Sensor diagnostics result
#[derive(Debug, Clone)]
pub struct SensorDiagnostics {
    pub node_id: ECUNodeId,
    pub sensor_status: SensorStatus,
    pub has_timeout: bool,
    pub outlier_rate: f32,
    pub quality_degraded: bool,
    pub rapid_changes: bool,
    pub temperature_trend: TemperatureTrend,
    pub last_reading: Option<TemperatureReading>,
    pub diagnostics_timestamp: u64,
}

impl core::fmt::Display for SensorStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SensorStatus::Healthy => write!(f, "HEALTHY"),
            SensorStatus::Degraded => write!(f, "DEGRADED"),
            SensorStatus::Unreliable => write!(f, "UNRELIABLE"),
            SensorStatus::Failed => write!(f, "FAILED"),
            SensorStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl core::fmt::Display for TemperatureTrend {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TemperatureTrend::Rising => write!(f, "RISING"),
            TemperatureTrend::Falling => write!(f, "FALLING"),
            TemperatureTrend::Stable => write!(f, "STABLE"),
            TemperatureTrend::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sensor_manager_creation() {
        let sensor_manager = SensorManager::new(ECUNodeId::Engine);
        assert_eq!(sensor_manager.node_id, ECUNodeId::Engine);
        assert_eq!(sensor_manager.temperature_count, 0);
        assert_eq!(sensor_manager.sensor_health.status, SensorStatus::Unknown);
    }
    
    #[test]
    fn test_temperature_processing() {
        let mut sensor_manager = SensorManager::new(ECUNodeId::Engine);
        
        let result = sensor_manager.process_temperature_reading(
            85.0, ReliabilityLevel::High, 1000
        );
        
        assert!(result.is_ok());
        assert_eq!(sensor_manager.temperature_count, 1);
        assert_eq!(sensor_manager.sensor_health.valid_readings, 1);
        
        let last_reading = sensor_manager.get_last_temperature_reading().unwrap();
        assert_eq!(last_reading.temperature, 85.0);
        assert_eq!(last_reading.reliability, ReliabilityLevel::High);
    }
    
    #[test]
    fn test_invalid_temperature_rejection() {
        let mut sensor_manager = SensorManager::new(ECUNodeId::Engine);
        
        // Test temperature out of range
        let result = sensor_manager.process_temperature_reading(
            200.0, ReliabilityLevel::High, 1000
        );
        
        assert!(result.is_err());
        assert_eq!(sensor_manager.sensor_health.invalid_readings, 1);
        assert_eq!(sensor_manager.temperature_count, 0);
    }
    
    #[test]
    fn test_outlier_detection() {
        let mut sensor_manager = SensorManager::new(ECUNodeId::Engine);
        
        // Add normal readings
        for i in 0..5 {
            sensor_manager.process_temperature_reading(
                80.0 + i as f32, ReliabilityLevel::High, 1000 + i
            ).unwrap();
        }
        
        // Add outlier
        sensor_manager.process_temperature_reading(
            150.0, ReliabilityLevel::High, 1006
        ).unwrap();
        
        assert!(sensor_manager.outlier_stats.outliers_detected > 0);
    }
    
    #[test]
    fn test_temperature_trend_detection() {
        let mut sensor_manager = SensorManager::new(ECUNodeId::Engine);
        
        // Add rising temperature readings
        for i in 0..5 {
            sensor_manager.process_temperature_reading(
                70.0 + (i as f32 * 5.0), ReliabilityLevel::High, 1000 + i
            ).unwrap();
        }
        
        let trend = sensor_manager.get_temperature_trend();
        assert_eq!(trend, TemperatureTrend::Rising);
    }
    
    #[test]
    fn test_sensor_health_monitoring() {
        let mut sensor_manager = SensorManager::new(ECUNodeId::Engine);
        
        // Add some valid readings
        for i in 0..10 {
            sensor_manager.process_temperature_reading(
                80.0, ReliabilityLevel::High, 1000 + i
            ).unwrap();
        }
        
        // Add some invalid readings
        for i in 0..3 {
            let _ = sensor_manager.process_temperature_reading(
                300.0, ReliabilityLevel::Low, 1010 + i
            );
        }
        
        let health = sensor_manager.get_sensor_health();
        assert_eq!(health.valid_readings, 10);
        assert_eq!(health.invalid_readings, 3);
        
        // Should be degraded due to error rate
        assert!(health.status == SensorStatus::Degraded || health.status == SensorStatus::Unreliable);
    }
    
    #[test]
    fn test_sensor_timeout_detection() {
        let mut sensor_manager = SensorManager::new(ECUNodeId::Engine);
        
        // Add a reading
        sensor_manager.process_temperature_reading(
            80.0, ReliabilityLevel::High, 1000
        ).unwrap();
        
        // Check for timeout after long period
        let has_timeout = sensor_manager.check_sensor_timeout(3000, 1000);
        assert!(has_timeout);
        assert_eq!(sensor_manager.sensor_health.status, SensorStatus::Failed);
    }
}
