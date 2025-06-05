//! Steering Control Unit (ECU) Implementation
//!
//! This binary implements the Steering ECU with ASIL-C safety level,
//! responsible for power steering, stability control, and steering assistance
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

/// Steering ECU with memory-mapped CRDT processing
struct SteeringECU {
    crdt_state: MemoryMappedCRDTState,
    cycle_count: u64,
    steering_temperature: f32,
    steering_angle: f32,
    power_assist_level: u8,
    stability_control_active: bool,
}

impl SteeringECU {
    /// Creates a new Steering ECU
    fn new() -> Self {
        Self {
            crdt_state: MemoryMappedCRDTState::new(ECUNodeId::Steering),
            cycle_count: 0,
            steering_temperature: 35.0, // Start at ambient temperature
            steering_angle: 0.0,
            power_assist_level: 50,
            stability_control_active: false,
        }
    }
    
    /// Simulates steering operation and temperature generation
    fn simulate_steering_operation(&mut self) {
        // Simulate steering patterns
        let steering_input = match self.cycle_count % 1500 {
            0..=250 => 0.0,        // Straight driving
            251..=500 => 15.0,     // Right turn
            501..=750 => 0.0,      // Straight
            751..=1000 => -15.0,   // Left turn
            1001..=1250 => 0.0,    // Straight
            _ => ((self.cycle_count % 30) as f32 - 15.0) * 0.5, // Small corrections
        };
        
        // Update steering angle with smoothing
        let angle_diff = steering_input - self.steering_angle;
        self.steering_angle += angle_diff * 0.15;
        self.steering_angle = self.steering_angle.clamp(-45.0, 45.0);
        
        // Update power assist based on steering effort
        let effort = self.steering_angle.abs();
        self.power_assist_level = (30.0 + effort * 2.0) as u8;
        self.power_assist_level = self.power_assist_level.min(100);
        
        // Update temperature based on power assist usage
        let heat_generation = (self.power_assist_level as f32) * 0.008;
        let cooling = 0.08; // Natural cooling
        
        self.steering_temperature += heat_generation - cooling;
        self.steering_temperature = self.steering_temperature.clamp(25.0, 65.0);
        
        // Activate stability control during sharp turns
        self.stability_control_active = effort > 20.0;
        
        // Add some random variation
        let variation = ((self.cycle_count % 31) as f32 - 15.0) * 0.03;
        self.steering_temperature += variation;
    }
    
    /// Writes steering data to memory-mapped input regions
    fn write_inputs_to_memory(&self) {
        unsafe {
            // Write temperature to input region
            ptr::write_volatile(
                memory_regions::TEMP_INPUT as *mut f32,
                self.steering_temperature
            );
            
            // Write emergency condition (stability control activation)
            let emergency_condition = if self.stability_control_active { 0x00000001 } else { 0x00000000 };
            ptr::write_volatile(
                memory_regions::EMERGENCY_INPUT as *mut u32,
                emergency_condition
            );
            
            // Write configuration data (steering-specific config)
            let config_data = ((self.power_assist_level as u32) << 16) | 
                             (if self.stability_control_active { 0x02 } else { 0x00 }) |
                             0x01; // ABS enabled
            ptr::write_volatile(
                memory_regions::CONFIG_INPUT as *mut u32,
                config_data
            );
            
            // Write error condition (steering temperature fault detection)
            let error_condition = if self.steering_temperature > 60.0 || self.steering_temperature <= 0.0 {
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
            
            // Update steering state based on CRDT results
            if emergency_state != 0 {
                // Emergency detected - engage stability control
                self.stability_control_active = true;
                // Reduce power assist for better control
                self.power_assist_level = self.power_assist_level.min(40);
            }
            
            // Use fused temperature for steering control decisions
            if fused_temp > 55.0 {
                // Reduce power assist at high temperatures to prevent overheating
                self.power_assist_level = (self.power_assist_level as f32 * 0.8) as u8;
            }
        }
    }
    
    /// Main execution cycle
    fn run_cycle(&mut self) -> Result<(), ECUError> {
        self.cycle_count += 1;
        
        // Simulate steering operation
        self.simulate_steering_operation();
        
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
    
    /// Logs current steering status
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
            
            if fused_temp > 55.0 {
                // High steering temperature warning
            }
            
            if self.stability_control_active {
                // Stability control active
            }
        }
    }
    
    /// Gets steering-specific diagnostics
    fn get_steering_diagnostics(&self) -> SteeringDiagnostics {
        unsafe {
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            let emergency_state = ptr::read_volatile(memory_regions::EMERGENCY_OUTPUT as *const u32);
            let error_count = ptr::read_volatile(memory_regions::ERROR_OUTPUT as *const u32);
            
            SteeringDiagnostics {
                node_id: ECUNodeId::Steering,
                steering_temperature: self.steering_temperature,
                fused_temperature: fused_temp,
                steering_angle: self.steering_angle,
                power_assist_level: self.power_assist_level,
                stability_control_active: self.stability_control_active,
                emergency_state,
                error_count,
                cycle_count: self.cycle_count,
            }
        }
    }
}

/// Steering-specific diagnostic information
#[derive(Debug, Clone)]
struct SteeringDiagnostics {
    node_id: ECUNodeId,
    steering_temperature: f32,
    fused_temperature: f32,
    steering_angle: f32,
    power_assist_level: u8,
    stability_control_active: bool,
    emergency_state: u32,
    error_count: u32,
    cycle_count: u64,
}

/// Global Steering ECU instance for simulation interface
static mut STEERING_ECU: Option<SteeringECU> = None;

/// Main entry point for Steering ECU
#[entry]
fn main() -> ! {
    // Initialize Steering ECU
    unsafe {
        STEERING_ECU = Some(SteeringECU::new());
    }
    
    // Main execution loop
    loop {
        unsafe {
            if let Some(ref mut steering_ecu) = STEERING_ECU {
                match steering_ecu.run_cycle() {
                    Ok(_) => {
                        // Cycle completed successfully
                        // Prevent optimization by using volatile operations
                        ptr::write_volatile(&mut steering_ecu.cycle_count as *mut u64, steering_ecu.cycle_count);
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
pub extern "C" fn steering_ecu_simulation_step() -> u32 {
    unsafe {
        if STEERING_ECU.is_none() {
            STEERING_ECU = Some(SteeringECU::new());
        }
        
        if let Some(ref mut ecu) = STEERING_ECU {
            let _ = ecu.run_cycle();
            
            // Return fused temperature from CRDT processing
            let fused_temp = ptr::read_volatile(memory_regions::TEMP_OUTPUT as *const f32);
            (fused_temp * 100.0) as u32
        } else {
            3500 // Default temperature (35.0°C)
        }
    }
}

/// Get steering diagnostics for Renode monitoring
#[no_mangle]
pub extern "C" fn steering_ecu_get_diagnostics() -> *const SteeringDiagnostics {
    static mut DIAGNOSTICS: Option<SteeringDiagnostics> = None;
    
    unsafe {
        if let Some(ref ecu) = STEERING_ECU {
            DIAGNOSTICS = Some(ecu.get_steering_diagnostics());
            if let Some(ref diag) = DIAGNOSTICS {
                diag as *const SteeringDiagnostics
            } else {
                core::ptr::null()
            }
        } else {
            core::ptr::null()
        }
    }
}

/// Inject emergency steering scenario for testing
#[no_mangle]
pub extern "C" fn steering_ecu_inject_emergency() -> bool {
    unsafe {
        if let Some(ref mut ecu) = STEERING_ECU {
            // Inject stability control activation
            ecu.stability_control_active = true;
            
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

/// Inject steering temperature for testing
#[no_mangle]
pub extern "C" fn steering_ecu_inject_temperature(temperature: f32) -> bool {
    unsafe {
        if let Some(ref mut ecu) = STEERING_ECU {
            ecu.steering_temperature = temperature;
            
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
pub extern "C" fn steering_ecu_get_crdt_state() -> CRDTStateSnapshot {
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
