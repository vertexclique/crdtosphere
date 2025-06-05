//! Platform-specific error types
//!
//! This module defines error types related to platform-specific operations
//! and hardware features across different embedded platforms.

/// Platform-specific error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformError {
    // AURIX-specific errors
    /// AURIX TriCore specific error
    TriCoreError(TriCoreError),
    /// AURIX ARM Cortex-R52 specific error
    CortexR52Error(CortexR52Error),
    /// AURIX multi-core synchronization error
    MultiCoreSyncError,
    /// AURIX memory protection unit error
    MPUError,

    // STM32-specific errors
    /// STM32 HAL error
    STM32HALError(STM32Error),
    /// STM32 DMA error
    DMAError,
    /// STM32 clock configuration error
    ClockConfigError,
    /// STM32 peripheral error
    PeripheralError,

    // ARM Cortex-M generic errors
    /// ARM Cortex-M specific error
    CortexMError(CortexMError),
    /// NVIC configuration error
    NVICError,
    /// SysTick configuration error
    SysTickError,
    /// Memory protection error
    MemoryProtectionError,

    // RISC-V specific errors
    /// RISC-V specific error
    RiscVError(RiscVError),
    /// RISC-V interrupt controller error
    InterruptControllerError,
    /// RISC-V timer error
    TimerError,
    /// RISC-V privilege level error
    PrivilegeLevelError,

    // Generic platform errors
    /// Hardware feature not available
    FeatureNotAvailable,
    /// Platform initialization failed
    InitializationFailed,
    /// Hardware abstraction layer error
    HALError,
    /// Platform configuration mismatch
    ConfigurationMismatch,
}

/// AURIX TriCore specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriCoreError {
    /// Instruction cache error
    ICacheError,
    /// Data cache error
    DCacheError,
    /// Core local memory error
    CLMError,
    /// Peripheral control processor error
    PCPError,
    /// Safety management unit error
    SMUError,
}

/// AURIX ARM Cortex-R52 specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CortexR52Error {
    /// Tightly coupled memory error
    TCMError,
    /// Error correction code error
    ECCError,
    /// Floating point unit error
    FPUError,
    /// Advanced SIMD error
    SIMDError,
    /// Generic interrupt controller error
    GICError,
}

/// STM32 specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum STM32Error {
    /// RCC (Reset and Clock Control) error
    RCCError,
    /// GPIO error
    GPIOError,
    /// Timer error
    TimerError,
    /// UART error
    UARTError,
    /// SPI error
    SPIError,
    /// I2C error
    I2CError,
    /// ADC error
    ADCError,
    /// CRC unit error
    CRCError,
}

/// ARM Cortex-M generic errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CortexMError {
    /// Memory management fault
    MemoryManagementFault,
    /// Bus fault
    BusFault,
    /// Usage fault
    UsageFault,
    /// Hard fault
    HardFault,
    /// Debug monitor fault
    DebugMonitorFault,
    /// PendSV fault
    PendSVFault,
}

/// RISC-V specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiscVError {
    /// Instruction address misaligned
    InstructionAddressMisaligned,
    /// Instruction access fault
    InstructionAccessFault,
    /// Illegal instruction
    IllegalInstruction,
    /// Load address misaligned
    LoadAddressMisaligned,
    /// Load access fault
    LoadAccessFault,
    /// Store address misaligned
    StoreAddressMisaligned,
    /// Store access fault
    StoreAccessFault,
    /// Environment call
    EnvironmentCall,
    /// Machine mode external interrupt
    MachineExternalInterrupt,
}

impl PlatformError {
    /// Returns the platform this error belongs to
    pub const fn platform(&self) -> &'static str {
        match self {
            Self::TriCoreError(_)
            | Self::CortexR52Error(_)
            | Self::MultiCoreSyncError
            | Self::MPUError => "AURIX",

            Self::STM32HALError(_)
            | Self::DMAError
            | Self::ClockConfigError
            | Self::PeripheralError => "STM32",

            Self::CortexMError(_)
            | Self::NVICError
            | Self::SysTickError
            | Self::MemoryProtectionError => "ARM Cortex-M",

            Self::RiscVError(_)
            | Self::InterruptControllerError
            | Self::TimerError
            | Self::PrivilegeLevelError => "RISC-V",

            Self::FeatureNotAvailable
            | Self::InitializationFailed
            | Self::HALError
            | Self::ConfigurationMismatch => "Generic",
        }
    }

    /// Returns true if this is a critical platform error
    pub const fn is_critical(&self) -> bool {
        match self {
            Self::TriCoreError(TriCoreError::SMUError)
            | Self::CortexR52Error(CortexR52Error::ECCError)
            | Self::CortexMError(CortexMError::HardFault)
            | Self::CortexMError(CortexMError::MemoryManagementFault)
            | Self::RiscVError(RiscVError::InstructionAccessFault)
            | Self::RiscVError(RiscVError::LoadAccessFault)
            | Self::RiscVError(RiscVError::StoreAccessFault)
            | Self::MultiCoreSyncError
            | Self::MPUError
            | Self::MemoryProtectionError
            | Self::InitializationFailed => true,
            _ => false,
        }
    }

    /// Returns true if this error is recoverable
    pub const fn is_recoverable(&self) -> bool {
        match self {
            // Non-recoverable critical errors
            Self::TriCoreError(TriCoreError::SMUError)
            | Self::CortexMError(CortexMError::HardFault)
            | Self::InitializationFailed => false,

            // Potentially recoverable errors
            Self::TriCoreError(_)
            | Self::CortexR52Error(_)
            | Self::MultiCoreSyncError
            | Self::MPUError
            | Self::STM32HALError(_)
            | Self::DMAError
            | Self::ClockConfigError
            | Self::PeripheralError
            | Self::CortexMError(_)
            | Self::NVICError
            | Self::SysTickError
            | Self::MemoryProtectionError
            | Self::RiscVError(_)
            | Self::InterruptControllerError
            | Self::TimerError
            | Self::PrivilegeLevelError
            | Self::FeatureNotAvailable
            | Self::HALError
            | Self::ConfigurationMismatch => true,
        }
    }

    /// Returns the error category
    pub const fn category(&self) -> &'static str {
        match self {
            Self::TriCoreError(_)
            | Self::CortexR52Error(_)
            | Self::CortexMError(_)
            | Self::RiscVError(_) => "CPU",

            Self::STM32HALError(_) | Self::DMAError | Self::PeripheralError => "Peripheral",

            Self::MultiCoreSyncError | Self::NVICError | Self::InterruptControllerError => {
                "Interrupt"
            }

            Self::MPUError | Self::MemoryProtectionError => "Memory",

            Self::ClockConfigError | Self::SysTickError | Self::TimerError => "Timing",

            Self::FeatureNotAvailable
            | Self::HALError
            | Self::ConfigurationMismatch
            | Self::InitializationFailed => "Configuration",

            Self::PrivilegeLevelError => "Security",
        }
    }
}

impl From<TriCoreError> for PlatformError {
    fn from(error: TriCoreError) -> Self {
        Self::TriCoreError(error)
    }
}

impl From<CortexR52Error> for PlatformError {
    fn from(error: CortexR52Error) -> Self {
        Self::CortexR52Error(error)
    }
}

impl From<STM32Error> for PlatformError {
    fn from(error: STM32Error) -> Self {
        Self::STM32HALError(error)
    }
}

impl From<CortexMError> for PlatformError {
    fn from(error: CortexMError) -> Self {
        Self::CortexMError(error)
    }
}

impl From<RiscVError> for PlatformError {
    fn from(error: RiscVError) -> Self {
        Self::RiscVError(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_identification() {
        assert_eq!(
            PlatformError::TriCoreError(TriCoreError::ICacheError).platform(),
            "AURIX"
        );
        assert_eq!(
            PlatformError::STM32HALError(STM32Error::RCCError).platform(),
            "STM32"
        );
        assert_eq!(
            PlatformError::CortexMError(CortexMError::HardFault).platform(),
            "ARM Cortex-M"
        );
        assert_eq!(
            PlatformError::RiscVError(RiscVError::IllegalInstruction).platform(),
            "RISC-V"
        );
        assert_eq!(PlatformError::FeatureNotAvailable.platform(), "Generic");
    }

    #[test]
    fn test_error_criticality() {
        assert!(PlatformError::TriCoreError(TriCoreError::SMUError).is_critical());
        assert!(PlatformError::CortexMError(CortexMError::HardFault).is_critical());
        assert!(!PlatformError::STM32HALError(STM32Error::GPIOError).is_critical());
        assert!(!PlatformError::FeatureNotAvailable.is_critical());
    }

    #[test]
    fn test_error_recoverability() {
        assert!(!PlatformError::CortexMError(CortexMError::HardFault).is_recoverable());
        assert!(!PlatformError::InitializationFailed.is_recoverable());
        assert!(PlatformError::STM32HALError(STM32Error::UARTError).is_recoverable());
        assert!(PlatformError::FeatureNotAvailable.is_recoverable());
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(
            PlatformError::CortexMError(CortexMError::HardFault).category(),
            "CPU"
        );
        assert_eq!(
            PlatformError::STM32HALError(STM32Error::UARTError).category(),
            "Peripheral"
        );
        assert_eq!(PlatformError::NVICError.category(), "Interrupt");
        assert_eq!(PlatformError::MPUError.category(), "Memory");
        assert_eq!(PlatformError::TimerError.category(), "Timing");
        assert_eq!(
            PlatformError::FeatureNotAvailable.category(),
            "Configuration"
        );
        assert_eq!(PlatformError::PrivilegeLevelError.category(), "Security");
    }

    #[test]
    fn test_error_conversions() {
        let tricore_error: PlatformError = TriCoreError::ICacheError.into();
        assert_eq!(
            tricore_error,
            PlatformError::TriCoreError(TriCoreError::ICacheError)
        );

        let stm32_error: PlatformError = STM32Error::RCCError.into();
        assert_eq!(
            stm32_error,
            PlatformError::STM32HALError(STM32Error::RCCError)
        );

        let cortex_m_error: PlatformError = CortexMError::HardFault.into();
        assert_eq!(
            cortex_m_error,
            PlatformError::CortexMError(CortexMError::HardFault)
        );

        let riscv_error: PlatformError = RiscVError::IllegalInstruction.into();
        assert_eq!(
            riscv_error,
            PlatformError::RiscVError(RiscVError::IllegalInstruction)
        );
    }
}
