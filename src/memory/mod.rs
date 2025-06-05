//! Memory management module for CRDTosphere
//!
//! This module provides configurable memory management for embedded CRDT implementations.
//! It includes compile-time memory configuration, validation, and static memory pools.

pub mod config;
pub mod macros;
pub mod validation;

// Re-export main types
pub use config::{DefaultConfig, MemoryConfig, NodeId};
pub use macros::define_memory_config;
pub use validation::MemoryValidator;
