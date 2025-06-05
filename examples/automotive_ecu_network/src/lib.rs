//! Automotive ECU Network Example
//!
//! This example demonstrates a realistic automotive ECU network using
//! CRDTosphere for distributed coordination between multiple STM32-based ECUs.
//!
//! # Architecture
//!
//! The system consists of 4 ECUs connected via CAN bus:
//! - Engine Control Unit (ASIL-D): Engine parameters, safety monitoring
//! - Brake Control Unit (ASIL-D): ABS, brake pressure, emergency braking
//! - Steering Control Unit (ASIL-C): Power steering, stability control
//! - Gateway ECU (ASIL-B): Central coordination, diagnostics
//!
//! # CRDTs Used
//!
//! - **SafetyCRDT**: Emergency brake commands with ISO 26262 safety prioritization
//! - **SensorFusion**: Multi-sensor temperature readings with reliability weighting
//! - **LWWRegister**: System configuration parameters
//! - **GCounter**: Error counters and diagnostic data
//!
//! # Real-World Features
//!
//! - CAN bus communication with proper message prioritization
//! - Safety-critical emergency brake coordination
//! - Multi-sensor temperature fusion with outlier detection
//! - Fault tolerance and graceful degradation
//! - ISO 26262 ASIL-level compliance

#![no_std]
#![no_main]

pub mod ecu_types;
pub mod can_protocol;
pub mod safety_manager;
pub mod sensor_manager;
pub mod memory_mapped_crdt;

pub use ecu_types::*;
pub use can_protocol::*;
pub use safety_manager::*;
pub use sensor_manager::*;

use crdtosphere::prelude::*;
use crdtosphere::automotive::{ASILLevel, ReliabilityLevel};
use heapless::Vec;

/// Main ECU application structure
pub struct ECUApplication<B: CANBus> {
    /// ECU state with all CRDTs
    pub state: ECUState,
    /// CAN bus interface
    pub can_bus: B,
    /// Safety manager
    pub safety_manager: SafetyManager,
    /// Sensor manager
    pub sensor_manager: SensorManager,
    /// System time
    pub system_time: SystemTime,
    /// Message processing statistics
    pub stats: ECUStatistics,
}

/// ECU performance and diagnostic statistics
#[derive(Debug, Default)]
pub struct ECUStatistics {
    /// Total CAN messages transmitted
    pub messages_transmitted: u64,
    /// Total CAN messages received
    pub messages_received: u64,
    /// Total CRDT merge operations
    pub crdt_merges: u64,
    /// Total safety violations detected
    pub safety_violations: u64,
    /// Total sensor readings processed
    pub sensor_readings: u64,
    /// Total emergency brake activations
    pub emergency_brakes: u64,
    /// CAN bus errors
    pub can_errors: u64,
    /// CRDT validation errors
    pub crdt_errors: u64,
}

impl<B: CANBus> ECUApplication<B> {
    /// Creates a new ECU application
    pub fn new(node_id: ECUNodeId, can_bus: B) -> Self {
        let safety_level = node_id.safety_level();
        let state = ECUState::new(node_id, safety_level);
        
        Self {
            state,
            can_bus,
            safety_manager: SafetyManager::new(node_id, safety_level),
            sensor_manager: SensorManager::new(node_id),
            system_time: SystemTime::new(),
            stats: ECUStatistics::default(),
        }
    }
    
    /// Main application loop - processes CAN messages and updates CRDTs
    pub fn run_cycle(&mut self) -> Result<(), ECUError> {
        // Increment system time
        self.system_time.tick();
        let current_time = self.system_time.now();
        
        // Process incoming CAN messages
        self.process_can_messages()?;
        
        // Update sensor readings (simulated)
        self.update_sensor_readings(current_time)?;
        
        // Check safety conditions
        self.check_safety_conditions(current_time)?;
        
        // Transmit periodic updates
        self.transmit_periodic_updates(current_time)?;
        
        // Validate CRDT states
        self.validate_crdt_states()?;
        
        Ok(())
    }
    
    /// Processes incoming CAN messages and updates CRDTs
    fn process_can_messages(&mut self) -> Result<(), ECUError> {
        while let Ok(Some(frame)) = self.can_bus.receive() {
            self.stats.messages_received += 1;
            
            match self.process_can_frame(&frame) {
                Ok(_) => {
                    self.stats.crdt_merges += 1;
                }
                Err(e) => {
                    self.stats.can_errors += 1;
                    // Log error but continue processing
                }
            }
        }
        Ok(())
    }
    
    /// Processes a single CAN frame and updates appropriate CRDT
    fn process_can_frame(&mut self, frame: &CANFrame) -> Result<(), ECUError> {
        let current_time = self.system_time.now();
        
        match frame.id {
            id if id == CANMessageId::EmergencyBrake as u16 => {
                let (source, brake_cmd, timestamp) = CANCodec::deserialize_brake_command(frame)
                    .map_err(|_| ECUError::DeserializationError)?;
                
                // Create temporary CRDT for merging
                let mut temp_brake_crdt = EmergencyBrakeCRDT::new(
                    source.as_node_id(),
                    source.safety_level()
                );
                temp_brake_crdt.set(brake_cmd, timestamp)?;
                
                // Merge with our state
                self.state.emergency_brake.merge(&temp_brake_crdt)?;
                
                // Check if this is an emergency brake activation
                if brake_cmd.emergency {
                    self.stats.emergency_brakes += 1;
                    self.safety_manager.handle_emergency_brake(brake_cmd, current_time)?;
                }
            }
            
            id if id == CANMessageId::TemperatureFusion as u16 => {
                let (source, temperature, reliability, timestamp) = 
                    CANCodec::deserialize_temperature_reading(frame)
                        .map_err(|_| ECUError::DeserializationError)?;
                
                // Add temperature reading to our fusion
                self.state.add_temperature_reading(temperature, timestamp, reliability)?;
                
                self.stats.sensor_readings += 1;
                
                // Check for temperature-based safety conditions
                self.sensor_manager.process_temperature_reading(
                    temperature, reliability, timestamp
                )?;
            }
            
            id if id == CANMessageId::EngineConfig as u16 => {
                let (source, config, timestamp) = CANCodec::deserialize_system_config(frame)
                    .map_err(|_| ECUError::DeserializationError)?;
                
                // Create temporary CRDT for merging
                let mut temp_config_crdt = LWWRegister::new(source.as_node_id());
                temp_config_crdt.set(config, timestamp)?;
                
                // Merge with our state
                self.state.system_config.merge(&temp_config_crdt)?;
            }
            
            id if id == CANMessageId::ErrorCounts as u16 => {
                let (source, count, timestamp) = CANCodec::deserialize_error_count(frame)
                    .map_err(|_| ECUError::DeserializationError)?;
                
                // Create temporary CRDT for merging
                let mut temp_counter_crdt = GCounter::new(source.as_node_id());
                // Note: We can't directly set a counter value, so we increment by the difference
                // This is a simplified approach for the demo
                if count > 0 {
                    temp_counter_crdt.increment(count as u32)?;
                    
                    // Merge with our state
                    self.state.error_counter.merge(&temp_counter_crdt)?;
                }
            }
            
            _ => {
                // Unknown message type - ignore
            }
        }
        
        Ok(())
    }
    
    /// Updates sensor readings (simulated for demo)
    fn update_sensor_readings(&mut self, current_time: u64) -> Result<(), ECUError> {
        // Simulate temperature readings based on ECU type
        let (base_temp, reliability) = match self.state.node_id {
            ECUNodeId::Engine => (85.0, ReliabilityLevel::High),      // Engine runs hot
            ECUNodeId::Brake => (45.0, ReliabilityLevel::Medium),     // Brake pads heat up
            ECUNodeId::Steering => (35.0, ReliabilityLevel::Medium),  // Steering is cooler
            ECUNodeId::Gateway => (40.0, ReliabilityLevel::Low),      // Gateway is ambient
        };
        
        // Add some variation (simplified simulation)
        let variation = ((current_time % 100) as f32 - 50.0) * 0.1;
        let temperature = base_temp + variation;
        
        // Add temperature reading to our fusion
        self.state.add_temperature_reading(temperature, current_time, reliability)?;
        
        self.stats.sensor_readings += 1;
        
        // Process the reading through sensor manager
        self.sensor_manager.process_temperature_reading(
            temperature, reliability, current_time
        )?;
        
        Ok(())
    }
    
    /// Checks safety conditions and triggers emergency responses
    fn check_safety_conditions(&mut self, current_time: u64) -> Result<(), ECUError> {
        // Check for critical temperature
        if self.state.is_temperature_critical() {
            self.stats.safety_violations += 1;
            
            // Trigger emergency brake if we're a safety-critical ECU
            if self.state.safety_level.priority() >= ASILLevel::AsilC as u8 {
                self.trigger_emergency_brake(current_time)?;
            }
        }
        
        // Let safety manager check conditions
        self.safety_manager.check_safety_conditions(&self.state, current_time)?;
        
        Ok(())
    }
    
    /// Transmits periodic updates to other ECUs
    fn transmit_periodic_updates(&mut self, current_time: u64) -> Result<(), ECUError> {
        // Transmit heartbeat every 100 cycles
        if current_time % 100 == 0 {
            let heartbeat = CANCodec::create_heartbeat(self.state.node_id, current_time)
                .map_err(|_| ECUError::SerializationError)?;
            
            if self.can_bus.is_transmit_ready() {
                self.can_bus.transmit(&heartbeat)
                    .map_err(|_| ECUError::TransmissionError)?;
                self.stats.messages_transmitted += 1;
            }
        }
        
        // Transmit temperature readings every 50 cycles
        if current_time % 50 == 0 {
            if let Some(temp) = self.state.get_fused_temperature() {
                let reliability = match self.state.node_id {
                    ECUNodeId::Engine => ReliabilityLevel::High,
                    ECUNodeId::Brake => ReliabilityLevel::Medium,
                    ECUNodeId::Steering => ReliabilityLevel::Medium,
                    ECUNodeId::Gateway => ReliabilityLevel::Low,
                };
                
                let temp_frame = CANCodec::serialize_temperature_reading(
                    self.state.node_id, temp, reliability, current_time
                ).map_err(|_| ECUError::SerializationError)?;
                
                if self.can_bus.is_transmit_ready() {
                    self.can_bus.transmit(&temp_frame)
                        .map_err(|_| ECUError::TransmissionError)?;
                    self.stats.messages_transmitted += 1;
                }
            }
        }
        
        // Transmit error counts every 200 cycles
        if current_time % 200 == 0 {
            let error_count = self.state.get_error_count();
            if error_count > 0 {
                let error_frame = CANCodec::serialize_error_count(
                    self.state.node_id, error_count, current_time
                ).map_err(|_| ECUError::SerializationError)?;
                
                if self.can_bus.is_transmit_ready() {
                    self.can_bus.transmit(&error_frame)
                        .map_err(|_| ECUError::TransmissionError)?;
                    self.stats.messages_transmitted += 1;
                }
            }
        }
        
        Ok(())
    }
    
    /// Validates all CRDT states
    fn validate_crdt_states(&mut self) -> Result<(), ECUError> {
        match self.state.validate_all() {
            Ok(_) => Ok(()),
            Err(e) => {
                self.stats.crdt_errors += 1;
                Err(e.into())
            }
        }
    }
    
    /// Triggers emergency brake with safety prioritization
    pub fn trigger_emergency_brake(&mut self, timestamp: u64) -> Result<(), ECUError> {
        // Set emergency brake in our state
        self.state.trigger_emergency_brake(timestamp)?;
        
        // Broadcast emergency brake command
        if let Some(brake_cmd) = self.state.get_emergency_brake() {
            let brake_frame = CANCodec::serialize_brake_command(
                self.state.node_id, brake_cmd, timestamp
            ).map_err(|_| ECUError::SerializationError)?;
            
            // Emergency messages have highest priority - send immediately
            self.can_bus.transmit(&brake_frame)
                .map_err(|_| ECUError::TransmissionError)?;
            
            self.stats.messages_transmitted += 1;
            self.stats.emergency_brakes += 1;
        }
        
        Ok(())
    }
    
    /// Gets current system status for monitoring
    pub fn get_system_status(&self) -> SystemStatus {
        SystemStatus {
            node_id: self.state.node_id,
            emergency_state: self.state.is_emergency_state(),
            temperature: self.state.get_fused_temperature(),
            safety_critical_temperature: self.state.get_safety_critical_temperature(),
            error_count: self.state.get_error_count(),
            can_bus_state: self.can_bus.get_error_state(),
            stats: self.stats.clone(),
        }
    }
}

/// System status for monitoring and debugging
#[derive(Debug, Clone)]
pub struct SystemStatus {
    pub node_id: ECUNodeId,
    pub emergency_state: bool,
    pub temperature: Option<f32>,
    pub safety_critical_temperature: Option<f32>,
    pub error_count: u64,
    pub can_bus_state: CANBusState,
    pub stats: ECUStatistics,
}

// Re-export ECUError from ecu_types to avoid duplication
pub use crate::ecu_types::ECUError;

impl Clone for ECUStatistics {
    fn clone(&self) -> Self {
        Self {
            messages_transmitted: self.messages_transmitted,
            messages_received: self.messages_received,
            crdt_merges: self.crdt_merges,
            safety_violations: self.safety_violations,
            sensor_readings: self.sensor_readings,
            emergency_brakes: self.emergency_brakes,
            can_errors: self.can_errors,
            crdt_errors: self.crdt_errors,
        }
    }
}

/// Utility functions for testing and simulation
pub mod simulation {
    use super::*;
    
    /// Creates a test scenario with multiple ECUs
    pub fn create_test_scenario() -> Vec<ECUApplication<MockCANBus>, 4> {
        let mut ecus = Vec::new();
        ecus.push(ECUApplication::new(ECUNodeId::Engine, MockCANBus::new())).ok();
        ecus.push(ECUApplication::new(ECUNodeId::Brake, MockCANBus::new())).ok();
        ecus.push(ECUApplication::new(ECUNodeId::Steering, MockCANBus::new())).ok();
        ecus.push(ECUApplication::new(ECUNodeId::Gateway, MockCANBus::new())).ok();
        ecus
    }
    
    /// Simulates emergency brake scenario
    pub fn simulate_emergency_brake_scenario(
        ecus: &mut [ECUApplication<MockCANBus>]
    ) -> Result<(), ECUError> {
        // Engine ECU detects critical temperature and triggers emergency brake
        let brake_frame = if let Some(engine_ecu) = ecus.get_mut(0) {
            engine_ecu.trigger_emergency_brake(1000)?;
            
            // Get the transmitted frame
            engine_ecu.can_bus.get_transmitted_frames().last().cloned()
        } else {
            None
        };
        
        // Simulate message propagation to other ECUs
        if let Some(frame) = brake_frame {
            for other_ecu in ecus.iter_mut().skip(1) {
                other_ecu.can_bus.inject_frame(frame.clone())
                    .map_err(|_| ECUError::TransmissionError)?;
            }
        }
        
        // Process messages in all ECUs
        for ecu in ecus.iter_mut() {
            ecu.run_cycle()?;
        }
        
        Ok(())
    }
    
    /// Simulates sensor fusion scenario
    pub fn simulate_sensor_fusion_scenario(
        ecus: &mut [ECUApplication<MockCANBus>]
    ) -> Result<(), ECUError> {
        // Each ECU generates temperature readings
        for (i, ecu) in ecus.iter_mut().enumerate() {
            let base_temp = 80.0 + (i as f32 * 5.0);
            let reliability = match i {
                0 => ReliabilityLevel::High,    // Engine
                1 => ReliabilityLevel::Medium,  // Brake
                2 => ReliabilityLevel::Medium,  // Steering
                3 => ReliabilityLevel::Low,     // Gateway
                _ => ReliabilityLevel::Low,
            };
            
            ecu.state.add_temperature_reading(base_temp, 2000 + i as u64, reliability)?;
        }
        
        // Simulate cross-ECU temperature sharing
        for i in 0..ecus.len() {
            for j in 0..ecus.len() {
                if i != j {
                    if let Some(temp) = ecus[i].state.get_fused_temperature() {
                        let reliability = match ecus[i].state.node_id {
                            ECUNodeId::Engine => ReliabilityLevel::High,
                            ECUNodeId::Brake => ReliabilityLevel::Medium,
                            ECUNodeId::Steering => ReliabilityLevel::Medium,
                            ECUNodeId::Gateway => ReliabilityLevel::Low,
                        };
                        
                        let temp_frame = CANCodec::serialize_temperature_reading(
                            ecus[i].state.node_id, temp, reliability, 2000 + i as u64
                        ).map_err(|_| ECUError::SerializationError)?;
                        
                        ecus[j].can_bus.inject_frame(temp_frame)
                            .map_err(|_| ECUError::TransmissionError)?;
                    }
                }
            }
        }
        
        // Process messages in all ECUs
        for ecu in ecus.iter_mut() {
            ecu.run_cycle()?;
        }
        
        Ok(())
    }
}
