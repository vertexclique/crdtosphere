//! Memory-Mapped CRDT Processing Module
//!
//! This module provides memory-mapped I/O integration for CRDT operations
//! in the automotive ECU network simulation. It handles reading inputs from
//! scenario-injected data and writing CRDT processing results to output regions.

use core::ptr;
use crdtosphere::automotive::{ReliabilityLevel, SensorFusion, SensorReading};
use crdtosphere::automotive::safety::{SafetyLevel, ASILLevel};
use crdtosphere::registers::LWWRegister;
use crdtosphere::counters::GCounter;
use crdtosphere::memory::DefaultConfig;
use crate::{ECUNodeId, SystemConfig, ECUError};

/// Memory region addresses for CRDT I/O
pub mod memory_regions {
    // Input regions (written by scenarios)
    pub const TEMP_INPUT: u32 = 0x50000000;
    pub const EMERGENCY_INPUT: u32 = 0x50000100;
    pub const CONFIG_INPUT: u32 = 0x50000200;
    pub const ERROR_INPUT: u32 = 0x50000300;
    
    // Output regions (written by CRDT processing)
    pub const TEMP_OUTPUT: u32 = 0x50001000;
    pub const EMERGENCY_OUTPUT: u32 = 0x50001100;
    pub const CONFIG_OUTPUT: u32 = 0x50001200;
    pub const ERROR_OUTPUT: u32 = 0x50001300;
    
    // CAN communication regions
    pub const CAN_TX: u32 = 0x50002000;
    pub const CAN_RX: u32 = 0x50003000;
}

/// CRDT state for memory-mapped processing
pub struct MemoryMappedCRDTState {
    pub temperature_fusion: SensorFusion<f32, DefaultConfig>,
    pub system_config: LWWRegister<SystemConfig, DefaultConfig>,
    pub error_counter: GCounter<DefaultConfig>,
    pub emergency_state: u32,
    pub node_id: ECUNodeId,
    pub reliability_level: ReliabilityLevel,
}

impl MemoryMappedCRDTState {
    /// Creates a new CRDT state for the given ECU
    pub fn new(node_id: ECUNodeId) -> Self {
        let reliability_level = match node_id {
            ECUNodeId::Engine => ReliabilityLevel::High,
            ECUNodeId::Brake => ReliabilityLevel::Medium,
            ECUNodeId::Steering => ReliabilityLevel::Medium,
            ECUNodeId::Gateway => ReliabilityLevel::Low,
        };

        Self {
            temperature_fusion: SensorFusion::new(node_id as u8),
            system_config: LWWRegister::new(node_id as u8),
            error_counter: GCounter::new(node_id as u8),
            emergency_state: 0,
            node_id,
            reliability_level,
        }
    }

    /// Reads all inputs from memory-mapped regions
    pub fn read_inputs(&self) -> CRDTInputs {
        unsafe {
            CRDTInputs {
                temperature: ptr::read_volatile(memory_regions::TEMP_INPUT as *const f32),
                emergency_condition: ptr::read_volatile(memory_regions::EMERGENCY_INPUT as *const u32),
                config_data: ptr::read_volatile(memory_regions::CONFIG_INPUT as *const u32),
                error_condition: ptr::read_volatile(memory_regions::ERROR_INPUT as *const u32),
            }
        }
    }

    /// Processes temperature fusion CRDT
    pub fn process_temperature_fusion(&mut self, raw_temp: f32, timestamp: u64) -> Result<(), ECUError> {
        // Only process if we have valid temperature data
        if raw_temp > 0.0 && raw_temp < 200.0 {
            // Create sensor reading with proper safety level
            let safety_level = match self.node_id {
                ECUNodeId::Engine => SafetyLevel::automotive(ASILLevel::AsilD),
                ECUNodeId::Brake => SafetyLevel::automotive(ASILLevel::AsilD),
                ECUNodeId::Steering => SafetyLevel::automotive(ASILLevel::AsilC),
                ECUNodeId::Gateway => SafetyLevel::automotive(ASILLevel::AsilB),
            };

            let reading = SensorReading::new(
                raw_temp,
                timestamp,
                self.node_id as u8,
                self.reliability_level,
                safety_level,
            );

            // Add reading to sensor fusion CRDT
            self.temperature_fusion.add_reading(reading)
                .map_err(ECUError::from)?;
        }

        // Get fused result
        let fused_temp = self.temperature_fusion.fused_value()
            .unwrap_or(raw_temp); // Fallback to raw temp if fusion fails

        // Write to output region
        unsafe {
            ptr::write_volatile(memory_regions::TEMP_OUTPUT as *mut f32, fused_temp);
        }

        Ok(())
    }

    /// Processes emergency coordination CRDT
    pub fn process_emergency_coordination(&mut self, emergency_input: u32, timestamp: u64) -> Result<(), ECUError> {
        // Check for emergency conditions
        let mut emergency_active = self.emergency_state != 0;

        // Emergency triggered by input
        if emergency_input != 0 {
            emergency_active = true;
            self.emergency_state = 0x00640100; // Emergency brake command
        }

        // Emergency triggered by critical temperature
        let current_temp = unsafe {
            ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32)
        };
        if current_temp > 110.0 {
            emergency_active = true;
            self.emergency_state = 0x00640100;
        }

        // Write emergency state to output region
        let emergency_value = if emergency_active { self.emergency_state } else { 0x00000000 };
        unsafe {
            ptr::write_volatile(memory_regions::EMERGENCY_OUTPUT as *mut u32, emergency_value);
        }

        // Write emergency flag
        let emergency_flag = if emergency_active { 0x00000001 } else { 0x00000000 };
        unsafe {
            ptr::write_volatile((memory_regions::EMERGENCY_OUTPUT + 4) as *mut u32, emergency_flag);
        }

        Ok(())
    }

    /// Processes configuration synchronization CRDT
    pub fn process_configuration_sync(&mut self, config_input: u32, timestamp: u64) -> Result<(), ECUError> {
        // Check for configuration updates
        if config_input != 0 {
            // Decode configuration from input
            let new_config = SystemConfig {
                max_rpm: ((config_input >> 16) & 0xFFFF) as u16,
                temp_warning: 95.0,
                temp_critical: 110.0,
                abs_enabled: (config_input & 0x1) != 0,
                stability_control: (config_input & 0x2) != 0,
            };

            self.system_config.set(new_config, timestamp)
                .map_err(ECUError::from)?;
        }

        // Get current configuration
        let current_config = self.system_config.get().cloned().unwrap_or_default();
        
        // Encode configuration for output
        let config_output = ((current_config.max_rpm as u32) << 16) |
                           (if current_config.abs_enabled { 0x1 } else { 0x0 }) |
                           (if current_config.stability_control { 0x2 } else { 0x0 });

        // Write configuration to output region
        unsafe {
            ptr::write_volatile(memory_regions::CONFIG_OUTPUT as *mut u32, config_output);
        }

        // Write timestamp
        unsafe {
            ptr::write_volatile((memory_regions::CONFIG_OUTPUT + 4) as *mut u32, timestamp as u32);
        }

        Ok(())
    }

    /// Updates error counters CRDT
    pub fn update_error_counters(&mut self, error_condition: u32) -> Result<(), ECUError> {
        // Increment error counter based on conditions
        if error_condition != 0 {
            self.error_counter.increment(1)
                .map_err(ECUError::from)?;
        }

        // Check for other error conditions
        let current_temp = unsafe {
            ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32)
        };
        
        // Temperature sensor fault detection
        if current_temp <= 0.0 || current_temp > 150.0 {
            self.error_counter.increment(1)
                .map_err(ECUError::from)?;
        }

        // Write error count to output region
        let error_count = self.error_counter.value();
        unsafe {
            ptr::write_volatile(memory_regions::ERROR_OUTPUT as *mut u32, error_count as u32);
        }

        Ok(())
    }

    /// Exchanges CRDT state via CAN bus simulation
    pub fn exchange_crdt_state_via_can(&mut self, timestamp: u64) -> Result<(), ECUError> {
        // Send our CRDT state to CAN TX region
        self.send_temperature_fusion_state(timestamp)?;
        self.send_emergency_state(timestamp)?;
        self.send_config_state(timestamp)?;
        self.send_error_count(timestamp)?;

        // Receive and merge CRDT states from CAN RX region
        self.receive_and_merge_states()?;

        Ok(())
    }

    /// Sends temperature fusion state to CAN TX
    fn send_temperature_fusion_state(&self, timestamp: u64) -> Result<(), ECUError> {
        let fused_temp = unsafe {
            ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32)
        };

        // Encode temperature message
        let temp_message = CRDTMessage {
            node_id: self.node_id as u8,
            message_type: CRDTMessageType::TemperatureFusion as u8,
            timestamp: timestamp as u32,
            data: fused_temp.to_bits(),
        };

        // Write to CAN TX region
        unsafe {
            let can_tx_ptr = memory_regions::CAN_TX as *mut CRDTMessage;
            ptr::write_volatile(can_tx_ptr, temp_message);
        }

        Ok(())
    }

    /// Sends emergency state to CAN TX
    fn send_emergency_state(&self, timestamp: u64) -> Result<(), ECUError> {
        let emergency_state = unsafe {
            ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32)
        };

        let emergency_message = CRDTMessage {
            node_id: self.node_id as u8,
            message_type: CRDTMessageType::EmergencyCoordination as u8,
            timestamp: timestamp as u32,
            data: emergency_state,
        };

        unsafe {
            let can_tx_ptr = (memory_regions::CAN_TX + 16) as *mut CRDTMessage;
            ptr::write_volatile(can_tx_ptr, emergency_message);
        }

        Ok(())
    }

    /// Sends configuration state to CAN TX
    fn send_config_state(&self, timestamp: u64) -> Result<(), ECUError> {
        let config_state = unsafe {
            ptr::read_volatile(memory_regions::CONFIG_OUTPUT as *const u32)
        };

        let config_message = CRDTMessage {
            node_id: self.node_id as u8,
            message_type: CRDTMessageType::ConfigurationSync as u8,
            timestamp: timestamp as u32,
            data: config_state,
        };

        unsafe {
            let can_tx_ptr = (memory_regions::CAN_TX + 32) as *mut CRDTMessage;
            ptr::write_volatile(can_tx_ptr, config_message);
        }

        Ok(())
    }

    /// Sends error count to CAN TX
    fn send_error_count(&self, timestamp: u64) -> Result<(), ECUError> {
        let error_count = unsafe {
            ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32)
        };

        let error_message = CRDTMessage {
            node_id: self.node_id as u8,
            message_type: CRDTMessageType::ErrorCounting as u8,
            timestamp: timestamp as u32,
            data: error_count,
        };

        unsafe {
            let can_tx_ptr = (memory_regions::CAN_TX + 48) as *mut CRDTMessage;
            ptr::write_volatile(can_tx_ptr, error_message);
        }

        Ok(())
    }

    /// Receives and merges CRDT states from other ECUs
    fn receive_and_merge_states(&mut self) -> Result<(), ECUError> {
        // Read messages from CAN RX region
        for offset in (0..256).step_by(16) {
            unsafe {
                let can_rx_ptr = (memory_regions::CAN_RX + offset) as *const CRDTMessage;
                let message = ptr::read_volatile(can_rx_ptr);

                // Skip messages from ourselves
                if message.node_id == self.node_id as u8 {
                    continue;
                }

                // Process message based on type
                match message.message_type {
                    t if t == CRDTMessageType::TemperatureFusion as u8 => {
                        let remote_temp = f32::from_bits(message.data);
                        let remote_reliability = match message.node_id {
                            0 => ReliabilityLevel::High,   // Engine
                            1 => ReliabilityLevel::Medium, // Brake
                            2 => ReliabilityLevel::Medium, // Steering
                            3 => ReliabilityLevel::Low,    // Gateway
                            _ => ReliabilityLevel::Low,
                        };
                        
                        // Create sensor reading for remote temperature
                        let safety_level = match message.node_id {
                            0 => SafetyLevel::automotive(ASILLevel::AsilD), // Engine
                            1 => SafetyLevel::automotive(ASILLevel::AsilD), // Brake
                            2 => SafetyLevel::automotive(ASILLevel::AsilC), // Steering
                            3 => SafetyLevel::automotive(ASILLevel::AsilB), // Gateway
                            _ => SafetyLevel::automotive(ASILLevel::AsilB),
                        };
                        
                        let remote_reading = SensorReading::new(
                            remote_temp,
                            message.timestamp as u64,
                            message.node_id,
                            remote_reliability,
                            safety_level,
                        );
                        
                        // Merge remote temperature reading
                        let _ = self.temperature_fusion.add_reading(remote_reading);
                    }
                    t if t == CRDTMessageType::EmergencyCoordination as u8 => {
                        // Merge emergency state
                        if message.data != 0 {
                            self.emergency_state = message.data;
                        }
                    }
                    t if t == CRDTMessageType::ConfigurationSync as u8 => {
                        // Merge configuration (LWW semantics)
                        let remote_config = SystemConfig {
                            max_rpm: ((message.data >> 16) & 0xFFFF) as u16,
                            temp_warning: 95.0,
                            temp_critical: 110.0,
                            abs_enabled: (message.data & 0x1) != 0,
                            stability_control: (message.data & 0x2) != 0,
                        };
                        
                        let _ = self.system_config.set(remote_config, message.timestamp as u64);
                    }
                    t if t == CRDTMessageType::ErrorCounting as u8 => {
                        // Merge error counts (G-Counter semantics)
                        // In a real implementation, we'd merge the vector clock
                        // For simulation, we'll just track the maximum
                        let current_errors = self.error_counter.value();
                        if message.data > current_errors as u32 {
                            // This is a simplified merge - real G-Counter would be more complex
                            for _ in current_errors..(message.data as u64) {
                                let _ = self.error_counter.increment(1);
                            }
                        }
                    }
                    _ => {} // Unknown message type
                }
            }
        }

        Ok(())
    }

    /// Writes all CRDT results to output regions
    pub fn write_outputs(&self) -> Result<(), ECUError> {
        // Temperature fusion result
        let fused_temp = self.temperature_fusion.fused_value()
            .unwrap_or(0.0);
        unsafe {
            ptr::write_volatile(memory_regions::TEMP_OUTPUT as *mut f32, fused_temp);
        }

        // Emergency state
        unsafe {
            ptr::write_volatile(memory_regions::EMERGENCY_OUTPUT as *mut u32, self.emergency_state);
        }

        // Configuration
        if let Some(config) = self.system_config.get() {
            let config_output = ((config.max_rpm as u32) << 16) |
                               (if config.abs_enabled { 0x1 } else { 0x0 }) |
                               (if config.stability_control { 0x2 } else { 0x0 });
            unsafe {
                ptr::write_volatile(memory_regions::CONFIG_OUTPUT as *mut u32, config_output);
            }
        }

        // Error count
        let error_count = self.error_counter.value();
        unsafe {
            ptr::write_volatile(memory_regions::ERROR_OUTPUT as *mut u32, error_count as u32);
        }

        Ok(())
    }
}

/// Input data structure for CRDT processing
#[derive(Debug, Clone, Copy)]
pub struct CRDTInputs {
    pub temperature: f32,
    pub emergency_condition: u32,
    pub config_data: u32,
    pub error_condition: u32,
}

/// CAN message structure for CRDT synchronization
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CRDTMessage {
    node_id: u8,
    message_type: u8,
    timestamp: u32,
    data: u32,
}

/// CRDT message types
#[repr(u8)]
enum CRDTMessageType {
    TemperatureFusion = 1,
    EmergencyCoordination = 2,
    ConfigurationSync = 3,
    ErrorCounting = 4,
}

/// Main CRDT processing function for ECUs
pub fn run_crdt_processing_cycle(
    state: &mut MemoryMappedCRDTState,
    timestamp: u64
) -> Result<(), ECUError> {
    // Read inputs from memory-mapped regions
    let inputs = state.read_inputs();

    // Process each CRDT type
    state.process_temperature_fusion(inputs.temperature, timestamp)?;
    state.process_emergency_coordination(inputs.emergency_condition, timestamp)?;
    state.process_configuration_sync(inputs.config_data, timestamp)?;
    state.update_error_counters(inputs.error_condition)?;

    // Exchange CRDT state via CAN bus
    state.exchange_crdt_state_via_can(timestamp)?;

    // Write all outputs
    state.write_outputs()?;

    Ok(())
}
