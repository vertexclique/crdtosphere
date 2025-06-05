//! Register CRDT implementations
//!
//! This module provides register-based CRDTs that store single values
//! with conflict resolution semantics.

pub mod lww;
pub mod mv;

// Re-export main types
pub use lww::LWWRegister;
pub use mv::MVRegister;
