//! Automotive Domain CRDTs
//!
//! This module provides CRDTs specifically designed for automotive applications,
//! with ISO 26262 safety compliance and ECU coordination patterns.

pub mod safety;
pub mod sensors;

// Re-export main types
pub use safety::{ASILLevel, SafetyCRDT, SafetyLevel};
pub use sensors::{ReliabilityLevel, SensorFusion, SensorReading};
