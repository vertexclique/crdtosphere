//! Brake Control Unit (ECU) Implementation
//!
//! This binary implements the Brake ECU with ASIL-D safety level,
//! responsible for brake control, ABS, and emergency braking coordination
//! using memory-mapped CRDT processing.

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

/// Brake ECU with memory-mapped CRDT processing
struct BrakeECU {
    crdt_state: MemoryMappedCRDTState,
    cycle_count: u64,
    brake_temperature: f32,
    brake_pressure: u8,
    abs_active: bool,
    emergency_brake_active: bool,
}

impl BrakeECU {
    /// Creates a new Brake ECU
    fn new() -> Self {
        Self {
            crdt_state: MemoryMappedCRDTState::new(ECUNodeId::Brake),
            cycle_count: 0,
            brake_temperature: 30.0, // Start at ambient temperature
            brake_pressure: 0,
            abs_active: false,
            emergency_brake_active: false,
        }
    }
    
    /// Simulates brake operation and temperature generation
    fn simulate_brake_operation(&mut self) {
        if self.emergency_brake_active {
            // Emergency braking generates significant heat
            self.brake_temperature = (self.brake_temperature + 2.0).min(80.0);
            self.brake_pressure = 100;
            return;
        }
        
        // Simulate brake usage patterns
        let brake_usage = match self.cycle_count % 1200 {
            0..=100 => 0.8,    // Heavy braking period
            101..=200 => 0.0,  // No braking
            201..=300 => 0.4,  // Medium braking
            301..=500 => 0.1,  // Light braking
            501..=600 => 0.0,  // No braking
            _ => 0.2,          // Normal braking
        };
        
        // Update brake pressure
        self.brake_pressure = (brake_usage * 100.0) as u8;
        
        // Update brake temperature based on usage
        let heat_generation = brake_usage * 1.5;
        let cooling = 0.15; // Natural cooling
        
        self.brake_temperature += heat_generation - cooling;
        self.brake_temperature = self.brake_temperature.clamp(25.0, 80.0);
        
        // Simulate ABS activation during heavy braking
        self.abs_active = brake_usage > 0.6;
        
        // Add some random variation
        let variation = ((self.cycle_count % 23) as f32 - 11.0) * 0.05;
        self.brake_temperature += variation;
    }
    
    /// Writes brake data to memory-mapped input regions
    fn write_inputs_to_memory(&self) {
        unsafe {
            // Write temperature to input region
            ptr::write_volatile(
                memory_regions::TEMP_INPUT as *mut f32,
                self.brake_temperature
            );
            
            // Write emergency condition (emergency brake detection)
            let emergency_condition = if self.emergency_brake_active { 0x00000001 } else { 0x00000000 };
            ptr::write_volatile(
                memory_regions::EMERGENCY_INPUT as *mut u32,
                emergency_condition
            );
            
            // Write configuration data (brake-specific config)
            let config_data = ((self.brake_pressure as u32) << 16) | 
                             (if self.abs_active { 0x01 } else { 0x00 }) |
                             0x02; // Stability control enabled
            ptr::write_volatile(
                memory_regions::CONFIG_INPUT as *mut u32,
                config_data
            );
            
            // Write error condition (brake temperature fault detection)
            let error_condition = if self.brake_temperature > 75.0 || self.brake_temperature <= 0.0 {
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
            
            // Update brake state based on CRDT results
            if emergency_state != 0 {
                self.emergency_brake_active = true;
            }
            
            // Use fused temperature for brake control decisions
            if fused_temp > 70.0 {
                // Reduce braking effectiveness at high temperatures
                self.brake_pressure = (self.brake_pressure as f32 * 0.9) as u8;
            }
        }
    }
    
    /// Main execution cycle
    fn run_cycle(&mut self) -> Result<(), ECUError> {
        self.cycle_count += 1;
        
        // Simulate brake operation
        self.simulate_brake_operation();
        
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
    
    /// Logs current brake status
    fn log_status(&self) {
        // In a real system, this would go to a logging system
        // For simulation, we'll just track key metrics
        
        unsafe {
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            let emergency_state = ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32);
            let error_count = ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32);
            
            // Check for critical conditions
            if emergency_state != 0 {
                // Emergency braking active
            }
            
            if fused_temp > 70.0 {
                // High brake temperature warning
            }
            
            if self.emergency_brake_active {
                // Emergency brake status
            }
        }
    }
    
    /// Gets brake-specific diagnostics
    fn get_brake_diagnostics(&self) -> BrakeDiagnostics {
        unsafe {
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            let emergency_state = ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32);
            let error_count = ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32);
            
            BrakeDiagnostics {
                node_id: ECUNodeId::Brake,
                brake_temperature: self.brake_temperature,
                fused_temperature: fused_temp,
                brake_pressure: self.brake_pressure,
                abs_active: self.abs_active,
                emergency_brake_active: self.emergency_brake_active,
                emergency_state,
                error_count,
                cycle_count: self.cycle_count,
            }
        }
    }
}

/// Brake-specific diagnostic information
#[derive(Debug, Clone)]
struct BrakeDiagnostics {
    node_id: ECUNodeId,
    brake_temperature: f32,
    fused_temperature: f32,
    brake_pressure: u8,
    abs_active: bool,
    emergency_brake_active: bool,
    emergency_state: u32,
    error_count: u32,
    cycle_count: u64,
}

/// Global Brake ECU instance for simulation interface
static mut BRAKE_ECU: Option<BrakeECU> = None;

/// Main entry point for Brake ECU
#[entry]
fn main() -> ! {
    // Initialize Brake ECU
    unsafe {
        BRAKE_ECU = Some(BrakeECU::new());
    }
    
    // Main execution loop
    loop {
        unsafe {
            if let Some(ref mut brake_ecu) = BRAKE_ECU {
                match brake_ecu.run_cycle() {
                    Ok(_) => {
                        // Cycle completed successfully
                        // Prevent optimization by using volatile operations
                        ptr::write_volatile(&mut brake_ecu.cycle_count as *mut u64, brake_ecu.cycle_count);
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
pub extern "C" fn brake_ecu_simulation_step() -> u32 {
    unsafe {
        if BRAKE_ECU.is_none() {
            BRAKE_ECU = Some(BrakeECU::new());
        }
        
        if let Some(ref mut ecu) = BRAKE_ECU {
            let _ = ecu.run_cycle();
            
            // Return fused temperature from CRDT processing
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            (fused_temp * 100.0) as u32
        } else {
            3000 // Default temperature (30.0°C)
        }
    }
}

/// Get brake diagnostics for Renode monitoring
#[no_mangle]
pub extern "C" fn brake_ecu_get_diagnostics() -> *const BrakeDiagnostics {
    static mut DIAGNOSTICS: Option<BrakeDiagnostics> = None;
    
    unsafe {
        if let Some(ref ecu) = BRAKE_ECU {
            DIAGNOSTICS = Some(ecu.get_brake_diagnostics());
            if let Some(ref diag) = DIAGNOSTICS {
                diag as *const BrakeDiagnostics
            } else {
                core::ptr::null()
            }
        } else {
            core::ptr::null()
        }
    }
}

/// Inject emergency brake scenario for testing
#[no_mangle]
pub extern "C" fn brake_ecu_inject_emergency() -> bool {
    unsafe {
        if let Some(ref mut ecu) = BRAKE_ECU {
            // Inject emergency brake condition
            ecu.emergency_brake_active = true;
            
            // Write to memory-mapped input region
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

/// Inject brake temperature for testing
#[no_mangle]
pub extern "C" fn brake_ecu_inject_temperature(temperature: f32) -> bool {
    unsafe {
        if let Some(ref mut ecu) = BRAKE_ECU {
            ecu.brake_temperature = temperature;
            
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
pub extern "C" fn brake_ecu_get_crdt_state() -> CRDTStateSnapshot {
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
