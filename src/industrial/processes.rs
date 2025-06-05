//! Process Control for Industrial Systems
//!
//! This module implements CRDTs for distributed industrial process coordination,
//! enabling synchronization of manufacturing processes across control systems.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

/// Industrial process states
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ProcessState {
    /// Process is stopped/idle
    Stopped = 0,
    /// Process is starting up
    Starting = 1,
    /// Process is running normally
    Running = 2,
    /// Process is pausing
    Pausing = 3,
    /// Process is paused
    Paused = 4,
    /// Process is stopping
    Stopping = 5,
    /// Process has an error
    Error = 6,
    /// Process is in emergency stop
    Emergency = 7,
    /// Process is in maintenance mode
    Maintenance = 8,
}

impl ProcessState {
    /// Returns true if process is operational
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            ProcessState::Running | ProcessState::Starting | ProcessState::Pausing
        )
    }

    /// Returns true if process requires attention
    pub fn requires_attention(&self) -> bool {
        matches!(self, ProcessState::Error | ProcessState::Emergency)
    }

    /// Returns true if process can be started
    pub fn can_start(&self) -> bool {
        matches!(self, ProcessState::Stopped | ProcessState::Paused)
    }

    /// Returns true if process can be stopped
    pub fn can_stop(&self) -> bool {
        matches!(
            self,
            ProcessState::Running | ProcessState::Paused | ProcessState::Error
        )
    }
}

/// Control actions for processes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ControlAction {
    /// Start the process
    Start = 1,
    /// Stop the process
    Stop = 2,
    /// Pause the process
    Pause = 3,
    /// Resume the process
    Resume = 4,
    /// Emergency stop
    EmergencyStop = 5,
    /// Reset process after error
    Reset = 6,
    /// Enter maintenance mode
    Maintenance = 7,
    /// Exit maintenance mode
    ExitMaintenance = 8,
}

impl ControlAction {
    /// Returns true if this is an emergency action
    pub fn is_emergency(&self) -> bool {
        matches!(self, ControlAction::EmergencyStop)
    }

    /// Returns true if this action requires elevated privileges
    pub fn requires_privileges(&self) -> bool {
        matches!(
            self,
            ControlAction::EmergencyStop
                | ControlAction::Reset
                | ControlAction::Maintenance
                | ControlAction::ExitMaintenance
        )
    }
}

/// Individual process step information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessStep {
    /// Process identifier
    pub process_id: NodeId,
    /// Current step number
    pub step_number: u16,
    /// Process state
    pub state: ProcessState,
    /// Progress percentage (0-100)
    pub progress: u8,
    /// Set point value (scaled as i32)
    pub setpoint: i32,
    /// Current value (scaled as i32)
    pub current_value: i32,
    /// Last control action applied
    pub last_action: ControlAction,
    /// Timestamp of last update
    pub timestamp: CompactTimestamp,
    /// Controller that manages this process
    pub controller_id: NodeId,
}

impl ProcessStep {
    /// Creates a new process step
    pub fn new(
        process_id: NodeId,
        step_number: u16,
        controller_id: NodeId,
        timestamp: u64,
    ) -> Self {
        Self {
            process_id,
            step_number,
            state: ProcessState::Stopped,
            progress: 0,
            setpoint: 0,
            current_value: 0,
            last_action: ControlAction::Stop,
            timestamp: CompactTimestamp::new(timestamp),
            controller_id,
        }
    }

    /// Updates process state
    pub fn update_state(&mut self, state: ProcessState, action: ControlAction, timestamp: u64) {
        self.state = state;
        self.last_action = action;
        self.timestamp = CompactTimestamp::new(timestamp);
    }

    /// Updates process values
    pub fn update_values(
        &mut self,
        setpoint: i32,
        current_value: i32,
        progress: u8,
        timestamp: u64,
    ) {
        self.setpoint = setpoint;
        self.current_value = current_value;
        self.progress = progress;
        self.timestamp = CompactTimestamp::new(timestamp);
    }

    /// Returns true if this step should override another
    pub fn should_override(&self, other: &ProcessStep) -> bool {
        // More recent updates win
        self.timestamp > other.timestamp
    }

    /// Returns true if process is at setpoint
    pub fn is_at_setpoint(&self, tolerance: i32) -> bool {
        (self.current_value - self.setpoint).abs() <= tolerance
    }

    /// Returns error from setpoint
    pub fn error_from_setpoint(&self) -> i32 {
        self.current_value - self.setpoint
    }
}

/// Industrial Process Control CRDT
///
/// This CRDT manages distributed process coordination across industrial control systems,
/// enabling synchronization of manufacturing processes and control actions.
///
/// # Type Parameters
/// - `C`: Memory configuration
///
/// # Features
/// - Multi-process state coordination
/// - Control action synchronization
/// - Setpoint and value tracking
/// - Emergency stop coordination
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::industrial::{ProcessControl, ProcessState, ControlAction};
///
/// // Create process control
/// let mut control = ProcessControl::<DefaultConfig>::new(1); // Controller ID 1
///
/// // Register new process
/// control.register_process(
///     42,   // process ID
///     1,    // step number
///     1000  // timestamp
/// )?;
///
/// // Start process
/// control.apply_control_action(42, ControlAction::Start, 1001)?;
///
/// // Update process values
/// control.update_process_values(42, 1000, 950, 75, 1002)?; // setpoint, current, progress
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct ProcessControl<C: MemoryConfig> {
    /// Array of process steps
    processes: [Option<ProcessStep>; 64], // Support up to 64 processes
    /// Number of processes currently managed
    process_count: usize,
    /// This controller's ID
    local_controller_id: NodeId,
    /// Last update timestamp
    last_update: CompactTimestamp,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<C: MemoryConfig> ProcessControl<C> {
    /// Creates a new process control system
    ///
    /// # Arguments
    /// * `controller_id` - The ID of this controller
    ///
    /// # Returns
    /// A new process control CRDT
    pub fn new(controller_id: NodeId) -> Self {
        Self {
            processes: [const { None }; 64],
            process_count: 0,
            local_controller_id: controller_id,
            last_update: CompactTimestamp::new(0),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Registers a new process
    ///
    /// # Arguments
    /// * `process_id` - Process identifier
    /// * `step_number` - Initial step number
    /// * `timestamp` - Registration timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn register_process(
        &mut self,
        process_id: NodeId,
        step_number: u16,
        timestamp: u64,
    ) -> CRDTResult<()> {
        let process_step =
            ProcessStep::new(process_id, step_number, self.local_controller_id, timestamp);
        self.add_process_step(process_step)?;
        self.last_update = CompactTimestamp::new(timestamp);
        Ok(())
    }

    /// Applies a control action to a process
    ///
    /// # Arguments
    /// * `process_id` - Process to control
    /// * `action` - Control action to apply
    /// * `timestamp` - Action timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn apply_control_action(
        &mut self,
        process_id: NodeId,
        action: ControlAction,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(process) = self.find_process_mut(process_id) {
            let new_state = match action {
                ControlAction::Start => {
                    if process.state.can_start() {
                        ProcessState::Starting
                    } else {
                        return Err(CRDTError::InvalidOperation);
                    }
                }
                ControlAction::Stop => {
                    if process.state.can_stop() {
                        ProcessState::Stopping
                    } else {
                        return Err(CRDTError::InvalidOperation);
                    }
                }
                ControlAction::Pause => ProcessState::Pausing,
                ControlAction::Resume => ProcessState::Starting,
                ControlAction::EmergencyStop => ProcessState::Emergency,
                ControlAction::Reset => ProcessState::Stopped,
                ControlAction::Maintenance => ProcessState::Maintenance,
                ControlAction::ExitMaintenance => ProcessState::Stopped,
            };

            process.update_state(new_state, action, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Updates process values
    ///
    /// # Arguments
    /// * `process_id` - Process to update
    /// * `setpoint` - New setpoint value
    /// * `current_value` - Current process value
    /// * `progress` - Progress percentage (0-100)
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_process_values(
        &mut self,
        process_id: NodeId,
        setpoint: i32,
        current_value: i32,
        progress: u8,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(process) = self.find_process_mut(process_id) {
            process.update_values(setpoint, current_value, progress, timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Updates process state directly (for state transitions)
    ///
    /// # Arguments
    /// * `process_id` - Process to update
    /// * `state` - New process state
    /// * `timestamp` - Update timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn update_process_state(
        &mut self,
        process_id: NodeId,
        state: ProcessState,
        timestamp: u64,
    ) -> CRDTResult<()> {
        if let Some(process) = self.find_process_mut(process_id) {
            process.state = state;
            process.timestamp = CompactTimestamp::new(timestamp);
            self.last_update = CompactTimestamp::new(timestamp);
            Ok(())
        } else {
            Err(CRDTError::InvalidNodeId)
        }
    }

    /// Gets all processes
    ///
    /// # Returns
    /// Iterator over process steps
    pub fn all_processes(&self) -> impl Iterator<Item = &ProcessStep> {
        self.processes.iter().filter_map(|p| p.as_ref())
    }

    /// Gets processes by state
    ///
    /// # Arguments
    /// * `state` - Process state to filter by
    ///
    /// # Returns
    /// Iterator over processes in the specified state
    pub fn processes_by_state(&self, state: ProcessState) -> impl Iterator<Item = &ProcessStep> {
        self.all_processes().filter(move |p| p.state == state)
    }

    /// Gets running processes
    ///
    /// # Returns
    /// Iterator over running processes
    pub fn running_processes(&self) -> impl Iterator<Item = &ProcessStep> {
        self.processes_by_state(ProcessState::Running)
    }

    /// Gets processes requiring attention
    ///
    /// # Returns
    /// Iterator over processes with errors or emergency states
    pub fn processes_requiring_attention(&self) -> impl Iterator<Item = &ProcessStep> {
        self.all_processes()
            .filter(|p| p.state.requires_attention())
    }

    /// Gets processes by controller
    ///
    /// # Arguments
    /// * `controller_id` - Controller ID to filter by
    ///
    /// # Returns
    /// Iterator over processes managed by the controller
    pub fn processes_by_controller(
        &self,
        controller_id: NodeId,
    ) -> impl Iterator<Item = &ProcessStep> {
        self.all_processes()
            .filter(move |p| p.controller_id == controller_id)
    }

    /// Gets process by ID
    ///
    /// # Arguments
    /// * `process_id` - Process ID to look up
    ///
    /// # Returns
    /// Process step if found
    pub fn get_process(&self, process_id: NodeId) -> Option<&ProcessStep> {
        self.all_processes().find(|p| p.process_id == process_id)
    }

    /// Gets the number of processes
    ///
    /// # Returns
    /// Number of processes
    pub fn process_count(&self) -> usize {
        self.process_count
    }

    /// Emergency stops all processes
    ///
    /// # Arguments
    /// * `timestamp` - Emergency stop timestamp
    ///
    /// # Returns
    /// Number of processes stopped
    pub fn emergency_stop_all(&mut self, timestamp: u64) -> usize {
        let mut stopped = 0;

        for i in 0..64 {
            if let Some(ref mut process) = self.processes[i] {
                if process.state.is_operational() {
                    process.update_state(
                        ProcessState::Emergency,
                        ControlAction::EmergencyStop,
                        timestamp,
                    );
                    stopped += 1;
                }
            }
        }

        if stopped > 0 {
            self.last_update = CompactTimestamp::new(timestamp);
        }

        stopped
    }

    /// Finds a process by ID (mutable)
    fn find_process_mut(&mut self, process_id: NodeId) -> Option<&mut ProcessStep> {
        for process_opt in &mut self.processes {
            if let Some(process) = process_opt {
                if process.process_id == process_id {
                    return Some(process);
                }
            }
        }
        None
    }

    /// Adds a process step to the control system
    fn add_process_step(&mut self, process_step: ProcessStep) -> CRDTResult<()> {
        // Check for existing process
        for i in 0..64 {
            if let Some(ref mut existing) = self.processes[i] {
                if existing.process_id == process_step.process_id {
                    // Update if new step should override
                    if process_step.should_override(existing) {
                        *existing = process_step;
                    }
                    return Ok(());
                }
            } else {
                // Empty slot - add new process
                self.processes[i] = Some(process_step);
                self.process_count += 1;
                return Ok(());
            }
        }

        // If no space, try to replace oldest stopped process
        self.make_space_for_process(process_step)
    }

    /// Makes space for a new process by replacing old stopped processes
    fn make_space_for_process(&mut self, new_process: ProcessStep) -> CRDTResult<()> {
        // Find oldest stopped process to replace
        let mut oldest_idx = None;
        let mut oldest_time = u64::MAX;

        for (i, process_opt) in self.processes.iter().enumerate() {
            if let Some(process) = process_opt {
                if process.state == ProcessState::Stopped
                    && process.timestamp.as_u64() < oldest_time
                {
                    oldest_time = process.timestamp.as_u64();
                    oldest_idx = Some(i);
                }
            }
        }

        if let Some(idx) = oldest_idx {
            self.processes[idx] = Some(new_process);
            Ok(())
        } else {
            Err(CRDTError::BufferOverflow)
        }
    }

    /// Validates process control data
    ///
    /// # Returns
    /// Ok(()) if valid, error otherwise
    pub fn validate_control(&self) -> CRDTResult<()> {
        // Check process IDs are valid
        for process in self.all_processes() {
            if process.process_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
            if process.controller_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        Ok(())
    }
}

impl<C: MemoryConfig> CRDT<C> for ProcessControl<C> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Merge all processes from other
        for process in other.all_processes() {
            self.add_process_step(*process)?;
        }

        // Update timestamp to latest
        if other.last_update > self.last_update {
            self.last_update = other.last_update;
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        if self.process_count != other.process_count {
            return false;
        }

        // Check that all processes match
        for process in self.all_processes() {
            let mut found = false;
            for other_process in other.all_processes() {
                if process.process_id == other_process.process_id && process == other_process {
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
        self.validate_control()
    }

    fn state_hash(&self) -> u32 {
        let mut hash = self.local_controller_id as u32;
        for process in self.all_processes() {
            hash ^= (process.process_id as u32)
                ^ (process.timestamp.as_u64() as u32)
                ^ (process.state as u32);
        }
        hash ^= self.process_count as u32;
        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // Can always merge process controls (space is made by removing old processes)
        true
    }
}

impl<C: MemoryConfig> BoundedCRDT<C> for ProcessControl<C> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 64; // Maximum processes

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.process_count
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        // Could implement cleanup of old stopped processes
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        self.process_count < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig> RealTimeCRDT<C> for ProcessControl<C> {
    const MAX_MERGE_CYCLES: u32 = 200; // Bounded by number of processes
    const MAX_VALIDATE_CYCLES: u32 = 100;
    const MAX_SERIALIZE_CYCLES: u32 = 150;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Process control merge is bounded
        self.merge(other)
    }

    fn validate_bounded(&self) -> CRDTResult<()> {
        // Validation is bounded
        self.validate()
    }

    fn remaining_budget(&self) -> Option<u32> {
        // For industrial systems, we don't track budget
        None
    }

    fn set_budget(&mut self, _cycles: u32) {
        // For industrial systems, we don't limit budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::DefaultConfig;

    #[test]
    fn test_process_state_properties() {
        assert!(ProcessState::Running.is_operational());
        assert!(ProcessState::Starting.is_operational());
        assert!(!ProcessState::Stopped.is_operational());

        assert!(ProcessState::Error.requires_attention());
        assert!(ProcessState::Emergency.requires_attention());
        assert!(!ProcessState::Running.requires_attention());

        assert!(ProcessState::Stopped.can_start());
        assert!(ProcessState::Paused.can_start());
        assert!(!ProcessState::Running.can_start());

        assert!(ProcessState::Running.can_stop());
        assert!(ProcessState::Error.can_stop());
        assert!(!ProcessState::Stopped.can_stop());
    }

    #[test]
    fn test_control_action_properties() {
        assert!(ControlAction::EmergencyStop.is_emergency());
        assert!(!ControlAction::Start.is_emergency());

        assert!(ControlAction::EmergencyStop.requires_privileges());
        assert!(ControlAction::Reset.requires_privileges());
        assert!(!ControlAction::Start.requires_privileges());
    }

    #[test]
    fn test_process_step_creation() {
        let step = ProcessStep::new(42, 1, 1, 1000);

        assert_eq!(step.process_id, 42);
        assert_eq!(step.step_number, 1);
        assert_eq!(step.controller_id, 1);
        assert_eq!(step.state, ProcessState::Stopped);
        assert_eq!(step.progress, 0);
    }

    #[test]
    fn test_process_step_updates() {
        let mut step = ProcessStep::new(42, 1, 1, 1000);

        step.update_state(ProcessState::Running, ControlAction::Start, 1001);
        assert_eq!(step.state, ProcessState::Running);
        assert_eq!(step.last_action, ControlAction::Start);

        step.update_values(1000, 950, 75, 1002);
        assert_eq!(step.setpoint, 1000);
        assert_eq!(step.current_value, 950);
        assert_eq!(step.progress, 75);

        assert!(!step.is_at_setpoint(10)); // Error is 50, tolerance is 10
        assert!(step.is_at_setpoint(100)); // Error is 50, tolerance is 100
        assert_eq!(step.error_from_setpoint(), -50);
    }

    #[test]
    fn test_process_control_creation() {
        let control = ProcessControl::<DefaultConfig>::new(1);

        assert_eq!(control.process_count(), 0);
        assert_eq!(control.local_controller_id, 1);
    }

    #[test]
    fn test_process_registration_and_control() {
        let mut control = ProcessControl::<DefaultConfig>::new(1);

        // Register process
        control.register_process(42, 1, 1000).unwrap();
        assert_eq!(control.process_count(), 1);

        // Start process
        control
            .apply_control_action(42, ControlAction::Start, 1001)
            .unwrap();
        let process = control.get_process(42).unwrap();
        assert_eq!(process.state, ProcessState::Starting);

        // Update to running state
        control
            .update_process_state(42, ProcessState::Running, 1002)
            .unwrap();

        // Update process values
        control
            .update_process_values(42, 1000, 950, 75, 1003)
            .unwrap();

        let process = control.get_process(42).unwrap();
        assert_eq!(process.state, ProcessState::Running);
        assert_eq!(process.setpoint, 1000);
        assert_eq!(process.current_value, 950);
        assert_eq!(process.progress, 75);
    }

    #[test]
    fn test_process_control_queries() {
        let mut control = ProcessControl::<DefaultConfig>::new(1);

        // Register multiple processes
        control.register_process(1, 1, 1000).unwrap();
        control.register_process(2, 1, 1001).unwrap();
        control.register_process(3, 1, 1002).unwrap();

        // Start some processes
        control
            .apply_control_action(1, ControlAction::Start, 1003)
            .unwrap();
        control
            .update_process_state(1, ProcessState::Running, 1004)
            .unwrap();

        control
            .apply_control_action(2, ControlAction::Start, 1005)
            .unwrap();
        control
            .update_process_state(2, ProcessState::Error, 1006)
            .unwrap();

        // Test queries
        assert_eq!(control.running_processes().count(), 1);
        assert_eq!(control.processes_requiring_attention().count(), 1);
        assert_eq!(control.processes_by_controller(1).count(), 3);
        assert_eq!(control.processes_by_state(ProcessState::Stopped).count(), 1);
    }

    #[test]
    fn test_emergency_stop() {
        let mut control = ProcessControl::<DefaultConfig>::new(1);

        // Register and start multiple processes
        control.register_process(1, 1, 1000).unwrap();
        control.register_process(2, 1, 1001).unwrap();

        control
            .apply_control_action(1, ControlAction::Start, 1002)
            .unwrap();
        control
            .update_process_state(1, ProcessState::Running, 1003)
            .unwrap();

        control
            .apply_control_action(2, ControlAction::Start, 1004)
            .unwrap();
        control
            .update_process_state(2, ProcessState::Running, 1005)
            .unwrap();

        assert_eq!(control.running_processes().count(), 2);

        // Emergency stop all
        let stopped = control.emergency_stop_all(1006);
        assert_eq!(stopped, 2);
        assert_eq!(
            control.processes_by_state(ProcessState::Emergency).count(),
            2
        );
    }

    #[test]
    fn test_invalid_control_actions() {
        let mut control = ProcessControl::<DefaultConfig>::new(1);

        // Register stopped process
        control.register_process(42, 1, 1000).unwrap();

        // Try to stop a stopped process (should fail)
        let result = control.apply_control_action(42, ControlAction::Stop, 1001);
        assert!(result.is_err());

        // Start the process first
        control
            .apply_control_action(42, ControlAction::Start, 1002)
            .unwrap();
        control
            .update_process_state(42, ProcessState::Running, 1003)
            .unwrap();

        // Now stop should work
        let result = control.apply_control_action(42, ControlAction::Stop, 1004);
        assert!(result.is_ok());

        // Try to start a running process (should fail)
        let result = control.apply_control_action(42, ControlAction::Start, 1005);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_control_merge() {
        let mut control1 = ProcessControl::<DefaultConfig>::new(1);
        let mut control2 = ProcessControl::<DefaultConfig>::new(2);

        // Add different processes to each control system
        control1.register_process(1, 1, 1000).unwrap();
        control2.register_process(2, 1, 1001).unwrap();

        // Merge
        control1.merge(&control2).unwrap();

        // Should have both processes
        assert_eq!(control1.process_count(), 2);
        assert!(control1.get_process(1).is_some());
        assert!(control1.get_process(2).is_some());
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut control = ProcessControl::<DefaultConfig>::new(1);

        assert_eq!(control.element_count(), 0);
        assert!(control.can_add_element());

        control.register_process(42, 1, 1000).unwrap();
        assert_eq!(control.element_count(), 1);
        assert!(control.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut control1 = ProcessControl::<DefaultConfig>::new(1);
        let control2 = ProcessControl::<DefaultConfig>::new(2);

        assert!(control1.merge_bounded(&control2).is_ok());
        assert!(control1.validate_bounded().is_ok());
    }
}
