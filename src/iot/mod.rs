//! IoT Domain CRDTs
//!
//! This module provides CRDTs specifically designed for Internet of Things (IoT)
//! distributed coordination, focusing on device management and sensor networks.

pub mod devices;
pub mod sensors;

// Re-export main types
pub use devices::{ConnectionState, DeviceInfo, DeviceRegistry, DeviceStatus};
pub use sensors::{ReadingQuality, SensorNetwork, SensorReading, SensorType};
