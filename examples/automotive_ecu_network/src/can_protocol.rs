//! CAN Protocol Layer for ECU Communication
//!
//! This module implements the CAN bus communication protocol for
//! synchronizing CRDTs between ECUs in the automotive network.

use crate::ecu_types::*;
use crdtosphere::automotive::ReliabilityLevel;
use heapless::Vec;
use core::convert::TryInto;

/// Maximum CAN frame data length
pub const CAN_MAX_DATA_LEN: usize = 8;

/// CAN frame for CRDT synchronization
#[derive(Debug, Clone)]
pub struct CANFrame {
    /// CAN message ID (11-bit standard)
    pub id: u16,
    /// Data payload (up to 8 bytes)
    pub data: Vec<u8, CAN_MAX_DATA_LEN>,
    /// Data length code
    pub dlc: u8,
}

impl CANFrame {
    /// Creates a new CAN frame
    pub fn new(id: u16, data: &[u8]) -> Result<Self, CANError> {
        if data.len() > CAN_MAX_DATA_LEN {
            return Err(CANError::DataTooLong);
        }
        
        let mut frame_data = Vec::new();
        for &byte in data {
            frame_data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        Ok(Self {
            id,
            dlc: data.len() as u8,
            data: frame_data,
        })
    }
    
    /// Gets the data as a slice
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    
    /// Checks if this is a high-priority safety message
    pub fn is_safety_critical(&self) -> bool {
        self.id >= 0x100 && self.id < 0x200
    }
}

/// CAN protocol errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CANError {
    DataTooLong,
    BufferFull,
    InvalidFrame,
    SerializationError,
    DeserializationError,
    TransmissionFailed,
    BusOff,
}

/// CAN message serializer/deserializer for CRDT data
pub struct CANCodec;

impl CANCodec {
    /// Serializes a brake command to CAN frame
    pub fn serialize_brake_command(
        source: ECUNodeId,
        brake_cmd: &BrakeCommand,
        timestamp: u64
    ) -> Result<CANFrame, CANError> {
        let mut data: Vec<u8, 8> = Vec::new();
        
        // Byte 0: Source ECU ID
        data.push(source as u8).map_err(|_| CANError::BufferFull)?;
        
        // Byte 1: Brake pressure
        data.push(brake_cmd.pressure).map_err(|_| CANError::BufferFull)?;
        
        // Byte 2: Flags (emergency bit)
        let flags = if brake_cmd.emergency { 0x01 } else { 0x00 };
        data.push(flags).map_err(|_| CANError::BufferFull)?;
        
        // Bytes 3-6: Timestamp (32-bit, little endian)
        let timestamp_bytes = (timestamp as u32).to_le_bytes();
        for &byte in &timestamp_bytes {
            data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        // Byte 7: Source ECU (redundant for safety)
        data.push(brake_cmd.source as u8).map_err(|_| CANError::BufferFull)?;
        
        CANFrame::new(CANMessageId::EmergencyBrake as u16, &data)
    }
    
    /// Deserializes a brake command from CAN frame
    pub fn deserialize_brake_command(frame: &CANFrame) -> Result<(ECUNodeId, BrakeCommand, u64), CANError> {
        if frame.data.len() < 8 {
            return Err(CANError::InvalidFrame);
        }
        
        let data = frame.data();
        
        // Parse source ECU
        let source = match data[0] {
            1 => ECUNodeId::Engine,
            2 => ECUNodeId::Brake,
            3 => ECUNodeId::Steering,
            4 => ECUNodeId::Gateway,
            _ => return Err(CANError::DeserializationError),
        };
        
        // Parse brake pressure
        let pressure = data[1];
        
        // Parse flags
        let emergency = (data[2] & 0x01) != 0;
        
        // Parse timestamp
        let timestamp_bytes: [u8; 4] = data[3..7].try_into()
            .map_err(|_| CANError::DeserializationError)?;
        let timestamp = u32::from_le_bytes(timestamp_bytes) as u64;
        
        // Verify source ECU (redundancy check)
        let source_check = match data[7] {
            1 => ECUNodeId::Engine,
            2 => ECUNodeId::Brake,
            3 => ECUNodeId::Steering,
            4 => ECUNodeId::Gateway,
            _ => return Err(CANError::DeserializationError),
        };
        
        if source != source_check {
            return Err(CANError::DeserializationError);
        }
        
        let brake_cmd = BrakeCommand::new(pressure, emergency, source);
        Ok((source, brake_cmd, timestamp))
    }
    
    /// Serializes a temperature reading to CAN frame
    pub fn serialize_temperature_reading(
        source: ECUNodeId,
        temperature: f32,
        reliability: ReliabilityLevel,
        timestamp: u64
    ) -> Result<CANFrame, CANError> {
        let mut data: Vec<u8, 8> = Vec::new();
        
        // Byte 0: Source ECU ID
        data.push(source as u8).map_err(|_| CANError::BufferFull)?;
        
        // Bytes 1-4: Temperature (IEEE 754 float, little endian)
        let temp_bytes = temperature.to_le_bytes();
        for &byte in &temp_bytes {
            data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        // Byte 5: Reliability level
        data.push(reliability as u8).map_err(|_| CANError::BufferFull)?;
        
        // Bytes 6-7: Timestamp (16-bit, little endian)
        let timestamp_bytes = (timestamp as u16).to_le_bytes();
        for &byte in &timestamp_bytes {
            data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        CANFrame::new(CANMessageId::TemperatureFusion as u16, &data)
    }
    
    /// Deserializes a temperature reading from CAN frame
    pub fn deserialize_temperature_reading(
        frame: &CANFrame
    ) -> Result<(ECUNodeId, f32, ReliabilityLevel, u64), CANError> {
        if frame.data.len() < 8 {
            return Err(CANError::InvalidFrame);
        }
        
        let data = frame.data();
        
        // Parse source ECU
        let source = match data[0] {
            1 => ECUNodeId::Engine,
            2 => ECUNodeId::Brake,
            3 => ECUNodeId::Steering,
            4 => ECUNodeId::Gateway,
            _ => return Err(CANError::DeserializationError),
        };
        
        // Parse temperature
        let temp_bytes: [u8; 4] = data[1..5].try_into()
            .map_err(|_| CANError::DeserializationError)?;
        let temperature = f32::from_le_bytes(temp_bytes);
        
        // Parse reliability level
        let reliability = match data[5] {
            1 => ReliabilityLevel::Low,
            2 => ReliabilityLevel::Medium,
            3 => ReliabilityLevel::High,
            4 => ReliabilityLevel::UltraHigh,
            _ => return Err(CANError::DeserializationError),
        };
        
        // Parse timestamp
        let timestamp_bytes: [u8; 2] = data[6..8].try_into()
            .map_err(|_| CANError::DeserializationError)?;
        let timestamp = u16::from_le_bytes(timestamp_bytes) as u64;
        
        Ok((source, temperature, reliability, timestamp))
    }
    
    /// Serializes system configuration to CAN frame
    pub fn serialize_system_config(
        source: ECUNodeId,
        config: &SystemConfig,
        timestamp: u64
    ) -> Result<CANFrame, CANError> {
        let mut data: Vec<u8, 8> = Vec::new();
        
        // Byte 0: Source ECU ID
        data.push(source as u8).map_err(|_| CANError::BufferFull)?;
        
        // Bytes 1-2: Max RPM (little endian)
        let rpm_bytes = config.max_rpm.to_le_bytes();
        for &byte in &rpm_bytes {
            data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        // Byte 3: Temperature warning (scaled to u8)
        let temp_warning = (config.temp_warning as u8).min(255);
        data.push(temp_warning).map_err(|_| CANError::BufferFull)?;
        
        // Byte 4: Temperature critical (scaled to u8)
        let temp_critical = (config.temp_critical as u8).min(255);
        data.push(temp_critical).map_err(|_| CANError::BufferFull)?;
        
        // Byte 5: Flags (ABS and stability control)
        let mut flags = 0u8;
        if config.abs_enabled { flags |= 0x01; }
        if config.stability_control { flags |= 0x02; }
        data.push(flags).map_err(|_| CANError::BufferFull)?;
        
        // Bytes 6-7: Timestamp (16-bit, little endian)
        let timestamp_bytes = (timestamp as u16).to_le_bytes();
        for &byte in &timestamp_bytes {
            data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        CANFrame::new(CANMessageId::EngineConfig as u16, &data)
    }
    
    /// Deserializes system configuration from CAN frame
    pub fn deserialize_system_config(
        frame: &CANFrame
    ) -> Result<(ECUNodeId, SystemConfig, u64), CANError> {
        if frame.data.len() < 8 {
            return Err(CANError::InvalidFrame);
        }
        
        let data = frame.data();
        
        // Parse source ECU
        let source = match data[0] {
            1 => ECUNodeId::Engine,
            2 => ECUNodeId::Brake,
            3 => ECUNodeId::Steering,
            4 => ECUNodeId::Gateway,
            _ => return Err(CANError::DeserializationError),
        };
        
        // Parse max RPM
        let rpm_bytes: [u8; 2] = data[1..3].try_into()
            .map_err(|_| CANError::DeserializationError)?;
        let max_rpm = u16::from_le_bytes(rpm_bytes);
        
        // Parse temperature thresholds
        let temp_warning = data[3] as f32;
        let temp_critical = data[4] as f32;
        
        // Parse flags
        let flags = data[5];
        let abs_enabled = (flags & 0x01) != 0;
        let stability_control = (flags & 0x02) != 0;
        
        // Parse timestamp
        let timestamp_bytes: [u8; 2] = data[6..8].try_into()
            .map_err(|_| CANError::DeserializationError)?;
        let timestamp = u16::from_le_bytes(timestamp_bytes) as u64;
        
        let config = SystemConfig {
            max_rpm,
            temp_warning,
            temp_critical,
            abs_enabled,
            stability_control,
        };
        
        Ok((source, config, timestamp))
    }
    
    /// Serializes error count to CAN frame
    pub fn serialize_error_count(
        source: ECUNodeId,
        count: u64,
        timestamp: u64
    ) -> Result<CANFrame, CANError> {
        let mut data: Vec<u8, 8> = Vec::new();
        
        // Byte 0: Source ECU ID
        data.push(source as u8).map_err(|_| CANError::BufferFull)?;
        
        // Bytes 1-4: Error count (32-bit, little endian)
        let count_bytes = (count as u32).to_le_bytes();
        for &byte in &count_bytes {
            data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        // Bytes 5-7: Timestamp (24-bit, little endian)
        let timestamp_bytes = (timestamp as u32).to_le_bytes();
        for &byte in &timestamp_bytes[0..3] {
            data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        CANFrame::new(CANMessageId::ErrorCounts as u16, &data)
    }
    
    /// Deserializes error count from CAN frame
    pub fn deserialize_error_count(
        frame: &CANFrame
    ) -> Result<(ECUNodeId, u64, u64), CANError> {
        if frame.data.len() < 8 {
            return Err(CANError::InvalidFrame);
        }
        
        let data = frame.data();
        
        // Parse source ECU
        let source = match data[0] {
            1 => ECUNodeId::Engine,
            2 => ECUNodeId::Brake,
            3 => ECUNodeId::Steering,
            4 => ECUNodeId::Gateway,
            _ => return Err(CANError::DeserializationError),
        };
        
        // Parse error count
        let count_bytes: [u8; 4] = data[1..5].try_into()
            .map_err(|_| CANError::DeserializationError)?;
        let count = u32::from_le_bytes(count_bytes) as u64;
        
        // Parse timestamp (24-bit)
        let mut timestamp_bytes = [0u8; 4];
        timestamp_bytes[0..3].copy_from_slice(&data[5..8]);
        let timestamp = u32::from_le_bytes(timestamp_bytes) as u64;
        
        Ok((source, count, timestamp))
    }
    
    /// Creates a heartbeat frame
    pub fn create_heartbeat(source: ECUNodeId, timestamp: u64) -> Result<CANFrame, CANError> {
        let mut data: Vec<u8, 8> = Vec::new();
        
        // Byte 0: Source ECU ID
        data.push(source as u8).map_err(|_| CANError::BufferFull)?;
        
        // Bytes 1-4: Timestamp (32-bit, little endian)
        let timestamp_bytes = (timestamp as u32).to_le_bytes();
        for &byte in &timestamp_bytes {
            data.push(byte).map_err(|_| CANError::BufferFull)?;
        }
        
        // Bytes 5-7: Status flags (reserved for future use)
        for _ in 0..3 {
            data.push(0).map_err(|_| CANError::BufferFull)?;
        }
        
        CANFrame::new(CANMessageId::SystemStatus as u16, &data)
    }
}

/// CAN bus interface for ECU communication
pub trait CANBus {
    /// Transmits a CAN frame
    fn transmit(&mut self, frame: &CANFrame) -> Result<(), CANError>;
    
    /// Receives a CAN frame (non-blocking)
    fn receive(&mut self) -> Result<Option<CANFrame>, CANError>;
    
    /// Checks if the bus is available for transmission
    fn is_transmit_ready(&self) -> bool;
    
    /// Gets the current bus error state
    fn get_error_state(&self) -> CANBusState;
}

/// CAN bus error states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CANBusState {
    ErrorActive,
    ErrorPassive,
    BusOff,
}

/// Mock CAN bus implementation for simulation
pub struct MockCANBus {
    /// Transmit buffer
    tx_buffer: Vec<CANFrame, 16>,
    /// Receive buffer
    rx_buffer: Vec<CANFrame, 16>,
    /// Bus state
    state: CANBusState,
    /// Error injection flag
    inject_errors: bool,
}

impl MockCANBus {
    /// Creates a new mock CAN bus
    pub fn new() -> Self {
        Self {
            tx_buffer: Vec::new(),
            rx_buffer: Vec::new(),
            state: CANBusState::ErrorActive,
            inject_errors: false,
        }
    }
    
    /// Enables error injection for testing
    pub fn enable_error_injection(&mut self) {
        self.inject_errors = true;
    }
    
    /// Simulates receiving a frame from another ECU
    pub fn inject_frame(&mut self, frame: CANFrame) -> Result<(), CANError> {
        self.rx_buffer.push(frame).map_err(|_| CANError::BufferFull)
    }
    
    /// Gets transmitted frames for testing
    pub fn get_transmitted_frames(&self) -> &[CANFrame] {
        &self.tx_buffer
    }
    
    /// Clears all buffers
    pub fn clear_buffers(&mut self) {
        self.tx_buffer.clear();
        self.rx_buffer.clear();
    }
}

impl CANBus for MockCANBus {
    fn transmit(&mut self, frame: &CANFrame) -> Result<(), CANError> {
        if self.inject_errors {
            return Err(CANError::TransmissionFailed);
        }
        
        if self.state == CANBusState::BusOff {
            return Err(CANError::BusOff);
        }
        
        self.tx_buffer.push(frame.clone()).map_err(|_| CANError::BufferFull)
    }
    
    fn receive(&mut self) -> Result<Option<CANFrame>, CANError> {
        if self.state == CANBusState::BusOff {
            return Err(CANError::BusOff);
        }
        
        if let Some(frame) = self.rx_buffer.pop() {
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }
    
    fn is_transmit_ready(&self) -> bool {
        self.state != CANBusState::BusOff && self.tx_buffer.len() < 16
    }
    
    fn get_error_state(&self) -> CANBusState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_brake_command_serialization() {
        let source = ECUNodeId::Brake;
        let brake_cmd = BrakeCommand::emergency_brake(source);
        let timestamp = 12345;
        
        let frame = CANCodec::serialize_brake_command(source, &brake_cmd, timestamp).unwrap();
        assert_eq!(frame.id, CANMessageId::EmergencyBrake as u16);
        assert_eq!(frame.dlc, 8);
        
        let (parsed_source, parsed_cmd, parsed_timestamp) = 
            CANCodec::deserialize_brake_command(&frame).unwrap();
        
        assert_eq!(parsed_source, source);
        assert_eq!(parsed_cmd.pressure, 100);
        assert!(parsed_cmd.emergency);
        assert_eq!(parsed_timestamp, timestamp);
    }
    
    #[test]
    fn test_temperature_reading_serialization() {
        let source = ECUNodeId::Engine;
        let temperature = 85.5;
        let reliability = ReliabilityLevel::High;
        let timestamp = 54321;
        
        let frame = CANCodec::serialize_temperature_reading(
            source, temperature, reliability, timestamp
        ).unwrap();
        
        let (parsed_source, parsed_temp, parsed_reliability, parsed_timestamp) = 
            CANCodec::deserialize_temperature_reading(&frame).unwrap();
        
        assert_eq!(parsed_source, source);
        assert!((parsed_temp - temperature).abs() < 0.1);
        assert_eq!(parsed_reliability, reliability);
        assert_eq!(parsed_timestamp, timestamp);
    }
    
    #[test]
    fn test_mock_can_bus() {
        let mut can_bus = MockCANBus::new();
        
        let frame = CANFrame::new(0x123, &[1, 2, 3, 4]).unwrap();
        
        assert!(can_bus.is_transmit_ready());
        assert!(can_bus.transmit(&frame).is_ok());
        
        assert_eq!(can_bus.get_transmitted_frames().len(), 1);
        assert_eq!(can_bus.get_transmitted_frames()[0].id, 0x123);
    }
    
    #[test]
    fn test_can_frame_safety_priority() {
        let safety_frame = CANFrame::new(0x100, &[1, 2, 3]).unwrap();
        let normal_frame = CANFrame::new(0x300, &[4, 5, 6]).unwrap();
        
        assert!(safety_frame.is_safety_critical());
        assert!(!normal_frame.is_safety_critical());
    }
}
