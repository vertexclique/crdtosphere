//! Memory validation utilities
//!
//! This module provides runtime memory validation to ensure
//! configurations are safe and within bounds.

use crate::memory::MemoryConfig;

/// Memory validator for runtime verification
pub struct MemoryValidator;

impl MemoryValidator {
    /// Validates memory configuration at runtime
    pub fn validate<C: MemoryConfig>() -> Result<(), &'static str> {
        C::validate()
    }

    /// Checks if estimated memory usage is within budget
    pub fn check_memory_budget<C: MemoryConfig>() -> Result<(), &'static str> {
        let estimated = C::estimate_memory_usage();
        let budget = C::TOTAL_CRDT_MEMORY;

        if estimated > budget {
            return Err("Estimated memory usage exceeds configured budget");
        }

        // Warn if usage is very close to budget (>90%)
        if estimated * 10 > budget * 9 {
            // Note: This is just a compile-time check, no runtime warning
        }

        Ok(())
    }

    /// Validates alignment requirements
    pub fn check_alignment<C: MemoryConfig>() -> Result<(), &'static str> {
        let alignment = C::MEMORY_ALIGNMENT;

        // Check that alignment is a power of 2
        if alignment == 0 || (alignment & (alignment - 1)) != 0 {
            return Err("MEMORY_ALIGNMENT must be a power of 2");
        }

        // Check reasonable alignment bounds
        if alignment > 64 {
            return Err("MEMORY_ALIGNMENT should not exceed 64 bytes");
        }

        Ok(())
    }

    /// Validates node count limits
    pub fn check_node_limits<C: MemoryConfig>() -> Result<(), &'static str> {
        let max_nodes = C::MAX_NODES;

        if max_nodes == 0 {
            return Err("MAX_NODES must be at least 1");
        }

        if max_nodes > 255 {
            return Err("MAX_NODES cannot exceed 255 for efficient node ID representation");
        }

        Ok(())
    }

    /// Validates CRDT instance limits
    pub fn check_crdt_limits<C: MemoryConfig>() -> Result<(), &'static str> {
        if C::MAX_REGISTERS == 0 && C::MAX_COUNTERS == 0 && C::MAX_SETS == 0 && C::MAX_MAPS == 0 {
            return Err("At least one CRDT type must have a non-zero limit");
        }

        // Check set element limits for efficient bitmap representation
        if C::MAX_SET_ELEMENTS > 64 {
            return Err("MAX_SET_ELEMENTS cannot exceed 64 for efficient bitmap representation");
        }

        if C::MAX_SET_ELEMENTS == 0 && C::MAX_SETS > 0 {
            return Err("MAX_SET_ELEMENTS must be non-zero if MAX_SETS > 0");
        }

        if C::MAX_MAP_ENTRIES == 0 && C::MAX_MAPS > 0 {
            return Err("MAX_MAP_ENTRIES must be non-zero if MAX_MAPS > 0");
        }

        Ok(())
    }

    /// Validates cache line size
    pub fn check_cache_line<C: MemoryConfig>() -> Result<(), &'static str> {
        let cache_line = C::CACHE_LINE_SIZE;

        // Check that cache line size is a power of 2
        if cache_line == 0 || (cache_line & (cache_line - 1)) != 0 {
            return Err("CACHE_LINE_SIZE must be a power of 2");
        }

        // Check reasonable cache line bounds (8 to 128 bytes)
        if !(8..=128).contains(&cache_line) {
            return Err("CACHE_LINE_SIZE should be between 8 and 128 bytes");
        }

        Ok(())
    }

    /// Comprehensive validation of all configuration aspects
    pub fn validate_all<C: MemoryConfig>() -> Result<(), &'static str> {
        Self::check_alignment::<C>()?;

        Self::check_node_limits::<C>()?;

        Self::check_crdt_limits::<C>()?;

        Self::check_cache_line::<C>()?;

        Self::check_memory_budget::<C>()?;

        Ok(())
    }
}

// Note: Memory configuration validation is performed at runtime only.
// Use MemoryValidator::validate_all::<YourConfig>() to validate configurations.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{DefaultConfig, define_memory_config};

    #[test]
    fn test_default_config_validation() {
        assert!(MemoryValidator::validate_all::<DefaultConfig>().is_ok());
    }

    #[test]
    fn test_memory_budget_check() {
        assert!(MemoryValidator::check_memory_budget::<DefaultConfig>().is_ok());
    }

    #[test]
    fn test_alignment_check() {
        assert!(MemoryValidator::check_alignment::<DefaultConfig>().is_ok());
    }

    #[test]
    fn test_node_limits_check() {
        assert!(MemoryValidator::check_node_limits::<DefaultConfig>().is_ok());
    }

    #[test]
    fn test_crdt_limits_check() {
        assert!(MemoryValidator::check_crdt_limits::<DefaultConfig>().is_ok());
    }

    #[test]
    fn test_cache_line_check() {
        assert!(MemoryValidator::check_cache_line::<DefaultConfig>().is_ok());
    }

    // Test configuration with runtime validation
    define_memory_config! {
        name: ValidatedConfig,
        total_memory: 16 * 1024,
        max_registers: 20,
        max_counters: 10,
        max_sets: 5,
        max_maps: 3,
        max_nodes: 8,
    }

    #[test]
    fn test_validated_config() {
        assert!(MemoryValidator::validate_all::<ValidatedConfig>().is_ok());
    }
}
