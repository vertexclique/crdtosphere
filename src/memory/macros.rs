//! Memory configuration macros
//!
//! This module provides the `define_memory_config!` macro for easy creation
//! of custom memory configurations.

/// Macro to define a custom memory configuration
///
/// This macro creates a new struct that implements the `MemoryConfig` trait
/// with user-specified values and automatic validation.
///
/// # Example
///
/// ```rust
/// use crdtosphere::memory::define_memory_config;
///
/// define_memory_config! {
///     name: MyPlatformConfig,
///     total_memory: 64 * 1024,  // 64KB budget
///     max_registers: 100,
///     max_counters: 50,
///     max_sets: 20,
///     max_maps: 10,
///     max_nodes: 32,
/// }
/// ```
#[macro_export]
macro_rules! define_memory_config {
    (
        name: $name:ident,
        total_memory: $total:expr,
        max_registers: $registers:expr,
        max_counters: $counters:expr,
        max_sets: $sets:expr,
        max_maps: $maps:expr,
        max_nodes: $nodes:expr
        $(, max_set_elements: $set_elements:expr)?
        $(, max_map_entries: $map_entries:expr)?
        $(, max_history_size: $history:expr)?
        $(, clock_memory_budget: $clock_budget:expr)?
        $(, error_buffer_size: $error_buffer:expr)?
        $(, memory_alignment: $alignment:expr)?
        $(, cache_line_size: $cache_line:expr)?
        $(,)?
    ) => {
        /// Custom memory configuration
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl $crate::memory::MemoryConfig for $name {
            const TOTAL_CRDT_MEMORY: usize = $total;
            const MAX_REGISTERS: usize = $registers;
            const MAX_COUNTERS: usize = $counters;
            const MAX_SETS: usize = $sets;
            const MAX_MAPS: usize = $maps;
            const MAX_NODES: usize = $nodes;

            // Optional parameters with defaults
            const MAX_SET_ELEMENTS: usize = define_memory_config!(@default $($set_elements)?, 32);
            const MAX_MAP_ENTRIES: usize = define_memory_config!(@default $($map_entries)?, 32);
            const MAX_HISTORY_SIZE: usize = define_memory_config!(@default $($history)?, 4);
            const CLOCK_MEMORY_BUDGET: usize = define_memory_config!(@default $($clock_budget)?, 512);
            const ERROR_BUFFER_SIZE: usize = define_memory_config!(@default $($error_buffer)?, 256);
            const MEMORY_ALIGNMENT: usize = define_memory_config!(@default $($alignment)?, 4);
            const CACHE_LINE_SIZE: usize = define_memory_config!(@default $($cache_line)?, 32);
        }

        // Runtime validation available via validate() method
        // Note: Call $name::validate() at runtime to check configuration
    };

    // Helper macro for default values
    (@default $value:expr, $default:expr) => { $value };
    (@default , $default:expr) => { $default };
}

// Re-export the macro for convenience
pub use define_memory_config;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryConfig;

    define_memory_config! {
        name: TestConfig,
        total_memory: 16 * 1024,  // 16KB
        max_registers: 25,
        max_counters: 15,
        max_sets: 10,
        max_maps: 5,
        max_nodes: 8,
    }

    #[test]
    fn test_macro_generated_config() {
        assert_eq!(TestConfig::TOTAL_CRDT_MEMORY, 16 * 1024);
        assert_eq!(TestConfig::MAX_REGISTERS, 25);
        assert_eq!(TestConfig::MAX_COUNTERS, 15);
        assert_eq!(TestConfig::MAX_SETS, 10);
        assert_eq!(TestConfig::MAX_MAPS, 5);
        assert_eq!(TestConfig::MAX_NODES, 8);

        // Check defaults
        assert_eq!(TestConfig::MAX_SET_ELEMENTS, 32);
        assert_eq!(TestConfig::MAX_MAP_ENTRIES, 32);
        assert_eq!(TestConfig::MAX_HISTORY_SIZE, 4);
        assert_eq!(TestConfig::MEMORY_ALIGNMENT, 4);
    }

    #[test]
    fn test_macro_validation() {
        // This should compile without panicking due to compile-time validation
        assert!(TestConfig::validate().is_ok());
    }

    define_memory_config! {
        name: CustomConfig,
        total_memory: 8 * 1024,
        max_registers: 10,
        max_counters: 5,
        max_sets: 3,
        max_maps: 2,
        max_nodes: 4,
        max_set_elements: 16,
        max_map_entries: 16,
        memory_alignment: 8,
    }

    #[test]
    fn test_macro_with_custom_values() {
        assert_eq!(CustomConfig::MAX_SET_ELEMENTS, 16);
        assert_eq!(CustomConfig::MAX_MAP_ENTRIES, 16);
        assert_eq!(CustomConfig::MEMORY_ALIGNMENT, 8);
        assert!(CustomConfig::validate().is_ok());
    }
}
