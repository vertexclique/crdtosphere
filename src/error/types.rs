//! Core error types for CRDTosphere
//!
//! This module defines the main error types used throughout the library.

use crate::error::{PlatformError, RealTimeError, SafetyError};

/// Main error type for CRDT operations
///
/// This enum encompasses all possible errors that can occur during CRDT operations
/// across different platforms and domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CRDTError {
    // Memory-related errors
    /// Out of memory in static allocation pools
    OutOfMemory,
    /// Invalid memory alignment
    InvalidAlignment,
    /// Buffer overflow detected
    BufferOverflow,
    /// Configuration limits exceeded
    ConfigurationExceeded,

    // Real-time errors
    /// Operation deadline exceeded
    DeadlineExceeded,
    /// Lock acquisition timeout
    LockTimeout,
    /// Interrupt overrun detected
    InterruptOverrun,
    /// Platform-specific timeout
    PlatformSpecificTimeout,

    // Multi-domain safety errors
    /// Safety violation detected
    SafetyViolation,
    /// Data integrity check failed
    IntegrityCheckFailed,
    /// Invalid safety level
    InvalidSafetyLevel,
    /// Domain-specific error
    DomainSpecificError,

    // CRDT-specific errors
    /// Clock skew detected
    ClockSkew,
    /// Invalid merge operation
    InvalidMerge,
    /// Causality violation
    CausalityViolation,
    /// Node count exceeded
    NodeCountExceeded,
    /// Invalid node ID
    InvalidNodeId,
    /// Invalid state detected
    InvalidState,
    /// Invalid operation attempted
    InvalidOperation,

    // Platform-specific errors
    /// Platform not supported
    PlatformNotSupported(PlatformError),
    /// Hardware feature unavailable
    HardwareFeatureUnavailable,
    /// Configuration mismatch
    ConfigurationMismatch,

    // Real-time specific errors
    /// Real-time constraint violation
    RealTimeViolation(RealTimeError),
}

impl CRDTError {
    /// Returns true if this is a recoverable error
    pub const fn is_recoverable(&self) -> bool {
        match self {
            // Non-recoverable errors
            Self::OutOfMemory
            | Self::BufferOverflow
            | Self::ConfigurationExceeded
            | Self::SafetyViolation
            | Self::IntegrityCheckFailed
            | Self::CausalityViolation
            | Self::PlatformNotSupported(_)
            | Self::ConfigurationMismatch => false,

            // Potentially recoverable errors
            Self::InvalidAlignment
            | Self::DeadlineExceeded
            | Self::LockTimeout
            | Self::InterruptOverrun
            | Self::PlatformSpecificTimeout
            | Self::InvalidSafetyLevel
            | Self::DomainSpecificError
            | Self::ClockSkew
            | Self::InvalidMerge
            | Self::NodeCountExceeded
            | Self::InvalidNodeId
            | Self::InvalidState
            | Self::InvalidOperation
            | Self::HardwareFeatureUnavailable
            | Self::RealTimeViolation(_) => true,
        }
    }

    /// Returns true if this is a safety-critical error
    pub const fn is_safety_critical(&self) -> bool {
        match self {
            Self::SafetyViolation
            | Self::IntegrityCheckFailed
            | Self::CausalityViolation
            | Self::BufferOverflow => true,
            _ => false,
        }
    }

    /// Returns true if this is a real-time related error
    pub const fn is_realtime_error(&self) -> bool {
        match self {
            Self::DeadlineExceeded
            | Self::LockTimeout
            | Self::InterruptOverrun
            | Self::PlatformSpecificTimeout
            | Self::RealTimeViolation(_) => true,
            _ => false,
        }
    }

    /// Returns true if this is a platform-specific error
    pub const fn is_platform_error(&self) -> bool {
        match self {
            Self::PlatformNotSupported(_)
            | Self::HardwareFeatureUnavailable
            | Self::PlatformSpecificTimeout => true,
            _ => false,
        }
    }

    /// Returns the error category as a string
    pub const fn category(&self) -> &'static str {
        match self {
            Self::OutOfMemory
            | Self::InvalidAlignment
            | Self::BufferOverflow
            | Self::ConfigurationExceeded => "Memory",

            Self::DeadlineExceeded
            | Self::LockTimeout
            | Self::InterruptOverrun
            | Self::PlatformSpecificTimeout
            | Self::RealTimeViolation(_) => "RealTime",

            Self::SafetyViolation
            | Self::IntegrityCheckFailed
            | Self::InvalidSafetyLevel
            | Self::DomainSpecificError => "Safety",

            Self::ClockSkew
            | Self::InvalidMerge
            | Self::CausalityViolation
            | Self::NodeCountExceeded
            | Self::InvalidNodeId
            | Self::InvalidState
            | Self::InvalidOperation => "CRDT",

            Self::PlatformNotSupported(_)
            | Self::HardwareFeatureUnavailable
            | Self::ConfigurationMismatch => "Platform",
        }
    }
}

impl From<SafetyError> for CRDTError {
    fn from(_error: SafetyError) -> Self {
        CRDTError::SafetyViolation
    }
}

impl From<PlatformError> for CRDTError {
    fn from(error: PlatformError) -> Self {
        Self::PlatformNotSupported(error)
    }
}

impl From<RealTimeError> for CRDTError {
    fn from(error: RealTimeError) -> Self {
        Self::RealTimeViolation(error)
    }
}

/// Result type for CRDT operations
pub type CRDTResult<T> = Result<T, CRDTError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_classification() {
        assert!(!CRDTError::OutOfMemory.is_recoverable());
        assert!(CRDTError::DeadlineExceeded.is_recoverable());

        assert!(CRDTError::IntegrityCheckFailed.is_safety_critical());
        assert!(!CRDTError::ClockSkew.is_safety_critical());

        assert!(CRDTError::DeadlineExceeded.is_realtime_error());
        assert!(!CRDTError::OutOfMemory.is_realtime_error());

        assert!(CRDTError::HardwareFeatureUnavailable.is_platform_error());
        assert!(!CRDTError::InvalidMerge.is_platform_error());
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(CRDTError::OutOfMemory.category(), "Memory");
        assert_eq!(CRDTError::DeadlineExceeded.category(), "RealTime");
        assert_eq!(CRDTError::IntegrityCheckFailed.category(), "Safety");
        assert_eq!(CRDTError::ClockSkew.category(), "CRDT");
        assert_eq!(CRDTError::HardwareFeatureUnavailable.category(), "Platform");
    }
}
