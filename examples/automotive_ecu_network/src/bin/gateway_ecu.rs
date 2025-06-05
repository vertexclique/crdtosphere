//! Gateway ECU Implementation
//!
//! This binary implements the Gateway ECU with ASIL-B safety level,
//! responsible for central coordination, diagnostics, and system monitoring
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
use automotive_ecu_network::{ECUNodeId, ECUError, SystemStatus};

/// Gateway ECU with memory-mapped CRDT processing
struct GatewayECU {
    crdt_state: MemoryMappedCRDTState,
    cycle_count: u64,
    gateway_temperature: f32,
    system_health_score: u8,
    diagnostic_mode: bool,
    message_routing_count: u64,
}

impl GatewayECU {
    /// Creates a new Gateway ECU
    fn new() -> Self {
        Self {
            crdt_state: MemoryMappedCRDTState::new(ECUNodeId::Gateway),
            cycle_count: 0,
            gateway_temperature: 40.0, // Start at ambient temperature
            system_health_score: 100,
            diagnostic_mode: false,
            message_routing_count: 0,
        }
    }
    
    /// Simulates gateway operation and temperature generation
    fn simulate_gateway_operation(&mut self) {
        // Update gateway temperature based on processing load
        let processing_load = (self.message_routing_count % 100) as f32 / 100.0;
        let heat_generation = processing_load * 0.3;
        let cooling = 0.05; // Natural cooling
        
        self.gateway_temperature += heat_generation - cooling;
        self.gateway_temperature = self.gateway_temperature.clamp(25.0, 55.0);
        
        // Simulate message routing
        self.message_routing_count += 1;
        
        // Calculate system health score based on CRDT state
        self.calculate_system_health();
        
        // Add some random variation
        let variation = ((self.cycle_count % 37) as f32 - 18.0) * 0.02;
        self.gateway_temperature += variation;
    }
    
    /// Calculates system health score based on CRDT outputs
    fn calculate_system_health(&mut self) {
        unsafe {
            let mut health_score = 100u8;
            
            // Check fused temperature
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            if fused_temp > 100.0 {
                health_score = health_score.saturating_sub(30);
            } else if fused_temp > 80.0 {
                health_score = health_score.saturating_sub(15);
            }
            
            // Check emergency state
            let emergency_state = ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32);
            if emergency_state != 0 {
                health_score = health_score.saturating_sub(25);
            }
            
            // Check error count
            let error_count = ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32);
            if error_count > 10 {
                health_score = health_score.saturating_sub(20);
            } else if error_count > 5 {
                health_score = health_score.saturating_sub(10);
            }
            
            self.system_health_score = health_score;
            self.diagnostic_mode = health_score < 70;
        }
    }
    
    /// Writes gateway data to memory-mapped input regions
    fn write_inputs_to_memory(&self) {
        unsafe {
            // Write temperature to input region
            ptr::write_volatile(
                memory_regions::TEMP_INPUT as *mut f32,
                self.gateway_temperature
            );
            
            // Write emergency condition (diagnostic mode activation)
            let emergency_condition = if self.diagnostic_mode { 0x00000001 } else { 0x00000000 };
            ptr::write_volatile(
                memory_regions::EMERGENCY_INPUT as *mut u32,
                emergency_condition
            );
            
            // Write configuration data (gateway-specific config)
            let config_data = ((self.system_health_score as u32) << 16) | 
                             (if self.diagnostic_mode { 0x04 } else { 0x00 }) |
                             0x03; // ABS + Stability enabled
            ptr::write_volatile(
                memory_regions::CONFIG_INPUT as *mut u32,
                config_data
            );
            
            // Write error condition (gateway temperature fault detection)
            let error_condition = if self.gateway_temperature > 50.0 || self.gateway_temperature <= 0.0 {
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
            
            // Update gateway state based on CRDT results
            if emergency_state != 0 {
                self.diagnostic_mode = true;
            }
            
            // Use fused temperature for system monitoring
            if fused_temp > 90.0 {
                // System overheating - enter diagnostic mode
                self.diagnostic_mode = true;
            }
        }
    }
    
    /// Main execution cycle
    fn run_cycle(&mut self) -> Result<(), ECUError> {
        self.cycle_count += 1;
        
        // Simulate gateway operation
        self.simulate_gateway_operation();
        
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
    
    /// Logs current gateway status
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
            
            if fused_temp > 90.0 {
                // System overheating warning
            }
            
            if self.diagnostic_mode {
                // Diagnostic mode active
            }
        }
    }
    
    /// Gets gateway-specific diagnostics
    fn get_gateway_diagnostics(&self) -> GatewayDiagnostics {
        unsafe {
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            let emergency_state = ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32);
            let error_count = ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32);
            
            GatewayDiagnostics {
                node_id: ECUNodeId::Gateway,
                gateway_temperature: self.gateway_temperature,
                fused_temperature: fused_temp,
                system_health_score: self.system_health_score,
                diagnostic_mode: self.diagnostic_mode,
                message_routing_count: self.message_routing_count,
                emergency_state,
                error_count,
                cycle_count: self.cycle_count,
            }
        }
    }
}

/// Gateway-specific diagnostic information
#[derive(Debug, Clone)]
struct GatewayDiagnostics {
    node_id: ECUNodeId,
    gateway_temperature: f32,
    fused_temperature: f32,
    system_health_score: u8,
    diagnostic_mode: bool,
    message_routing_count: u64,
    emergency_state: u32,
    error_count: u32,
    cycle_count: u64,
}

/// Global Gateway ECU instance for simulation interface
static mut GATEWAY_ECU: Option<GatewayECU> = None;

/// Main entry point for Gateway ECU
#[entry]
fn main() -> ! {
    // Initialize Gateway ECU
    unsafe {
        GATEWAY_ECU = Some(GatewayECU::new());
    }
    
    // Main execution loop
    loop {
        unsafe {
            if let Some(ref mut gateway_ecu) = GATEWAY_ECU {
                match gateway_ecu.run_cycle() {
                    Ok(_) => {
                        // Cycle completed successfully
                        // Prevent optimization by using volatile operations
                        ptr::write_volatile(&mut gateway_ecu.cycle_count as *mut u64, gateway_ecu.cycle_count);
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
pub extern "C" fn gateway_ecu_simulation_step() -> u32 {
    unsafe {
        if GATEWAY_ECU.is_none() {
            GATEWAY_ECU = Some(GatewayECU::new());
        }
        
        if let Some(ref mut ecu) = GATEWAY_ECU {
            let _ = ecu.run_cycle();
            
            // Return fused temperature from CRDT processing
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            (fused_temp * 100.0) as u32
        } else {
            4000 // Default temperature (40.0°C)
        }
    }
}

/// Get gateway diagnostics for Renode monitoring
#[no_mangle]
pub extern "C" fn gateway_ecu_get_diagnostics() -> *const GatewayDiagnostics {
    static mut DIAGNOSTICS: Option<GatewayDiagnostics> = None;
    
    unsafe {
        if let Some(ref ecu) = GATEWAY_ECU {
            DIAGNOSTICS = Some(ecu.get_gateway_diagnostics());
            if let Some(ref diag) = DIAGNOSTICS {
                diag as *const GatewayDiagnostics
            } else {
                core::ptr::null()
            }
        } else {
            core::ptr::null()
        }
    }
}

/// Get system health score
#[no_mangle]
pub extern "C" fn gateway_ecu_get_health_score() -> u8 {
    unsafe {
        if let Some(ref ecu) = GATEWAY_ECU {
            ecu.system_health_score
        } else {
            100 // Default healthy score
        }
    }
}

/// Inject emergency gateway scenario for testing
#[no_mangle]
pub extern "C" fn gateway_ecu_inject_emergency() -> bool {
    unsafe {
        if let Some(ref mut ecu) = GATEWAY_ECU {
            // Inject diagnostic mode activation
            ecu.diagnostic_mode = true;
            
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

/// Inject gateway temperature for testing
#[no_mangle]
pub extern "C" fn gateway_ecu_inject_temperature(temperature: f32) -> bool {
    unsafe {
        if let Some(ref mut ecu) = GATEWAY_ECU {
            ecu.gateway_temperature = temperature;
            
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
pub extern "C" fn gateway_ecu_get_crdt_state() -> CRDTStateSnapshot {
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
