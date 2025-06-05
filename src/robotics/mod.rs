//! Robotics Domain CRDTs
//!
//! This module provides CRDTs specifically designed for multi-robot coordination,
//! focusing on distributed state synchronization between robots.

pub mod mapping;
pub mod signals;
pub mod status;

// Re-export main types
pub use mapping::{MapData, MapPoint, MapPointType, SharedMap};
pub use signals::{CoordinationSignals, Signal, SignalPriority, SignalType};
pub use status::{BatteryLevel, OperationalMode, Position3D, RobotStatus};
