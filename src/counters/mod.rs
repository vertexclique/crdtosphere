//! Counter CRDT implementations
//!
//! This module provides counter-based CRDTs for tracking numeric values
//! with different semantics (grow-only, increment/decrement).

pub mod gcounter;
pub mod pncounter;

// Re-export main types
pub use gcounter::GCounter;
pub use pncounter::PNCounter;
