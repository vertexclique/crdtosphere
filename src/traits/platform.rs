//! Platform CRDT trait definition
//!
//! This module defines traits for CRDTs that are optimized for specific
//! embedded platforms and hardware architectures.

use crate::error::CRDTResult;
use crate::memory::MemoryConfig;
use crate::traits::CRDT;

/// Trait for CRDTs that are platform-aware
///
/// This trait provides platform-specific optimizations and ensures
/// compatibility with different embedded hardware architectures.
pub trait PlatformCRDT<C: MemoryConfig>: CRDT<C> {
    /// The platform this CRDT is optimized for
    type Platform: Platform;

    /// Returns the target platform
    fn target_platform() -> Self::Platform;

    /// Performs platform-specific initialization
    fn platform_init(&mut self) -> CRDTResult<()>;

    /// Performs platform-specific cleanup
    fn platform_cleanup(&mut self) -> CRDTResult<()>;

    /// Returns platform-specific capabilities
    fn platform_capabilities(&self) -> PlatformCapabilities;

    /// Checks if the current platform is supported
    fn is_platform_supported() -> bool;

    /// Returns platform-specific memory alignment requirements
    fn platform_alignment() -> usize {
        Self::Platform::memory_alignment()
    }

    /// Returns platform-specific cache line size
    fn platform_cache_line_size() -> usize {
        Self::Platform::cache_line_size()
    }

    /// Performs platform-optimized merge
    fn platform_merge(&mut self, other: &Self) -> CRDTResult<()>;
}

/// Platform trait for different embedded platforms
pub trait Platform {
    /// Platform name
    const NAME: &'static str;

    /// CPU architecture
    const ARCHITECTURE: Architecture;

    /// Memory alignment requirement
    fn memory_alignment() -> usize;

    /// Cache line size
    fn cache_line_size() -> usize;

    /// Maximum interrupt latency in CPU cycles
    fn max_interrupt_latency() -> u32;

    /// Supports atomic operations
    fn supports_atomics() -> bool;

    /// Supports floating point operations
    fn supports_fpu() -> bool;

    /// Supports SIMD operations
    fn supports_simd() -> bool;

    /// Returns platform-specific features
    fn features() -> PlatformFeatures;
}

/// CPU architectures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    /// ARM Cortex-M series
    CortexM,
    /// ARM Cortex-R series
    CortexR,
    /// AURIX TriCore
    TriCore,
    /// RISC-V
    RiscV,
    /// x86/x64 (for testing)
    X86,
}

/// Platform capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapabilities {
    /// Supports hardware acceleration
    pub hardware_acceleration: bool,
    /// Supports DMA operations
    pub dma_support: bool,
    /// Supports memory protection
    pub memory_protection: bool,
    /// Supports real-time guarantees
    pub realtime_support: bool,
    /// Supports multi-core operations
    pub multicore_support: bool,
    /// Maximum number of cores
    pub max_cores: u8,
}

/// Platform-specific features
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformFeatures {
    /// Atomic operations support
    pub atomics: bool,
    /// Floating point unit
    pub fpu: bool,
    /// SIMD instructions
    pub simd: bool,
    /// Hardware CRC
    pub hardware_crc: bool,
    /// Hardware encryption
    pub hardware_crypto: bool,
    /// Memory management unit
    pub mmu: bool,
    /// Cache coherency
    pub cache_coherent: bool,
}

/// AURIX platform implementation
pub struct AurixPlatform;

impl Platform for AurixPlatform {
    const NAME: &'static str = "AURIX";
    const ARCHITECTURE: Architecture = Architecture::TriCore;

    fn memory_alignment() -> usize {
        4
    }
    fn cache_line_size() -> usize {
        32
    }
    fn max_interrupt_latency() -> u32 {
        100
    }
    fn supports_atomics() -> bool {
        true
    }
    fn supports_fpu() -> bool {
        true
    }
    fn supports_simd() -> bool {
        false
    }

    fn features() -> PlatformFeatures {
        PlatformFeatures {
            atomics: true,
            fpu: true,
            simd: false,
            hardware_crc: true,
            hardware_crypto: true,
            mmu: true,
            cache_coherent: true,
        }
    }
}

/// STM32 platform implementation
pub struct STM32Platform;

impl Platform for STM32Platform {
    const NAME: &'static str = "STM32";
    const ARCHITECTURE: Architecture = Architecture::CortexM;

    fn memory_alignment() -> usize {
        4
    }
    fn cache_line_size() -> usize {
        32
    }
    fn max_interrupt_latency() -> u32 {
        50
    }
    fn supports_atomics() -> bool {
        true
    }
    fn supports_fpu() -> bool {
        true
    }
    fn supports_simd() -> bool {
        false
    }

    fn features() -> PlatformFeatures {
        PlatformFeatures {
            atomics: true,
            fpu: true,
            simd: false,
            hardware_crc: true,
            hardware_crypto: false,
            mmu: false,
            cache_coherent: false,
        }
    }
}

/// ARM Cortex-M platform implementation
pub struct CortexMPlatform;

impl Platform for CortexMPlatform {
    const NAME: &'static str = "Cortex-M";
    const ARCHITECTURE: Architecture = Architecture::CortexM;

    fn memory_alignment() -> usize {
        4
    }
    fn cache_line_size() -> usize {
        32
    }
    fn max_interrupt_latency() -> u32 {
        25
    }
    fn supports_atomics() -> bool {
        true
    }
    fn supports_fpu() -> bool {
        false
    }
    fn supports_simd() -> bool {
        false
    }

    fn features() -> PlatformFeatures {
        PlatformFeatures {
            atomics: true,
            fpu: false,
            simd: false,
            hardware_crc: false,
            hardware_crypto: false,
            mmu: false,
            cache_coherent: false,
        }
    }
}

/// RISC-V platform implementation
pub struct RiscVPlatform;

impl Platform for RiscVPlatform {
    const NAME: &'static str = "RISC-V";
    const ARCHITECTURE: Architecture = Architecture::RiscV;

    fn memory_alignment() -> usize {
        4
    }
    fn cache_line_size() -> usize {
        64
    }
    fn max_interrupt_latency() -> u32 {
        30
    }
    fn supports_atomics() -> bool {
        true
    }
    fn supports_fpu() -> bool {
        true
    }
    fn supports_simd() -> bool {
        true
    }

    fn features() -> PlatformFeatures {
        PlatformFeatures {
            atomics: true,
            fpu: true,
            simd: true,
            hardware_crc: false,
            hardware_crypto: false,
            mmu: true,
            cache_coherent: true,
        }
    }
}

/// Trait for platform-specific optimizations
pub trait PlatformOptimized<C: MemoryConfig, P: Platform>: PlatformCRDT<C> {
    /// Performs platform-optimized serialization
    fn optimized_serialize(&self, buffer: &mut [u8]) -> CRDTResult<usize>;

    /// Performs platform-optimized deserialization
    fn optimized_deserialize(buffer: &[u8]) -> CRDTResult<Self>
    where
        Self: Sized;

    /// Uses platform-specific instructions for hash computation
    fn platform_hash(&self) -> u32;

    /// Uses platform-specific memory operations
    fn platform_memcpy(&mut self, src: &Self) -> CRDTResult<()>;

    /// Returns platform-specific performance metrics
    fn performance_metrics(&self) -> PerformanceMetrics;
}

/// Performance metrics for platform optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceMetrics {
    /// Merge operation cycles
    pub merge_cycles: u32,
    /// Serialization cycles
    pub serialize_cycles: u32,
    /// Hash computation cycles
    pub hash_cycles: u32,
    /// Memory copy cycles
    pub memcpy_cycles: u32,
    /// Cache misses
    pub cache_misses: u32,
}

/// Trait for multi-core platform support
pub trait MultiCorePlatform<C: MemoryConfig>: PlatformCRDT<C> {
    /// Number of cores available
    fn core_count() -> u8;

    /// Current core ID
    fn current_core_id() -> u8;

    /// Performs inter-core synchronization
    fn sync_cores(&mut self) -> CRDTResult<()>;

    /// Distributes work across cores
    fn distribute_work(&mut self, core_mask: u8) -> CRDTResult<()>;

    /// Collects results from other cores
    fn collect_results(&mut self) -> CRDTResult<()>;

    /// Checks if core-local operations are supported
    fn supports_core_local_ops() -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CRDTError;
    use crate::memory::DefaultConfig;

    // Mock platform CRDT for testing
    struct MockPlatformCRDT {
        value: u32,
        initialized: bool,
    }

    impl MockPlatformCRDT {
        fn new() -> Self {
            Self {
                value: 0,
                initialized: false,
            }
        }
    }

    impl CRDT<DefaultConfig> for MockPlatformCRDT {
        type Error = CRDTError;

        fn merge(&mut self, other: &Self) -> CRDTResult<()> {
            self.value = self.value.max(other.value);
            Ok(())
        }

        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }

        fn size_bytes(&self) -> usize {
            core::mem::size_of::<Self>()
        }

        fn validate(&self) -> CRDTResult<()> {
            Ok(())
        }

        fn state_hash(&self) -> u32 {
            self.value
        }

        fn can_merge(&self, _other: &Self) -> bool {
            true
        }
    }

    impl PlatformCRDT<DefaultConfig> for MockPlatformCRDT {
        type Platform = STM32Platform;

        fn target_platform() -> Self::Platform {
            STM32Platform
        }

        fn platform_init(&mut self) -> CRDTResult<()> {
            self.initialized = true;
            Ok(())
        }

        fn platform_cleanup(&mut self) -> CRDTResult<()> {
            self.initialized = false;
            Ok(())
        }

        fn platform_capabilities(&self) -> PlatformCapabilities {
            PlatformCapabilities {
                hardware_acceleration: false,
                dma_support: true,
                memory_protection: false,
                realtime_support: true,
                multicore_support: false,
                max_cores: 1,
            }
        }

        fn is_platform_supported() -> bool {
            true
        }

        fn platform_merge(&mut self, other: &Self) -> CRDTResult<()> {
            // Platform-optimized merge (same as regular merge for this example)
            self.merge(other)
        }
    }

    #[test]
    fn test_platform_features() {
        assert_eq!(STM32Platform::NAME, "STM32");
        assert_eq!(STM32Platform::ARCHITECTURE, Architecture::CortexM);
        assert_eq!(STM32Platform::memory_alignment(), 4);
        assert_eq!(STM32Platform::cache_line_size(), 32);
        assert!(STM32Platform::supports_atomics());
        assert!(STM32Platform::supports_fpu());
        assert!(!STM32Platform::supports_simd());

        let features = STM32Platform::features();
        assert!(features.atomics);
        assert!(features.fpu);
        assert!(!features.simd);
        assert!(features.hardware_crc);
        assert!(!features.hardware_crypto);
    }

    #[test]
    fn test_platform_crdt() {
        let mut crdt = MockPlatformCRDT::new();

        assert!(!crdt.initialized);
        assert!(MockPlatformCRDT::is_platform_supported());

        assert!(crdt.platform_init().is_ok());
        assert!(crdt.initialized);

        let capabilities = crdt.platform_capabilities();
        assert!(capabilities.dma_support);
        assert!(capabilities.realtime_support);
        assert!(!capabilities.multicore_support);
        assert_eq!(capabilities.max_cores, 1);

        assert!(crdt.platform_cleanup().is_ok());
        assert!(!crdt.initialized);
    }

    #[test]
    fn test_platform_alignment() {
        assert_eq!(MockPlatformCRDT::platform_alignment(), 4);
        assert_eq!(MockPlatformCRDT::platform_cache_line_size(), 32);
    }

    #[test]
    fn test_architecture_types() {
        assert_eq!(AurixPlatform::ARCHITECTURE, Architecture::TriCore);
        assert_eq!(STM32Platform::ARCHITECTURE, Architecture::CortexM);
        assert_eq!(CortexMPlatform::ARCHITECTURE, Architecture::CortexM);
        assert_eq!(RiscVPlatform::ARCHITECTURE, Architecture::RiscV);
    }
}
