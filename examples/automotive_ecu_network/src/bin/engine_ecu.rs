//! Engine Control Unit (ECU) Implementation
//!
//! This binary implements the Engine ECU with ASIL-D safety level,
//! responsible for engine control, temperature monitoring, and
//! emergency response coordination using memory-mapped CRDT processing.

#![no_std]
#![no_main]

use panic_halt as _;
use cortex_m_rt::entry;
use core::ptr;
use linked_list_allocator::LockedHeap;
use stm32f4xx_hal as _; // Provides interrupt vectors


#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

use automotive_ecu_network::memory_mapped_crdt::{
    MemoryMappedCRDTState, run_crdt_processing_cycle, memory_regions
};
use automotive_ecu_network::{ECUNodeId, ECUError};

/// Engine ECU with memory-mapped CRDT processing
struct EngineECU {
    crdt_state: MemoryMappedCRDTState,
    cycle_count: u64,
    engine_temperature: f32,
    engine_rpm: u16,
    emergency_shutdown: bool,
}

impl EngineECU {
    /// Creates a new Engine ECU
    fn new() -> Self {
        Self {
            crdt_state: MemoryMappedCRDTState::new(ECUNodeId::Engine),
            cycle_count: 0,
            engine_temperature: 85.0, // Start at normal operating temperature
            engine_rpm: 800,           // Idle RPM
            emergency_shutdown: false,
        }
    }
    
    /// Simulates engine operation and temperature generation
    fn simulate_engine_operation(&mut self) {
        if self.emergency_shutdown {
            // Engine is shut down - temperature decreases
            self.engine_temperature = (self.engine_temperature - 0.5).max(25.0);
            self.engine_rpm = 0;
            return;
        }
        
        // Simulate engine load and temperature variation
        let load_factor = match self.cycle_count % 1000 {
            0..=200 => 0.3,    // Light load
            201..=600 => 0.7,  // Medium load
            601..=800 => 1.0,  // Heavy load
            _ => 0.5,          // Variable load
        };
        
        // Update RPM based on load
        self.engine_rpm = (800.0 + load_factor * 5200.0) as u16;
        
        // Update temperature based on load and cooling
        let target_temp = 80.0 + load_factor * 20.0; // 80°C to 100°C (realistic engine temps)
        let temp_change = (target_temp - self.engine_temperature) * 0.05; // Slower temperature changes
        self.engine_temperature += temp_change;
        
        // Add some random variation (smaller range)
        let variation = ((self.cycle_count % 37) as f32 - 18.0) * 0.05;
        self.engine_temperature += variation;
        
        // Simulate occasional temperature spikes (every 2000 cycles)
        if self.cycle_count % 2000 == 1500 {
            self.engine_temperature += 8.0; // Smaller temperature spike
        }
        
        // Clamp temperature to realistic engine operating bounds
        self.engine_temperature = self.engine_temperature.clamp(75.0, 120.0); // Never below 75°C when running
    }
    
    /// Writes engine data to memory-mapped input regions
    fn write_inputs_to_memory(&self) {
        unsafe {
            // Write temperature to input region
            ptr::write_volatile(
                memory_regions::TEMP_INPUT as *mut f32,
                self.engine_temperature
            );
            
            // Write emergency condition (critical temperature detection)
            let emergency_condition = if self.engine_temperature > 110.0 { 0x00000001 } else { 0x00000000 };
            ptr::write_volatile(
                memory_regions::EMERGENCY_INPUT as *mut u32,
                emergency_condition
            );
            
            // Write configuration data (engine-specific config)
            let config_data = ((self.engine_rpm as u32) << 16) | 0x03; // ABS + Stability enabled
            ptr::write_volatile(
                memory_regions::CONFIG_INPUT as *mut u32,
                config_data
            );
            
            // Write error condition (temperature sensor fault detection)
            let error_condition = if self.engine_temperature <= 0.0 || self.engine_temperature > 150.0 {
                0x00000001
            } else {
                0x00000000
            };
            ptr::write_volatile(
                memory_regions::ERROR_INPUT as *mut u32,
                error_condition
            );
        }
    }
    
    /// Reads CRDT results from memory-mapped output regions
    fn read_outputs_from_memory(&mut self) {
        unsafe {
            // Read fused temperature result
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            
            // Read emergency state
            let emergency_state = ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32);
            
            // Update engine state based on CRDT results
            if emergency_state != 0 {
                self.emergency_shutdown = true;
            }
            
            // Use fused temperature for engine control decisions
            if fused_temp > 110.0 {
                self.emergency_shutdown = true;
            }
        }
    }
    
    /// Main execution cycle
    fn run_cycle(&mut self) -> Result<(), ECUError> {
        self.cycle_count += 1;
        
        // Simulate engine operation
        self.simulate_engine_operation();
        
        // Write inputs to memory-mapped regions
        self.write_inputs_to_memory();
        
        // Run CRDT processing cycle
        run_crdt_processing_cycle(&mut self.crdt_state, self.cycle_count)?;
        
        // Read CRDT results from memory
        self.read_outputs_from_memory();
        
        // Log status every 100 cycles
        if self.cycle_count % 100 == 0 {
            self.log_status();
        }
        
        Ok(())
    }
    
    /// Logs current engine status
    fn log_status(&self) {
        // In a real system, this would go to a logging system
        // For simulation, we'll just track key metrics
        
        unsafe {
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            let emergency_state = ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32);
            let error_count = ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32);
            
            // Check for critical conditions
            if emergency_state != 0 {
                // Emergency state detected
            }
            
            if fused_temp > 100.0 {
                // High temperature warning
            }
            
            if self.emergency_shutdown {
                // Engine shutdown status
            }
        }
    }
    
    /// Gets engine-specific diagnostics
    fn get_engine_diagnostics(&self) -> EngineDiagnostics {
        unsafe {
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            let emergency_state = ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32);
            let error_count = ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32);
            
            EngineDiagnostics {
                node_id: ECUNodeId::Engine,
                engine_temperature: self.engine_temperature,
                fused_temperature: fused_temp,
                engine_rpm: self.engine_rpm,
                emergency_shutdown: self.emergency_shutdown,
                emergency_state,
                error_count,
                cycle_count: self.cycle_count,
            }
        }
    }
}

/// Engine-specific diagnostic information
#[derive(Debug, Clone)]
struct EngineDiagnostics {
    node_id: ECUNodeId,
    engine_temperature: f32,
    fused_temperature: f32,
    engine_rpm: u16,
    emergency_shutdown: bool,
    emergency_state: u32,
    error_count: u32,
    cycle_count: u64,
}

/// Global Engine ECU instance for simulation interface
static mut ENGINE_ECU: Option<EngineECU> = None;

/// Main entry point for Engine ECU
#[entry]
fn main() -> ! {
    // Initialize Engine ECU
    unsafe {
        ENGINE_ECU = Some(EngineECU::new());
    }
    
    // Main execution loop
    loop {
        unsafe {
            if let Some(ref mut engine_ecu) = ENGINE_ECU {
                match engine_ecu.run_cycle() {
                    Ok(_) => {
                        // Cycle completed successfully
                        // Prevent optimization by using volatile operations
                        ptr::write_volatile(&mut engine_ecu.cycle_count as *mut u64, engine_ecu.cycle_count);
                    }
                    Err(_e) => {
                        // Handle error - in a real system, this would trigger fault handling
                        // For simulation, we'll continue
                    }
                }
            }
        }
        
        // Simulate cycle timing (in real system, this would be interrupt-driven)
        cortex_m::asm::delay(1000); // Simple delay for simulation
        
        // Prevent infinite loop optimization
        cortex_m::asm::nop();
    }
}

/// Simulation interface for Renode
#[no_mangle]
pub extern "C" fn engine_ecu_simulation_step() -> u32 {
    unsafe {
        if ENGINE_ECU.is_none() {
            ENGINE_ECU = Some(EngineECU::new());
        }
        
        if let Some(ref mut ecu) = ENGINE_ECU {
            let _ = ecu.run_cycle();
            
            // Return fused temperature from CRDT processing
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            (fused_temp * 100.0) as u32
        } else {
            7500 // Default temperature (75.0°C)
        }
    }
}

/// Get engine diagnostics for Renode monitoring
#[no_mangle]
pub extern "C" fn engine_ecu_get_diagnostics() -> *const EngineDiagnostics {
    static mut DIAGNOSTICS: Option<EngineDiagnostics> = None;
    
    unsafe {
        if let Some(ref ecu) = ENGINE_ECU {
            DIAGNOSTICS = Some(ecu.get_engine_diagnostics());
            if let Some(ref diag) = DIAGNOSTICS {
                diag as *const EngineDiagnostics
            } else {
                core::ptr::null()
            }
        } else {
            core::ptr::null()
        }
    }
}

/// Inject emergency scenario for testing
#[no_mangle]
pub extern "C" fn engine_ecu_inject_emergency() -> bool {
    unsafe {
        if let Some(ref mut ecu) = ENGINE_ECU {
            // Inject critical temperature
            ecu.engine_temperature = 115.0;
            
            // Write to memory-mapped input region
            ptr::write_volatile(
                memory_regions::TEMP_INPUT as *mut f32,
                115.0
            );
            ptr::write_volatile(
                memory_regions::EMERGENCY_INPUT as *mut u32,
                0x00000001
            );
            
            true
        } else {
            false
        }
    }
}

/// Inject temperature for testing
#[no_mangle]
pub extern "C" fn engine_ecu_inject_temperature(temperature: f32) -> bool {
    unsafe {
        if let Some(ref mut ecu) = ENGINE_ECU {
            ecu.engine_temperature = temperature;
            
            // Write to memory-mapped input region
            ptr::write_volatile(
                memory_regions::TEMP_INPUT as *mut f32,
                temperature
            );
            
            true
        } else {
            false
        }
    }
}

/// Get current CRDT state for monitoring
#[no_mangle]
pub extern "C" fn engine_ecu_get_crdt_state() -> CRDTStateSnapshot {
    unsafe {
        CRDTStateSnapshot {
            fused_temperature: ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32),
            emergency_state: ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32),
            emergency_flag: ptr::read_volatile((memory_regions::EMERGENCY_OUTPUT + 4) as *const u32),
            config_state: ptr::read_volatile(memory_regions::CONFIG_OUTPUT as *const u32),
            config_timestamp: ptr::read_volatile((memory_regions::CONFIG_OUTPUT + 4) as *const u32),
            error_count: ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32),
        }
    }
}

/// CRDT state snapshot for monitoring
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CRDTStateSnapshot {
    pub fused_temperature: f32,
    pub emergency_state: u32,
    pub emergency_flag: u32,
    pub config_state: u32,
    pub config_timestamp: u32,
    pub error_count: u32,
}
