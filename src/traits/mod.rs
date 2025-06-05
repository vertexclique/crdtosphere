//! Core CRDT traits module
//!
//! This module defines the fundamental traits that all CRDTs must implement,
//! providing the foundation for the entire CRDTosphere library.

pub mod bounded;
pub mod crdt;
pub mod platform;
pub mod realtime;
pub mod safety;

// Re-export main traits
pub use bounded::BoundedCRDT;
pub use crdt::CRDT;
pub use platform::PlatformCRDT;
pub use realtime::RealTimeCRDT;
pub use safety::SafetyCRDT;
