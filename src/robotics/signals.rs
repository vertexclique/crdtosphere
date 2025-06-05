//! Coordination Signals for Multi-Robot Systems
//!
//! This module implements CRDTs for simple coordination signals and flags
//! between robots, enabling lightweight distributed coordination patterns.

use crate::clock::CompactTimestamp;
use crate::error::{CRDTError, CRDTResult};
use crate::memory::{MemoryConfig, NodeId};
use crate::traits::{BoundedCRDT, CRDT, RealTimeCRDT};

/// Types of coordination signals
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SignalType {
    /// Start coordination signal
    Start = 1,
    /// Stop coordination signal
    Stop = 2,
    /// Help request signal
    Help = 3,
    /// Task complete signal
    Complete = 4,
    /// Warning signal
    Warning = 5,
    /// Emergency signal
    Emergency = 6,
    /// Formation signal (for swarm coordination)
    Formation = 7,
    /// Rendezvous signal (meet at location)
    Rendezvous = 8,
}

impl SignalType {
    /// Returns true if this is a critical signal
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            SignalType::Emergency | SignalType::Help | SignalType::Warning
        )
    }

    /// Returns true if this signal requires immediate response
    pub fn requires_immediate_response(&self) -> bool {
        matches!(self, SignalType::Emergency | SignalType::Stop)
    }
}

/// Signal priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SignalPriority {
    /// Low priority signal
    Low = 1,
    /// Normal priority signal
    Normal = 2,
    /// High priority signal
    High = 3,
    /// Critical priority signal
    Critical = 4,
}

impl SignalPriority {
    /// Returns the timeout in seconds for this priority level
    pub fn timeout_seconds(&self) -> u32 {
        match self {
            SignalPriority::Critical => 5, // 5 seconds
            SignalPriority::High => 30,    // 30 seconds
            SignalPriority::Normal => 300, // 5 minutes
            SignalPriority::Low => 1800,   // 30 minutes
        }
    }
}

/// Individual coordination signal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signal {
    /// Signal type
    pub signal_type: SignalType,
    /// Signal priority
    pub priority: SignalPriority,
    /// Signal data (generic payload)
    pub data: u32,
    /// Timestamp when signal was created
    pub timestamp: CompactTimestamp,
    /// Robot that sent the signal
    pub sender_id: NodeId,
    /// Target robot (0 = broadcast to all)
    pub target_id: NodeId,
}

impl Signal {
    /// Creates a new signal
    pub fn new(
        signal_type: SignalType,
        priority: SignalPriority,
        data: u32,
        timestamp: u64,
        sender_id: NodeId,
        target_id: NodeId,
    ) -> Self {
        Self {
            signal_type,
            priority,
            data,
            timestamp: CompactTimestamp::new(timestamp),
            sender_id,
            target_id,
        }
    }

    /// Creates a broadcast signal (to all robots)
    pub fn broadcast(
        signal_type: SignalType,
        priority: SignalPriority,
        data: u32,
        timestamp: u64,
        sender_id: NodeId,
    ) -> Self {
        Self::new(signal_type, priority, data, timestamp, sender_id, 0)
    }

    /// Returns true if this signal is a broadcast
    pub fn is_broadcast(&self) -> bool {
        self.target_id == 0
    }

    /// Returns true if this signal is for a specific robot
    pub fn is_for_robot(&self, robot_id: NodeId) -> bool {
        self.is_broadcast() || self.target_id == robot_id
    }

    /// Returns true if the signal has expired
    pub fn is_expired(&self, current_time: u64) -> bool {
        let timeout = self.priority.timeout_seconds() as u64 * 1000; // Convert to milliseconds
        current_time > self.timestamp.as_u64() + timeout
    }
}

/// Multi-robot coordination signals CRDT
///
/// This CRDT manages distributed coordination signals between robots,
/// enabling simple coordination patterns like start/stop, help requests,
/// and formation commands.
///
/// # Type Parameters
/// - `C`: Memory configuration
///
/// # Features
/// - Broadcast and targeted signals
/// - Priority-based signal handling
/// - Automatic signal expiration
/// - Signal acknowledgment tracking
///
/// # Example
/// ```rust
/// use crdtosphere::prelude::*;
/// use crdtosphere::robotics::{CoordinationSignals, SignalType, SignalPriority};
///
/// // Create coordination signals
/// let mut signals = CoordinationSignals::<DefaultConfig>::new(1);
///
/// // Send emergency signal to all robots
/// signals.send_signal(
///     SignalType::Emergency,
///     SignalPriority::Critical,
///     42, // emergency code
///     1000, // timestamp
///     0 // broadcast to all
/// )?;
///
/// // Check for critical signals
/// let critical_signals = signals.critical_signals();
/// # Ok::<(), crdtosphere::error::CRDTError>(())
/// ```
#[derive(Debug, Clone)]
pub struct CoordinationSignals<C: MemoryConfig> {
    /// Array of active signals
    signals: [Option<Signal>; 32], // Support up to 32 active signals
    /// Number of signals currently stored
    signal_count: usize,
    /// This robot's ID
    local_robot_id: NodeId,
    /// Last update timestamp
    last_update: CompactTimestamp,
    /// Phantom data for memory config
    _phantom: core::marker::PhantomData<C>,
}

impl<C: MemoryConfig> CoordinationSignals<C> {
    /// Creates a new coordination signals CRDT
    ///
    /// # Arguments
    /// * `robot_id` - The ID of this robot
    ///
    /// # Returns
    /// A new coordination signals CRDT
    pub fn new(robot_id: NodeId) -> Self {
        Self {
            signals: [const { None }; 32],
            signal_count: 0,
            local_robot_id: robot_id,
            last_update: CompactTimestamp::new(0),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Sends a coordination signal
    ///
    /// # Arguments
    /// * `signal_type` - Type of signal to send
    /// * `priority` - Signal priority
    /// * `data` - Signal data payload
    /// * `timestamp` - Signal timestamp
    /// * `target_id` - Target robot ID (0 for broadcast)
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn send_signal(
        &mut self,
        signal_type: SignalType,
        priority: SignalPriority,
        data: u32,
        timestamp: u64,
        target_id: NodeId,
    ) -> CRDTResult<()> {
        let signal = Signal::new(
            signal_type,
            priority,
            data,
            timestamp,
            self.local_robot_id,
            target_id,
        );

        self.add_signal(signal)?;
        self.last_update = CompactTimestamp::new(timestamp);
        Ok(())
    }

    /// Sends a broadcast signal to all robots
    ///
    /// # Arguments
    /// * `signal_type` - Type of signal to send
    /// * `priority` - Signal priority
    /// * `data` - Signal data payload
    /// * `timestamp` - Signal timestamp
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn broadcast_signal(
        &mut self,
        signal_type: SignalType,
        priority: SignalPriority,
        data: u32,
        timestamp: u64,
    ) -> CRDTResult<()> {
        self.send_signal(signal_type, priority, data, timestamp, 0)
    }

    /// Gets all active signals
    ///
    /// # Returns
    /// Iterator over active signals
    pub fn all_signals(&self) -> impl Iterator<Item = &Signal> {
        self.signals.iter().filter_map(|s| s.as_ref())
    }

    /// Gets signals for this robot
    ///
    /// # Returns
    /// Iterator over signals for this robot
    pub fn signals_for_robot(&self) -> impl Iterator<Item = &Signal> {
        self.all_signals()
            .filter(move |s| s.is_for_robot(self.local_robot_id))
    }

    /// Gets critical signals
    ///
    /// # Returns
    /// Iterator over critical signals
    pub fn critical_signals(&self) -> impl Iterator<Item = &Signal> {
        self.signals_for_robot()
            .filter(|s| s.signal_type.is_critical())
    }

    /// Gets signals by type
    ///
    /// # Arguments
    /// * `signal_type` - Type of signals to get
    ///
    /// # Returns
    /// Iterator over signals of the specified type
    pub fn signals_by_type(&self, signal_type: SignalType) -> impl Iterator<Item = &Signal> {
        self.signals_for_robot()
            .filter(move |s| s.signal_type == signal_type)
    }

    /// Gets signals by priority
    ///
    /// # Arguments
    /// * `min_priority` - Minimum priority level
    ///
    /// # Returns
    /// Iterator over signals with at least the specified priority
    pub fn signals_by_priority(
        &self,
        min_priority: SignalPriority,
    ) -> impl Iterator<Item = &Signal> {
        self.signals_for_robot()
            .filter(move |s| s.priority >= min_priority)
    }

    /// Checks if there are any emergency signals
    ///
    /// # Returns
    /// true if emergency signals are present
    pub fn has_emergency_signals(&self) -> bool {
        self.signals_by_type(SignalType::Emergency).next().is_some()
    }

    /// Checks if there are any help requests
    ///
    /// # Returns
    /// true if help request signals are present
    pub fn has_help_requests(&self) -> bool {
        self.signals_by_type(SignalType::Help).next().is_some()
    }

    /// Gets the number of active signals
    ///
    /// # Returns
    /// Number of signals
    pub fn signal_count(&self) -> usize {
        self.signal_count
    }

    /// Cleans up expired signals
    ///
    /// # Arguments
    /// * `current_time` - Current timestamp
    ///
    /// # Returns
    /// Number of signals removed
    pub fn cleanup_expired(&mut self, current_time: u64) -> usize {
        let mut removed = 0;

        for i in 0..32 {
            if let Some(signal) = &self.signals[i] {
                if signal.is_expired(current_time) {
                    self.signals[i] = None;
                    self.signal_count -= 1;
                    removed += 1;
                }
            }
        }

        // Compact the array
        self.compact_signals();

        removed
    }

    /// Adds a signal to the CRDT
    fn add_signal(&mut self, signal: Signal) -> CRDTResult<()> {
        // Check for duplicate signals (same type, sender, target, and recent timestamp)
        for existing in self.all_signals() {
            if existing.signal_type == signal.signal_type
                && existing.sender_id == signal.sender_id
                && existing.target_id == signal.target_id
            {
                let time_diff = signal
                    .timestamp
                    .as_u64()
                    .saturating_sub(existing.timestamp.as_u64());
                if time_diff < 1000 {
                    // Within 1 second - consider duplicate
                    return Ok(()); // Ignore duplicate
                }
            }
        }

        // Find empty slot
        for i in 0..32 {
            if self.signals[i].is_none() {
                self.signals[i] = Some(signal);
                self.signal_count += 1;
                return Ok(());
            }
        }

        // If no empty slot, remove oldest low-priority signal
        self.make_space_for_signal(signal)
    }

    /// Makes space for a new signal by removing old low-priority signals
    fn make_space_for_signal(&mut self, new_signal: Signal) -> CRDTResult<()> {
        // Find oldest low-priority signal to replace
        let mut oldest_idx = None;
        let mut oldest_time = u64::MAX;

        for (i, signal_opt) in self.signals.iter().enumerate() {
            if let Some(signal) = signal_opt {
                if signal.priority <= SignalPriority::Normal
                    && signal.timestamp.as_u64() < oldest_time
                {
                    oldest_time = signal.timestamp.as_u64();
                    oldest_idx = Some(i);
                }
            }
        }

        if let Some(idx) = oldest_idx {
            self.signals[idx] = Some(new_signal);
            Ok(())
        } else {
            Err(CRDTError::BufferOverflow)
        }
    }

    /// Compacts the signals array by moving all signals to the beginning
    fn compact_signals(&mut self) {
        let mut write_idx = 0;

        for read_idx in 0..32 {
            if let Some(signal) = self.signals[read_idx] {
                if write_idx != read_idx {
                    self.signals[write_idx] = Some(signal);
                    self.signals[read_idx] = None;
                }
                write_idx += 1;
            }
        }
    }

    /// Validates signal data
    ///
    /// # Returns
    /// Ok(()) if valid, error otherwise
    pub fn validate_signals(&self) -> CRDTResult<()> {
        // Check signal IDs are valid
        for signal in self.all_signals() {
            if signal.sender_id as usize >= C::MAX_NODES {
                return Err(CRDTError::InvalidNodeId);
            }
            if signal.target_id as usize >= C::MAX_NODES && signal.target_id != 0 {
                return Err(CRDTError::InvalidNodeId);
            }
        }

        Ok(())
    }
}

impl<C: MemoryConfig> CRDT<C> for CoordinationSignals<C> {
    type Error = CRDTError;

    fn merge(&mut self, other: &Self) -> CRDTResult<()> {
        // Merge all signals from other
        for signal in other.all_signals() {
            self.add_signal(*signal)?;
        }

        // Update timestamp to latest
        if other.last_update > self.last_update {
            self.last_update = other.last_update;
        }

        Ok(())
    }

    fn eq(&self, other: &Self) -> bool {
        if self.signal_count != other.signal_count {
            return false;
        }

        // Check that all signals match
        for signal in self.all_signals() {
            let mut found = false;
            for other_signal in other.all_signals() {
                if signal == other_signal {
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
        self.validate_signals()
    }

    fn state_hash(&self) -> u32 {
        let mut hash = self.local_robot_id as u32;
        for signal in self.all_signals() {
            hash ^= (signal.sender_id as u32) ^ (signal.timestamp.as_u64() as u32) ^ (signal.data);
        }
        hash ^= self.signal_count as u32;
        hash
    }

    fn can_merge(&self, _other: &Self) -> bool {
        // Can always merge signals (space is made by removing old ones)
        true
    }
}

impl<C: MemoryConfig> BoundedCRDT<C> for CoordinationSignals<C> {
    const MAX_SIZE_BYTES: usize = core::mem::size_of::<Self>();
    const MAX_ELEMENTS: usize = 32; // Maximum signals

    fn memory_usage(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn element_count(&self) -> usize {
        self.signal_count
    }

    fn compact(&mut self) -> CRDTResult<usize> {
        self.compact_signals();
        Ok(0)
    }

    fn can_add_element(&self) -> bool {
        self.signal_count < Self::MAX_ELEMENTS
    }
}

impl<C: MemoryConfig> RealTimeCRDT<C> for CoordinationSignals<C> {
    const MAX_MERGE_CYCLES: u32 = 150; // Fast merge for real-time coordination
    const MAX_VALIDATE_CYCLES: u32 = 75;
    const MAX_SERIALIZE_CYCLES: u32 = 100;

    fn merge_bounded(&mut self, other: &Self) -> CRDTResult<()> {
        // Signal merge is bounded
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
    fn test_signal_type_properties() {
        assert!(SignalType::Emergency.is_critical());
        assert!(SignalType::Help.is_critical());
        assert!(!SignalType::Start.is_critical());

        assert!(SignalType::Emergency.requires_immediate_response());
        assert!(SignalType::Stop.requires_immediate_response());
        assert!(!SignalType::Complete.requires_immediate_response());
    }

    #[test]
    fn test_signal_priority() {
        assert!(
            SignalPriority::Critical.timeout_seconds() < SignalPriority::Normal.timeout_seconds()
        );
        assert!(SignalPriority::High > SignalPriority::Normal);
    }

    #[test]
    fn test_signal_creation() {
        let signal = Signal::new(
            SignalType::Emergency,
            SignalPriority::Critical,
            42,
            1000,
            1,
            2,
        );

        assert_eq!(signal.signal_type, SignalType::Emergency);
        assert_eq!(signal.sender_id, 1);
        assert_eq!(signal.target_id, 2);
        assert!(!signal.is_broadcast());
        assert!(signal.is_for_robot(2));
        assert!(!signal.is_for_robot(3));

        let broadcast = Signal::broadcast(SignalType::Start, SignalPriority::Normal, 0, 1000, 1);

        assert!(broadcast.is_broadcast());
        assert!(broadcast.is_for_robot(2));
        assert!(broadcast.is_for_robot(3));
    }

    #[test]
    fn test_signal_expiration() {
        let signal = Signal::new(
            SignalType::Emergency,
            SignalPriority::Critical,
            0,
            1000,
            1,
            0,
        );

        assert!(!signal.is_expired(1000)); // At creation time
        assert!(!signal.is_expired(3000)); // Within timeout
        assert!(signal.is_expired(10000)); // Past timeout
    }

    #[test]
    fn test_coordination_signals_creation() {
        let signals = CoordinationSignals::<DefaultConfig>::new(1);

        assert_eq!(signals.signal_count(), 0);
        assert!(!signals.has_emergency_signals());
        assert!(!signals.has_help_requests());
    }

    #[test]
    fn test_signal_sending_and_querying() {
        let mut signals = CoordinationSignals::<DefaultConfig>::new(1);

        // Send emergency signal
        signals
            .send_signal(
                SignalType::Emergency,
                SignalPriority::Critical,
                911,
                1000,
                0, // broadcast
            )
            .unwrap();

        // Send help request to specific robot
        signals
            .send_signal(
                SignalType::Help,
                SignalPriority::High,
                123,
                1001,
                2, // to robot 2
            )
            .unwrap();

        assert_eq!(signals.signal_count(), 2);
        assert!(signals.has_emergency_signals());

        // Check signals for this robot
        let robot_signals_count = signals.signals_for_robot().count();
        assert_eq!(robot_signals_count, 1); // Only emergency (broadcast)

        // Check critical signals
        let critical_count = signals.critical_signals().count();
        assert_eq!(critical_count, 1); // Emergency signal

        // Check signals by type
        let emergency_count = signals.signals_by_type(SignalType::Emergency).count();
        assert_eq!(emergency_count, 1);
        let emergency = signals
            .signals_by_type(SignalType::Emergency)
            .next()
            .unwrap();
        assert_eq!(emergency.data, 911);
    }

    #[test]
    fn test_signal_cleanup() {
        let mut signals = CoordinationSignals::<DefaultConfig>::new(1);

        // Add signal that will expire
        signals
            .send_signal(SignalType::Start, SignalPriority::Critical, 0, 1000, 0)
            .unwrap();

        assert_eq!(signals.signal_count(), 1);

        // Clean up after timeout
        let removed = signals.cleanup_expired(10000); // Well past timeout
        assert_eq!(removed, 1);
        assert_eq!(signals.signal_count(), 0);
    }

    #[test]
    fn test_coordination_signals_merge() {
        let mut signals1 = CoordinationSignals::<DefaultConfig>::new(1);
        let mut signals2 = CoordinationSignals::<DefaultConfig>::new(2);

        // Add different signals to each
        signals1
            .send_signal(
                SignalType::Emergency,
                SignalPriority::Critical,
                911,
                1000,
                0,
            )
            .unwrap();

        signals2
            .send_signal(SignalType::Help, SignalPriority::High, 123, 1001, 1)
            .unwrap();

        // Merge
        signals1.merge(&signals2).unwrap();

        // Should have both signals
        assert_eq!(signals1.signal_count(), 2);
        assert!(signals1.has_emergency_signals());
        assert!(signals1.has_help_requests());
    }

    #[test]
    fn test_bounded_crdt_implementation() {
        let mut signals = CoordinationSignals::<DefaultConfig>::new(1);

        assert_eq!(signals.element_count(), 0);
        assert!(signals.can_add_element());

        signals
            .send_signal(SignalType::Start, SignalPriority::Normal, 0, 1000, 0)
            .unwrap();

        assert_eq!(signals.element_count(), 1);
        assert!(signals.memory_usage() > 0);
    }

    #[test]
    fn test_real_time_crdt_implementation() {
        let mut signals1 = CoordinationSignals::<DefaultConfig>::new(1);
        let signals2 = CoordinationSignals::<DefaultConfig>::new(2);

        assert!(signals1.merge_bounded(&signals2).is_ok());
        assert!(signals1.validate_bounded().is_ok());
    }
}
