//! Industrial Domain CRDTs
//!
//! This module provides CRDTs specifically designed for industrial automation
//! and control systems, focusing on distributed coordination in manufacturing.

pub mod equipment;
pub mod processes;

// Re-export main types
pub use equipment::{EquipmentInfo, EquipmentRegistry, EquipmentStatus, MaintenanceState};
pub use processes::{ControlAction, ProcessControl, ProcessState, ProcessStep};
