//! Safety Manager for Automotive ECU Network
//!
//! This module implements safety monitoring and emergency response
//! coordination according to ISO 26262 automotive safety standards.

use crate::ecu_types::*;
use crdtosphere::automotive::{SafetyLevel, ASILLevel};

/// Safety manager for coordinating emergency responses
pub struct SafetyManager {
    /// This ECU's node ID
    node_id: ECUNodeId,
    /// This ECU's safety level
    safety_level: SafetyLevel,
    /// Emergency brake activation history
    emergency_history: [Option<EmergencyEvent>; 8],
    /// Number of emergency events recorded
    emergency_count: usize,
    /// Last safety check timestamp
    last_safety_check: u64,
    /// Safety violation counter
    safety_violations: u64,
}

/// Emergency event record
#[derive(Debug, Clone, Copy)]
pub struct EmergencyEvent {
    /// Timestamp of the emergency
    pub timestamp: u64,
    /// Source ECU that triggered the emergency
    pub source: ECUNodeId,
    /// Type of emergency
    pub event_type: EmergencyType,
    /// Brake command associated with the emergency
    pub brake_command: BrakeCommand,
}

/// Types of emergency events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmergencyType {
    /// Critical temperature detected
    CriticalTemperature,
    /// Manual emergency brake activation
    ManualEmergencyBrake,
    /// System fault detected
    SystemFault,
    /// External emergency signal
    ExternalEmergency,
    /// Sensor failure detected
    SensorFailure,
}

impl SafetyManager {
    /// Creates a new safety manager
    pub fn new(node_id: ECUNodeId, safety_level: SafetyLevel) -> Self {
        Self {
            node_id,
            safety_level,
            emergency_history: [const { None }; 8],
            emergency_count: 0,
            last_safety_check: 0,
            safety_violations: 0,
        }
    }
    
    /// Handles an emergency brake command
    pub fn handle_emergency_brake(
        &mut self,
        brake_cmd: BrakeCommand,
        timestamp: u64
    ) -> Result<(), ECUError> {
        // Determine emergency type
        let event_type = if brake_cmd.emergency {
            EmergencyType::ManualEmergencyBrake
        } else {
            EmergencyType::SystemFault
        };
        
        // Record the emergency event
        let emergency_event = EmergencyEvent {
            timestamp,
            source: brake_cmd.source,
            event_type,
            brake_command: brake_cmd,
        };
        
        self.record_emergency_event(emergency_event);
        
        // Validate safety level authorization
        self.validate_emergency_authorization(brake_cmd.source)?;
        
        // Execute safety response based on our ECU type
        self.execute_safety_response(emergency_event)?;
        
        Ok(())
    }
    
    /// Records an emergency event in history
    fn record_emergency_event(&mut self, event: EmergencyEvent) {
        if self.emergency_count < 8 {
            self.emergency_history[self.emergency_count] = Some(event);
            self.emergency_count += 1;
        } else {
            // Shift history and add new event
            for i in 0..7 {
                self.emergency_history[i] = self.emergency_history[i + 1];
            }
            self.emergency_history[7] = Some(event);
        }
    }
    
    /// Validates that the source ECU has authorization for emergency commands
    fn validate_emergency_authorization(&mut self, source: ECUNodeId) -> Result<(), ECUError> {
        let source_safety_level = source.safety_level();
        
        // Only ASIL-C and above can trigger emergency brakes
        if source_safety_level.priority() < ASILLevel::AsilC as u8 {
            self.safety_violations += 1;
            return Err(ECUError::SafetyViolation);
        }
        
        Ok(())
    }
    
    /// Executes safety response based on ECU type and emergency
    fn execute_safety_response(&mut self, event: EmergencyEvent) -> Result<(), ECUError> {
        match self.node_id {
            ECUNodeId::Engine => {
                // Engine ECU: Reduce power, prepare for shutdown
                self.handle_engine_emergency_response(event)
            }
            ECUNodeId::Brake => {
                // Brake ECU: Apply emergency braking, activate ABS
                self.handle_brake_emergency_response(event)
            }
            ECUNodeId::Steering => {
                // Steering ECU: Stabilize steering, activate stability control
                self.handle_steering_emergency_response(event)
            }
            ECUNodeId::Gateway => {
                // Gateway ECU: Coordinate emergency response, log events
                self.handle_gateway_emergency_response(event)
            }
        }
    }
    
    /// Engine ECU emergency response
    fn handle_engine_emergency_response(&mut self, event: EmergencyEvent) -> Result<(), ECUError> {
        match event.event_type {
            EmergencyType::CriticalTemperature => {
                // Reduce engine power to prevent damage
                // In a real system, this would control fuel injection, ignition timing, etc.
            }
            EmergencyType::ManualEmergencyBrake => {
                // Cut engine power to assist braking
                // Activate engine braking if available
            }
            EmergencyType::SystemFault => {
                // Enter safe mode, limit engine operation
            }
            _ => {
                // Default emergency response
            }
        }
        Ok(())
    }
    
    /// Brake ECU emergency response
    fn handle_brake_emergency_response(&mut self, event: EmergencyEvent) -> Result<(), ECUError> {
        match event.event_type {
            EmergencyType::ManualEmergencyBrake | EmergencyType::CriticalTemperature => {
                // Apply maximum safe braking force
                // Activate ABS to prevent wheel lockup
                // Monitor brake temperature to prevent fade
            }
            EmergencyType::SystemFault => {
                // Switch to backup braking system if available
                // Ensure basic braking functionality
            }
            _ => {
                // Prepare for emergency braking
            }
        }
        Ok(())
    }
    
    /// Steering ECU emergency response
    fn handle_steering_emergency_response(&mut self, event: EmergencyEvent) -> Result<(), ECUError> {
        match event.event_type {
            EmergencyType::ManualEmergencyBrake => {
                // Activate stability control
                // Prevent steering lockup during emergency braking
                // Maintain vehicle stability
            }
            EmergencyType::SystemFault => {
                // Switch to manual steering mode if possible
                // Ensure basic steering functionality
            }
            _ => {
                // Prepare stability systems
            }
        }
        Ok(())
    }
    
    /// Gateway ECU emergency response
    fn handle_gateway_emergency_response(&mut self, event: EmergencyEvent) -> Result<(), ECUError> {
        match event.event_type {
            EmergencyType::ManualEmergencyBrake => {
                // Coordinate emergency response across all ECUs
                // Log emergency event for diagnostics
                // Notify external systems if connected
            }
            EmergencyType::SystemFault => {
                // Isolate faulty systems
                // Maintain communication between healthy ECUs
            }
            _ => {
                // Monitor and coordinate response
            }
        }
        Ok(())
    }
    
    /// Checks overall safety conditions
    pub fn check_safety_conditions(
        &mut self,
        ecu_state: &ECUState,
        current_time: u64
    ) -> Result<(), ECUError> {
        self.last_safety_check = current_time;
        
        // Check for temperature-based safety violations
        if let Some(temp) = ecu_state.get_safety_critical_temperature() {
            if let Some(config) = ecu_state.get_system_config() {
                if temp > config.temp_critical {
                    self.safety_violations += 1;
                    
                    // Trigger emergency response for critical temperature
                    let emergency_event = EmergencyEvent {
                        timestamp: current_time,
                        source: self.node_id,
                        event_type: EmergencyType::CriticalTemperature,
                        brake_command: BrakeCommand::emergency_brake(self.node_id),
                    };
                    
                    self.record_emergency_event(emergency_event);
                    self.execute_safety_response(emergency_event)?;
                }
            }
        }
        
        // Check for excessive error counts
        let error_count = ecu_state.get_error_count();
        if error_count > 100 {  // Threshold for system fault
            self.safety_violations += 1;
            
            let emergency_event = EmergencyEvent {
                timestamp: current_time,
                source: self.node_id,
                event_type: EmergencyType::SystemFault,
                brake_command: BrakeCommand::new(50, false, self.node_id), // Partial brake
            };
            
            self.record_emergency_event(emergency_event);
            self.execute_safety_response(emergency_event)?;
        }
        
        // Check for sensor failures (no recent readings)
        if current_time > self.last_safety_check + 1000 {  // 1000 cycles without update
            if ecu_state.temperature_fusion.is_empty() {
                self.safety_violations += 1;
                
                let emergency_event = EmergencyEvent {
                    timestamp: current_time,
                    source: self.node_id,
                    event_type: EmergencyType::SensorFailure,
                    brake_command: BrakeCommand::new(25, false, self.node_id), // Light brake
                };
                
                self.record_emergency_event(emergency_event);
                self.execute_safety_response(emergency_event)?;
            }
        }
        
        Ok(())
    }
    
    /// Gets emergency event history
    pub fn get_emergency_history(&self) -> &[Option<EmergencyEvent>] {
        &self.emergency_history[..self.emergency_count.min(8)]
    }
    
    /// Gets total safety violations count
    pub fn get_safety_violations(&self) -> u64 {
        self.safety_violations
    }
    
    /// Gets the most recent emergency event
    pub fn get_last_emergency(&self) -> Option<&EmergencyEvent> {
        if self.emergency_count > 0 {
            let index = (self.emergency_count - 1).min(7);
            self.emergency_history[index].as_ref()
        } else {
            None
        }
    }
    
    /// Checks if the system is currently in emergency state
    pub fn is_emergency_active(&self, current_time: u64) -> bool {
        if let Some(last_emergency) = self.get_last_emergency() {
            // Emergency is active for 5000 cycles after activation
            current_time < last_emergency.timestamp + 5000
        } else {
            false
        }
    }
    
    /// Validates safety level hierarchy
    pub fn validate_safety_hierarchy(&self, other_safety_level: SafetyLevel) -> bool {
        // Higher safety levels can override lower ones
        other_safety_level >= self.safety_level
    }
    
    /// Gets safety manager status for diagnostics
    pub fn get_safety_status(&self, current_time: u64) -> SafetyStatus {
        SafetyStatus {
            node_id: self.node_id,
            safety_level: self.safety_level,
            emergency_active: self.is_emergency_active(current_time),
            safety_violations: self.safety_violations,
            emergency_count: self.emergency_count,
            last_emergency: self.get_last_emergency().copied(),
            last_safety_check: self.last_safety_check,
        }
    }
}

/// Safety status for monitoring
#[derive(Debug, Clone)]
pub struct SafetyStatus {
    pub node_id: ECUNodeId,
    pub safety_level: SafetyLevel,
    pub emergency_active: bool,
    pub safety_violations: u64,
    pub emergency_count: usize,
    pub last_emergency: Option<EmergencyEvent>,
    pub last_safety_check: u64,
}

impl core::fmt::Display for EmergencyType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EmergencyType::CriticalTemperature => write!(f, "CRITICAL_TEMP"),
            EmergencyType::ManualEmergencyBrake => write!(f, "MANUAL_BRAKE"),
            EmergencyType::SystemFault => write!(f, "SYSTEM_FAULT"),
            EmergencyType::ExternalEmergency => write!(f, "EXTERNAL"),
            EmergencyType::SensorFailure => write!(f, "SENSOR_FAIL"),
        }
    }
}

impl core::fmt::Display for EmergencyEvent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Emergency[{}@{}: {} from {}]",
               self.event_type, self.timestamp, self.brake_command, self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crdtosphere::automotive::ASILLevel;
    
    #[test]
    fn test_safety_manager_creation() {
        let safety_manager = SafetyManager::new(
            ECUNodeId::Engine,
            SafetyLevel::automotive(ASILLevel::AsilD)
        );
        
        assert_eq!(safety_manager.node_id, ECUNodeId::Engine);
        assert_eq!(safety_manager.emergency_count, 0);
        assert_eq!(safety_manager.safety_violations, 0);
    }
    
    #[test]
    fn test_emergency_brake_handling() {
        let mut safety_manager = SafetyManager::new(
            ECUNodeId::Brake,
            SafetyLevel::automotive(ASILLevel::AsilD)
        );
        
        let brake_cmd = BrakeCommand::emergency_brake(ECUNodeId::Engine);
        let result = safety_manager.handle_emergency_brake(brake_cmd, 1000);
        
        assert!(result.is_ok());
        assert_eq!(safety_manager.emergency_count, 1);
        
        let last_emergency = safety_manager.get_last_emergency().unwrap();
        assert_eq!(last_emergency.source, ECUNodeId::Engine);
        assert_eq!(last_emergency.event_type, EmergencyType::ManualEmergencyBrake);
    }
    
    #[test]
    fn test_safety_authorization() {
        let mut safety_manager = SafetyManager::new(
            ECUNodeId::Gateway,
            SafetyLevel::automotive(ASILLevel::AsilB)
        );
        
        // Gateway (ASIL-B) should not be able to authorize emergency from QM source
        let result = safety_manager.validate_emergency_authorization(ECUNodeId::Gateway);
        assert!(result.is_err()); // Gateway is ASIL-B, needs ASIL-C+
        
        // Engine (ASIL-D) should be able to authorize
        let result = safety_manager.validate_emergency_authorization(ECUNodeId::Engine);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_safety_hierarchy_validation() {
        let safety_manager = SafetyManager::new(
            ECUNodeId::Steering,
            SafetyLevel::automotive(ASILLevel::AsilC)
        );
        
        // ASIL-D should be able to override ASIL-C
        assert!(safety_manager.validate_safety_hierarchy(
            SafetyLevel::automotive(ASILLevel::AsilD)
        ));
        
        // ASIL-B should not be able to override ASIL-C
        assert!(!safety_manager.validate_safety_hierarchy(
            SafetyLevel::automotive(ASILLevel::AsilB)
        ));
        
        // Same level should be allowed
        assert!(safety_manager.validate_safety_hierarchy(
            SafetyLevel::automotive(ASILLevel::AsilC)
        ));
    }
    
    #[test]
    fn test_emergency_history() {
        let mut safety_manager = SafetyManager::new(
            ECUNodeId::Engine,
            SafetyLevel::automotive(ASILLevel::AsilD)
        );
        
        // Add multiple emergency events
        for i in 0..5 {
            let brake_cmd = BrakeCommand::emergency_brake(ECUNodeId::Engine);
            safety_manager.handle_emergency_brake(brake_cmd, 1000 + i).unwrap();
        }
        
        assert_eq!(safety_manager.emergency_count, 5);
        
        let history = safety_manager.get_emergency_history();
        assert_eq!(history.len(), 5);
        
        // Check that timestamps are in order
        for i in 0..4 {
            if let (Some(event1), Some(event2)) = (&history[i], &history[i + 1]) {
                assert!(event1.timestamp <= event2.timestamp);
            }
        }
    }
}
