//! Clock management module
//!
//! This module provides the CompactTimestamp type used by CRDTs.
//! All CRDTs use explicit timestamps passed as parameters for deterministic behavior.

/// Compact timestamp for embedded systems
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompactTimestamp {
    /// Timestamp value
    pub value: u64,
}

impl CompactTimestamp {
    /// Creates a new timestamp
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    /// Creates a zero timestamp
    pub const fn zero() -> Self {
        Self { value: 0 }
    }

    /// Returns the timestamp value
    pub const fn value(&self) -> u64 {
        self.value
    }

    /// Returns the timestamp as u64
    pub const fn as_u64(&self) -> u64 {
        self.value
    }
}
