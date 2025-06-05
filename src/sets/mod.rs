//! Set CRDT implementations
//!
//! This module provides set-based CRDTs for tracking collections of elements
//! with different semantics (grow-only, add/remove).

pub mod gset;
pub mod orset;

// Re-export main types
pub use gset::GSet;
pub use orset::ORSet;
