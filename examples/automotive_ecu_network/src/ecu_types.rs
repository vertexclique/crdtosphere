//! ECU Types and Shared Functionality for Automotive Network
//!
//! This module defines the common types and functionality shared across
//! all ECUs in the automotive network demonstration.

use crdtosphere::prelude::*;
use crdtosphere::automotive::{SafetyCRDT, SensorFusion, SensorReading, ReliabilityLevel, SafetyLevel, ASILLevel};
use heapless::Vec;
use core::fmt;

/// ECU-specific error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ECUError {
    /// CRDT operation failed
    CRDTError(CRDTError),
    /// Safety violation detected
    SafetyViolation,
    /// Sensor error or invalid reading
    SensorError,
    /// Communication timeout
    CommunicationTimeout,
    /// Invalid configuration
    InvalidConfiguration,
    /// System fault
    SystemFault,
    /// Serialization error
    SerializationError,
    /// Deserialization error
    DeserializationError,
    /// Transmission error
    TransmissionError,
}

impl From<CRDTError> for ECUError {
    fn from(error: CRDTError) -> Self {
        ECUError::CRDTError(error)
    }
}

/// ECU Node IDs in the automotive network
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ECUNodeId {
    Engine = 1,
    Brake = 2,
    Steering = 3,
    Gateway = 4,
}

impl ECUNodeId {
    pub fn as_node_id(self) -> u8 {
        self as u8
    }
}

/// CAN Message IDs for different CRDT types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CANMessageId {
    // Safety-critical messages (high priority)
    EmergencyBrake = 0x100,
    EngineShutdown = 0x101,
    SteeringLock = 0x102,
    
    // Sensor fusion messages
    TemperatureFusion = 0x200,
    PressureFusion = 0x201,
    SpeedFusion = 0x202,
    
    // Configuration messages
    EngineConfig = 0x300,
    BrakeConfig = 0x301,
    SteeringConfig = 0x302,
    
    // Diagnostic messages
    ErrorCounts = 0x400,
    SystemStatus = 0x401,
}

/// Emergency brake command with safety prioritization
pub type EmergencyBrakeCRDT = SafetyCRDT<BrakeCommand, DefaultConfig>;

/// Engine temperature sensor fusion
pub type TemperatureFusionCRDT = SensorFusion<f32, DefaultConfig>;

/// System configuration register
pub type ConfigRegisterCRDT = LWWRegister<SystemConfig, DefaultConfig>;

/// Error counter
pub type ErrorCounterCRDT = GCounter<DefaultConfig>;

/// Brake command with pressure and safety level
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrakeCommand {
    /// Brake pressure percentage (0-100)
    pub pressure: u8,
    /// Emergency brake flag
    pub emergency: bool,
    /// Source ECU
    pub source: ECUNodeId,
}

impl BrakeCommand {
    pub fn new(pressure: u8, emergency: bool, source: ECUNodeId) -> Self {
        Self {
            pressure: pressure.min(100), // Clamp to 100%
            emergency,
            source,
        }
    }
    
    pub fn emergency_brake(source: ECUNodeId) -> Self {
        Self {
            pressure: 100,
            emergency: true,
            source,
        }
    }
}

/// System configuration parameters
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemConfig {
    /// Maximum engine RPM
    pub max_rpm: u16,
    /// Temperature warning threshold (Celsius)
    pub temp_warning: f32,
    /// Temperature critical threshold (Celsius)
    pub temp_critical: f32,
    /// ABS enabled flag
    pub abs_enabled: bool,
    /// Stability control enabled flag
    pub stability_control: bool,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            max_rpm: 6000,
            temp_warning: 90.0,
            temp_critical: 105.0,
            abs_enabled: true,
            stability_control: true,
        }
    }
}

/// ECU State containing all CRDTs for an ECU
pub struct ECUState {
    /// Emergency brake command coordination
    pub emergency_brake: EmergencyBrakeCRDT,
    
    /// Engine temperature sensor fusion
    pub temperature_fusion: TemperatureFusionCRDT,
    
    /// System configuration
    pub system_config: ConfigRegisterCRDT,
    
    /// Error counter
    pub error_counter: ErrorCounterCRDT,
    
    /// This ECU's node ID
    pub node_id: ECUNodeId,
    
    /// Safety level of this ECU
    pub safety_level: SafetyLevel,
}

impl ECUState {
    /// Creates a new ECU state for the given node
    pub fn new(node_id: ECUNodeId, safety_level: SafetyLevel) -> Self {
        let node_id_u8 = node_id.as_node_id();
        
        Self {
            emergency_brake: EmergencyBrakeCRDT::new(node_id_u8, safety_level),
            temperature_fusion: TemperatureFusionCRDT::new(node_id_u8),
            system_config: ConfigRegisterCRDT::new(node_id_u8),
            error_counter: ErrorCounterCRDT::new(node_id_u8),
            node_id,
            safety_level,
        }
    }
    
    /// Triggers emergency brake with maximum safety priority
    pub fn trigger_emergency_brake(&mut self, timestamp: u64) -> Result<(), CRDTError> {
        let brake_cmd = BrakeCommand::emergency_brake(self.node_id);
        self.emergency_brake.set(brake_cmd, timestamp)
    }
    
    /// Adds a temperature sensor reading
    pub fn add_temperature_reading(
        &mut self, 
        temperature: f32, 
        timestamp: u64,
        reliability: ReliabilityLevel
    ) -> Result<(), CRDTError> {
        let reading = SensorReading::new(
            temperature,
            timestamp,
            self.node_id.as_node_id(),
            reliability,
            self.safety_level,
        );
        self.temperature_fusion.add_reading(reading)
    }
    
    /// Updates system configuration
    pub fn update_config(&mut self, config: SystemConfig, timestamp: u64) -> Result<(), CRDTError> {
        self.system_config.set(config, timestamp)
    }
    
    /// Increments error counter
    pub fn increment_errors(&mut self, count: u32) -> Result<(), CRDTError> {
        self.error_counter.increment(count)
    }
    
    /// Merges state from another ECU
    pub fn merge_from(&mut self, other: &ECUState) -> Result<(), CRDTError> {
        self.emergency_brake.merge(&other.emergency_brake)?;
        self.temperature_fusion.merge(&other.temperature_fusion)?;
        self.system_config.merge(&other.system_config)?;
        self.error_counter.merge(&other.error_counter)?;
        Ok(())
    }
    
    /// Gets current emergency brake status
    pub fn get_emergency_brake(&self) -> Option<&BrakeCommand> {
        self.emergency_brake.get()
    }
    
    /// Gets fused temperature reading
    pub fn get_fused_temperature(&self) -> Option<f32> {
        self.temperature_fusion.fused_value()
    }
    
    /// Gets safety-critical temperature (ASIL-C and above only)
    pub fn get_safety_critical_temperature(&self) -> Option<f32> {
        self.temperature_fusion.safety_critical_value()
    }
    
    /// Gets current system configuration
    pub fn get_system_config(&self) -> Option<&SystemConfig> {
        self.system_config.get()
    }
    
    /// Gets total error count
    pub fn get_error_count(&self) -> u64 {
        self.error_counter.value()
    }
    
    /// Checks if system is in emergency state
    pub fn is_emergency_state(&self) -> bool {
        if let Some(brake_cmd) = self.get_emergency_brake() {
            brake_cmd.emergency
        } else {
            false
        }
    }
    
    /// Checks if temperature is critical
    pub fn is_temperature_critical(&self) -> bool {
        if let (Some(temp), Some(config)) = (self.get_fused_temperature(), self.get_system_config()) {
            temp > config.temp_critical
        } else {
            false
        }
    }
    
    /// Validates all CRDT states
    pub fn validate_all(&self) -> Result<(), CRDTError> {
        self.emergency_brake.validate()?;
        self.temperature_fusion.validate()?;
        self.system_config.validate()?;
        self.error_counter.validate()?;
        Ok(())
    }
}

/// CAN message payload for CRDT synchronization
#[derive(Debug, Clone)]
pub struct CANMessage {
    pub id: CANMessageId,
    pub source: ECUNodeId,
    pub timestamp: u64,
    pub payload: CANPayload,
}

/// CAN message payload types
#[derive(Debug, Clone)]
pub enum CANPayload {
    EmergencyBrake(BrakeCommand),
    TemperatureReading {
        temperature: f32,
        reliability: ReliabilityLevel,
    },
    SystemConfig(SystemConfig),
    ErrorCount(u64),
    Heartbeat,
}

impl CANMessage {
    pub fn emergency_brake(source: ECUNodeId, brake_cmd: BrakeCommand, timestamp: u64) -> Self {
        Self {
            id: CANMessageId::EmergencyBrake,
            source,
            timestamp,
            payload: CANPayload::EmergencyBrake(brake_cmd),
        }
    }
    
    pub fn temperature_reading(
        source: ECUNodeId, 
        temperature: f32, 
        reliability: ReliabilityLevel,
        timestamp: u64
    ) -> Self {
        Self {
            id: CANMessageId::TemperatureFusion,
            source,
            timestamp,
            payload: CANPayload::TemperatureReading { temperature, reliability },
        }
    }
    
    pub fn system_config(source: ECUNodeId, config: SystemConfig, timestamp: u64) -> Self {
        Self {
            id: CANMessageId::EngineConfig,
            source,
            timestamp,
            payload: CANPayload::SystemConfig(config),
        }
    }
    
    pub fn error_count(source: ECUNodeId, count: u64, timestamp: u64) -> Self {
        Self {
            id: CANMessageId::ErrorCounts,
            source,
            timestamp,
            payload: CANPayload::ErrorCount(count),
        }
    }
    
    pub fn heartbeat(source: ECUNodeId, timestamp: u64) -> Self {
        Self {
            id: CANMessageId::SystemStatus,
            source,
            timestamp,
            payload: CANPayload::Heartbeat,
        }
    }
}

/// System time provider for timestamps
pub struct SystemTime {
    ticks: u64,
}

impl SystemTime {
    pub fn new() -> Self {
        Self { ticks: 0 }
    }
    
    pub fn tick(&mut self) {
        self.ticks = self.ticks.wrapping_add(1);
    }
    
    pub fn now(&self) -> u64 {
        self.ticks
    }
}

/// ECU-specific safety levels
impl ECUNodeId {
    pub fn safety_level(self) -> SafetyLevel {
        match self {
            ECUNodeId::Engine => SafetyLevel::automotive(ASILLevel::AsilD),    // Engine control is critical
            ECUNodeId::Brake => SafetyLevel::automotive(ASILLevel::AsilD),     // Brake control is critical
            ECUNodeId::Steering => SafetyLevel::automotive(ASILLevel::AsilC),  // Steering is important
            ECUNodeId::Gateway => SafetyLevel::automotive(ASILLevel::AsilB),   // Gateway is coordination
        }
    }
}

/// Display implementations for debugging
impl fmt::Display for ECUNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ECUNodeId::Engine => write!(f, "ENGINE"),
            ECUNodeId::Brake => write!(f, "BRAKE"),
            ECUNodeId::Steering => write!(f, "STEERING"),
            ECUNodeId::Gateway => write!(f, "GATEWAY"),
        }
    }
}

impl fmt::Display for BrakeCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Brake[{}%{}{}]", 
               self.pressure,
               if self.emergency { " EMERGENCY" } else { "" },
               self.source)
    }
}

impl fmt::Display for SystemConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config[RPM:{} Temp:{:.1}°C/{:.1}°C ABS:{} SC:{}]",
               self.max_rpm, self.temp_warning, self.temp_critical,
               self.abs_enabled, self.stability_control)
    }
}
