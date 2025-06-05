//! Memory configuration trait and implementations
//!
//! This module defines the MemoryConfig trait that allows users to configure
//! memory limits for CRDTs at compile time.

/// Memory configuration trait for compile-time memory management
///
/// This trait defines memory limits and constraints for CRDT implementations.
/// All memory usage is determined at compile time to ensure deterministic behavior.
pub trait MemoryConfig: Clone {
    /// Total memory budget for all CRDTs in bytes
    const TOTAL_CRDT_MEMORY: usize;

    /// Maximum number of register CRDT instances
    const MAX_REGISTERS: usize;

    /// Maximum number of counter CRDT instances
    const MAX_COUNTERS: usize;

    /// Maximum number of set CRDT instances
    const MAX_SETS: usize;

    /// Maximum number of map CRDT instances
    const MAX_MAPS: usize;

    /// Maximum number of elements per set CRDT
    const MAX_SET_ELEMENTS: usize;

    /// Maximum number of entries per map CRDT
    const MAX_MAP_ENTRIES: usize;

    /// Maximum history size for multi-value CRDTs
    const MAX_HISTORY_SIZE: usize;

    /// Maximum number of nodes (ECUs/robots/devices/controllers) in the network
    const MAX_NODES: usize;

    /// Memory budget for clock management in bytes
    const CLOCK_MEMORY_BUDGET: usize;

    /// Error buffer size in bytes
    const ERROR_BUFFER_SIZE: usize;

    /// Memory alignment requirement in bytes (must be power of 2)
    const MEMORY_ALIGNMENT: usize;

    /// Cache line size for optimization in bytes
    const CACHE_LINE_SIZE: usize;

    /// Validates that the configuration is consistent and within bounds
    fn validate() -> Result<(), &'static str> {
        // Check that alignment is a power of 2
        if Self::MEMORY_ALIGNMENT == 0
            || (Self::MEMORY_ALIGNMENT & (Self::MEMORY_ALIGNMENT - 1)) != 0
        {
            return Err("MEMORY_ALIGNMENT must be a power of 2");
        }

        // Check node count limits
        if Self::MAX_NODES > 255 {
            return Err("MAX_NODES cannot exceed 255 for efficient node ID representation");
        }

        // Check set element limits for bitmap representation
        if Self::MAX_SET_ELEMENTS > 64 {
            return Err("MAX_SET_ELEMENTS cannot exceed 64 for efficient bitmap representation");
        }

        // Basic memory budget check
        let estimated_usage = Self::estimate_memory_usage();
        if estimated_usage > Self::TOTAL_CRDT_MEMORY {
            return Err("Estimated memory usage exceeds configured budget");
        }

        Ok(())
    }

    /// Estimates total memory usage based on configuration
    fn estimate_memory_usage() -> usize {
        let clock_memory = Self::CLOCK_MEMORY_BUDGET;
        let error_memory = Self::ERROR_BUFFER_SIZE;

        // Estimate CRDT memory usage (conservative estimates)
        let register_memory = Self::MAX_REGISTERS * 16; // ~16 bytes per register
        let counter_memory = Self::MAX_COUNTERS * 8; // ~8 bytes per counter
        let set_memory = Self::MAX_SETS * (8 + Self::MAX_SET_ELEMENTS.div_ceil(8)); // Metadata + bitmap
        let map_memory = Self::MAX_MAPS * Self::MAX_MAP_ENTRIES * 12; // ~12 bytes per entry

        clock_memory + error_memory + register_memory + counter_memory + set_memory + map_memory
    }
}

/// Node ID type for embedded systems
///
/// Uses u8 for up to 256 nodes which is sufficient for most embedded networks
pub type NodeId = u8;

/// Default memory configuration for testing and examples
#[derive(Debug, Clone, Copy)]
pub struct DefaultConfig;

impl Default for DefaultConfig {
    fn default() -> Self {
        Self
    }
}

impl MemoryConfig for DefaultConfig {
    const TOTAL_CRDT_MEMORY: usize = 32 * 1024; // 32KB
    const MAX_REGISTERS: usize = 50;
    const MAX_COUNTERS: usize = 25;
    const MAX_SETS: usize = 15;
    const MAX_MAPS: usize = 10;
    const MAX_SET_ELEMENTS: usize = 32;
    const MAX_MAP_ENTRIES: usize = 32;
    const MAX_HISTORY_SIZE: usize = 4;
    const MAX_NODES: usize = 16;
    const CLOCK_MEMORY_BUDGET: usize = 512;
    const ERROR_BUFFER_SIZE: usize = 256;
    const MEMORY_ALIGNMENT: usize = 4;
    const CACHE_LINE_SIZE: usize = 32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        assert!(DefaultConfig::validate().is_ok());
    }

    #[test]
    fn test_memory_estimation() {
        let estimated = DefaultConfig::estimate_memory_usage();
        assert!(estimated <= DefaultConfig::TOTAL_CRDT_MEMORY);
        assert!(estimated > 0);
    }
}
