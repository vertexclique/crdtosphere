//! Error handling module for CRDTosphere
//!
//! This module provides comprehensive error types for multi-domain embedded CRDT operations.

pub mod platform;
pub mod realtime;
pub mod safety;
pub mod types;

// Re-export main types
pub use platform::PlatformError;
pub use realtime::RealTimeError;
pub use safety::SafetyError;
pub use types::{CRDTError, CRDTResult};
