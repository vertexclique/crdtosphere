//! Platform-specific constants and optimizations
//!
//! This module provides platform-specific constants and optimizations
//! for different embedded platforms without requiring HAL dependencies.

/// Platform-specific constants for AURIX TriCore
#[cfg(feature = "aurix")]
pub mod constants {
    /// Maximum merge cycles for AURIX platform
    pub const MAX_MERGE_CYCLES: u32 = 500;

    /// Maximum interrupt latency in CPU cycles
    pub const MAX_INTERRUPT_LATENCY: u32 = 100;

    /// Cache line size in bytes
    pub const CACHE_LINE_SIZE: usize = 32;

    /// Supports multi-core operations
    pub const SUPPORTS_MULTICORE: bool = true;

    /// Maximum number of cores
    pub const MAX_CORES: u8 = 3;

    /// Memory alignment requirement
    pub const MEMORY_ALIGNMENT: usize = 32;

    /// Platform name
    pub const PLATFORM_NAME: &str = "AURIX";
}

/// Platform-specific constants for STM32
#[cfg(feature = "stm32")]
pub mod constants {
    /// Maximum merge cycles for STM32 platform
    pub const MAX_MERGE_CYCLES: u32 = 200;

    /// Maximum interrupt latency in CPU cycles
    pub const MAX_INTERRUPT_LATENCY: u32 = 50;

    /// Cache line size in bytes
    pub const CACHE_LINE_SIZE: usize = 32;

    /// Supports multi-core operations
    pub const SUPPORTS_MULTICORE: bool = false;

    /// Maximum number of cores
    pub const MAX_CORES: u8 = 1;

    /// Memory alignment requirement
    pub const MEMORY_ALIGNMENT: usize = 4;

    /// Platform name
    pub const PLATFORM_NAME: &str = "STM32";
}

/// Platform-specific constants for Cortex-M
#[cfg(feature = "cortex-m")]
pub mod constants {
    /// Maximum merge cycles for Cortex-M platform
    pub const MAX_MERGE_CYCLES: u32 = 100;

    /// Maximum interrupt latency in CPU cycles
    pub const MAX_INTERRUPT_LATENCY: u32 = 25;

    /// Cache line size in bytes
    pub const CACHE_LINE_SIZE: usize = 32;

    /// Supports multi-core operations
    pub const SUPPORTS_MULTICORE: bool = false;

    /// Maximum number of cores
    pub const MAX_CORES: u8 = 1;

    /// Memory alignment requirement
    pub const MEMORY_ALIGNMENT: usize = 4;

    /// Platform name
    pub const PLATFORM_NAME: &str = "Cortex-M";
}

/// Platform-specific constants for RISC-V
#[cfg(feature = "riscv")]
pub mod constants {
    /// Maximum merge cycles for RISC-V platform
    pub const MAX_MERGE_CYCLES: u32 = 300;

    /// Maximum interrupt latency in CPU cycles
    pub const MAX_INTERRUPT_LATENCY: u32 = 30;

    /// Cache line size in bytes
    pub const CACHE_LINE_SIZE: usize = 64;

    /// Supports multi-core operations
    pub const SUPPORTS_MULTICORE: bool = true;

    /// Maximum number of cores (variable for RISC-V)
    pub const MAX_CORES: u8 = 8;

    /// Memory alignment requirement
    pub const MEMORY_ALIGNMENT: usize = 8;

    /// Platform name
    pub const PLATFORM_NAME: &str = "RISC-V";
}

/// Default platform constants (when no specific platform is selected)
#[cfg(not(any(
    feature = "aurix",
    feature = "stm32",
    feature = "cortex-m",
    feature = "riscv"
)))]
pub mod constants {
    /// Maximum merge cycles for default platform
    pub const MAX_MERGE_CYCLES: u32 = 150;

    /// Maximum interrupt latency in CPU cycles
    pub const MAX_INTERRUPT_LATENCY: u32 = 40;

    /// Cache line size in bytes
    pub const CACHE_LINE_SIZE: usize = 32;

    /// Supports multi-core operations
    pub const SUPPORTS_MULTICORE: bool = false;

    /// Maximum number of cores
    pub const MAX_CORES: u8 = 1;

    /// Memory alignment requirement
    pub const MEMORY_ALIGNMENT: usize = 4;

    /// Platform name
    pub const PLATFORM_NAME: &str = "Generic";
}

/// Platform-specific validation limits
pub mod validation {
    /// Maximum active nodes for platform-specific validation
    #[cfg(feature = "aurix")]
    pub const MAX_ACTIVE_NODES: usize = 3; // AURIX TriCore limit

    /// Maximum active nodes for platform-specific validation
    #[cfg(feature = "stm32")]
    pub const MAX_ACTIVE_NODES: usize = 8; // STM32 power-aware limit

    /// Maximum active nodes for platform-specific validation
    #[cfg(feature = "cortex-m")]
    pub const MAX_ACTIVE_NODES: usize = 4; // Cortex-M memory constraint

    /// Maximum active nodes for platform-specific validation
    #[cfg(feature = "riscv")]
    pub const MAX_ACTIVE_NODES: usize = 16; // RISC-V flexible limit

    /// Maximum active nodes for platform-specific validation (default)
    #[cfg(not(any(
        feature = "aurix",
        feature = "stm32",
        feature = "cortex-m",
        feature = "riscv"
    )))]
    pub const MAX_ACTIVE_NODES: usize = 8; // Default conservative limit

    /// Maximum memory usage for platform-specific validation
    #[cfg(feature = "aurix")]
    pub const MAX_MEMORY_USAGE: usize = 8192; // AURIX has more memory

    /// Maximum memory usage for platform-specific validation
    #[cfg(feature = "stm32")]
    pub const MAX_MEMORY_USAGE: usize = 2048; // STM32 moderate memory

    /// Maximum memory usage for platform-specific validation
    #[cfg(feature = "cortex-m")]
    pub const MAX_MEMORY_USAGE: usize = 1024; // Cortex-M memory constraint

    /// Maximum memory usage for platform-specific validation
    #[cfg(feature = "riscv")]
    pub const MAX_MEMORY_USAGE: usize = 4096; // RISC-V variable memory

    /// Maximum memory usage for platform-specific validation
    #[cfg(not(any(
        feature = "aurix",
        feature = "stm32",
        feature = "cortex-m",
        feature = "riscv"
    )))]
    pub const MAX_MEMORY_USAGE: usize = 2048; // Default moderate limit
}

/// Platform-specific error handling types
pub mod error_handling {
    #[allow(unused_imports)]
    use crate::error::CRDTError;

    /// AURIX safety actions for error handling
    #[cfg(feature = "aurix")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AurixSafetyAction {
        /// Continue normal operation
        ContinueOperation,
        /// Enter safe state
        SafeState,
        /// Reset the system
        SystemReset,
        /// Isolate the node
        IsolateNode,
    }

    #[cfg(feature = "aurix")]
    impl From<CRDTError> for AurixSafetyAction {
        fn from(err: CRDTError) -> Self {
            match err {
                CRDTError::BufferOverflow => AurixSafetyAction::SafeState,
                CRDTError::InvalidState => AurixSafetyAction::SystemReset,
                CRDTError::InvalidNodeId => AurixSafetyAction::IsolateNode,
                _ => AurixSafetyAction::ContinueOperation,
            }
        }
    }

    /// STM32 power management actions for error handling
    #[cfg(feature = "stm32")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum STM32PowerAction {
        /// Continue normal operation
        Continue,
        /// Reduce CPU frequency
        ReduceFrequency,
        /// Enter stop mode
        EnterStopMode,
        /// Enter standby mode
        EnterStandbyMode,
    }

    #[cfg(feature = "stm32")]
    impl From<CRDTError> for STM32PowerAction {
        fn from(err: CRDTError) -> Self {
            match err {
                CRDTError::BufferOverflow => STM32PowerAction::ReduceFrequency,
                CRDTError::InvalidState => STM32PowerAction::EnterStopMode,
                CRDTError::ConfigurationExceeded => STM32PowerAction::EnterStandbyMode,
                _ => STM32PowerAction::Continue,
            }
        }
    }

    /// Cortex-M memory management actions for error handling
    #[cfg(feature = "cortex-m")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CortexMMemoryAction {
        /// Continue normal operation
        Continue,
        /// Compact memory
        CompactMemory,
        /// Reduce capacity
        ReduceCapacity,
        /// Reset to minimal state
        ResetMinimal,
    }

    #[cfg(feature = "cortex-m")]
    impl From<CRDTError> for CortexMMemoryAction {
        fn from(err: CRDTError) -> Self {
            match err {
                CRDTError::BufferOverflow => CortexMMemoryAction::CompactMemory,
                CRDTError::ConfigurationExceeded => CortexMMemoryAction::ReduceCapacity,
                CRDTError::InvalidState => CortexMMemoryAction::ResetMinimal,
                _ => CortexMMemoryAction::Continue,
            }
        }
    }

    /// RISC-V performance actions for error handling
    #[cfg(feature = "riscv")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RiscVPerformanceAction {
        /// Continue normal operation
        Continue,
        /// Optimize for performance
        OptimizePerformance,
        /// Distribute load
        DistributeLoad,
        /// Scale down operations
        ScaleDown,
    }

    #[cfg(feature = "riscv")]
    impl From<CRDTError> for RiscVPerformanceAction {
        fn from(err: CRDTError) -> Self {
            match err {
                CRDTError::BufferOverflow => RiscVPerformanceAction::OptimizePerformance,
                CRDTError::ConfigurationExceeded => RiscVPerformanceAction::DistributeLoad,
                CRDTError::InvalidState => RiscVPerformanceAction::ScaleDown,
                _ => RiscVPerformanceAction::Continue,
            }
        }
    }
}

/// Platform-specific multi-core support
pub mod multicore {
    #[allow(unused_imports)]
    use super::constants;
    #[allow(unused_imports)]
    use crate::error::CRDTResult;

    /// Multi-core coordination trait for platforms that support it
    #[cfg(any(feature = "aurix", feature = "riscv"))]
    pub trait MultiCoreCRDT {
        /// Number of cores available
        fn core_count() -> u8 {
            constants::MAX_CORES
        }

        /// Distribute work across cores
        fn distribute_work(&mut self, core_mask: u8) -> CRDTResult<()>;

        /// Collect results from other cores
        fn collect_results(&mut self) -> CRDTResult<()>;

        /// Check if core-local operations are supported
        fn supports_core_local_ops() -> bool {
            constants::SUPPORTS_MULTICORE
        }
    }

    /// Single-core trait for platforms that don't support multi-core
    #[cfg(any(feature = "stm32", feature = "cortex-m"))]
    pub trait SingleCoreCRDT {
        /// Optimize for single-core performance
        fn optimize_single_core(&mut self) -> CRDTResult<()>;

        /// Check if single-core optimizations are available
        fn supports_single_core_opts() -> bool {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_constants() {
        // Test that constants are defined and reasonable
        assert!(constants::MAX_MERGE_CYCLES > 0);
        assert!(constants::MAX_INTERRUPT_LATENCY > 0);
        assert!(constants::CACHE_LINE_SIZE > 0);
        assert!(constants::MAX_CORES > 0);
        assert!(constants::MEMORY_ALIGNMENT > 0);
        assert!(!constants::PLATFORM_NAME.is_empty());
    }

    #[test]
    fn test_validation_constants() {
        // Test that validation constants are reasonable
        assert!(validation::MAX_ACTIVE_NODES > 0);
        assert!(validation::MAX_MEMORY_USAGE > 0);
    }

    #[test]
    fn test_platform_specific_values() {
        // Test platform-specific optimizations
        #[cfg(feature = "aurix")]
        {
            assert_eq!(constants::MAX_CORES, 3);
            assert_eq!(constants::MEMORY_ALIGNMENT, 32);
            assert!(constants::SUPPORTS_MULTICORE);
            assert_eq!(constants::PLATFORM_NAME, "AURIX");
        }

        #[cfg(feature = "stm32")]
        {
            assert_eq!(constants::MAX_CORES, 1);
            assert_eq!(constants::MEMORY_ALIGNMENT, 4);
            assert!(!constants::SUPPORTS_MULTICORE);
            assert_eq!(constants::PLATFORM_NAME, "STM32");
        }

        #[cfg(feature = "cortex-m")]
        {
            assert_eq!(constants::MAX_CORES, 1);
            assert_eq!(constants::MEMORY_ALIGNMENT, 4);
            assert!(!constants::SUPPORTS_MULTICORE);
            assert_eq!(constants::PLATFORM_NAME, "Cortex-M");
        }

        #[cfg(feature = "riscv")]
        {
            assert_eq!(constants::MAX_CORES, 8);
            assert_eq!(constants::MEMORY_ALIGNMENT, 8);
            assert!(constants::SUPPORTS_MULTICORE);
            assert_eq!(constants::PLATFORM_NAME, "RISC-V");
        }
    }

    #[cfg(feature = "aurix")]
    #[test]
    fn test_aurix_error_handling() {
        use crate::error::CRDTError;
        use error_handling::*;

        let action: AurixSafetyAction = CRDTError::BufferOverflow.into();
        assert_eq!(action, AurixSafetyAction::SafeState);

        let action: AurixSafetyAction = CRDTError::InvalidState.into();
        assert_eq!(action, AurixSafetyAction::SystemReset);

        let action: AurixSafetyAction = CRDTError::InvalidNodeId.into();
        assert_eq!(action, AurixSafetyAction::IsolateNode);
    }

    #[cfg(feature = "stm32")]
    #[test]
    fn test_stm32_error_handling() {
        use crate::error::CRDTError;
        use error_handling::*;

        let action: STM32PowerAction = CRDTError::BufferOverflow.into();
        assert_eq!(action, STM32PowerAction::ReduceFrequency);

        let action: STM32PowerAction = CRDTError::InvalidState.into();
        assert_eq!(action, STM32PowerAction::EnterStopMode);
    }
}
