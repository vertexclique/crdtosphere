//! CRDT Demonstration Module
//! 
//! Demonstrates various CRDT operations with LED feedback

use crdtosphere::prelude::*;
use crate::led_controller::{LedController, LedOperation, BlinkPattern, LedStats};
use defmt::{info, warn, error};

// Define memory configuration optimized for STM32F767ZI
define_memory_config! {
    name: NucleoF767Config,
    total_memory: 64 * 1024,  // 64KB budget (plenty for F767ZI's 512KB SRAM)
    max_registers: 8,
    max_counters: 4,
    max_sets: 4,
    max_maps: 2,
    max_nodes: 4,             // Simulate 4 nodes
}

pub struct CrdtDemo {
    // CRDT instances
    button_counter: GCounter<NucleoF767Config>,
    device_registry: ORSet<u8, NucleoF767Config>,
    config_register: LWWRegister<u8, NucleoF767Config>,
    sensor_data: LWWMap<u8, i16, NucleoF767Config>,
    
    // Current node ID (simulates different nodes)
    current_node: u8,
    
    // Statistics
    led_stats: LedStats,
}

impl CrdtDemo {
    pub fn new() -> Result<Self, CRDTError> {
        info!("CRDT: Initializing demo with NucleoF767Config");
        
        Ok(Self {
            button_counter: GCounter::new(1),
            device_registry: ORSet::new(1),
            config_register: LWWRegister::new(1),
            sensor_data: LWWMap::new(1),
            current_node: 1,
            led_stats: LedStats::new(),
        })
    }
    
    /// Simulate button press - increment counter and show insert LED
    pub fn handle_button_press(&mut self, led_controller: &mut LedController) -> Result<(), CRDTError> {
        info!("CRDT: Button pressed by node {}", self.current_node);
        
        // Increment counter (convert u8 to u32)
        self.button_counter.increment(self.current_node as u32)?;
        
        // Show insert operation
        led_controller.indicate_operation(LedOperation::Insert, BlinkPattern::Single);
        self.led_stats.record_operation(LedOperation::Insert);
        
        info!("CRDT: Button count now: {}", self.button_counter.value());
        
        Ok(())
    }
    
    /// Add a device to the registry
    pub fn add_device(&mut self, device_id: u8, led_controller: &mut LedController) -> Result<(), CRDTError> {
        info!("CRDT: Adding device {} by node {}", device_id, self.current_node);
        
        let timestamp = self.current_node as u64 * 1000 + device_id as u64;
        self.device_registry.add(device_id, timestamp)?;
        
        // Show insert operation
        led_controller.indicate_operation(LedOperation::Insert, BlinkPattern::Double);
        self.led_stats.record_operation(LedOperation::Insert);
        
        info!("CRDT: Device registry now has {} devices", self.device_registry.len());
        
        Ok(())
    }
    
    /// Remove a device from the registry (tombstone)
    pub fn remove_device(&mut self, device_id: u8, led_controller: &mut LedController) -> Result<(), CRDTError> {
        info!("CRDT: Removing device {} by node {}", device_id, self.current_node);
        
        if self.device_registry.contains(&device_id) {
            let timestamp = self.current_node as u64 * 1000 + device_id as u64 + 5000; // Later timestamp for remove
            self.device_registry.remove(&device_id, timestamp)?;
            
            // Show delete operation
            led_controller.indicate_operation(LedOperation::Delete, BlinkPattern::Single);
            self.led_stats.record_operation(LedOperation::Delete);
            
            info!("CRDT: Device {} removed, registry now has {} devices", device_id, self.device_registry.len());
        } else {
            warn!("CRDT: Device {} not found in registry", device_id);
        }
        
        Ok(())
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config_value: u8, led_controller: &mut LedController) -> Result<(), CRDTError> {
        info!("CRDT: Updating config to {} by node {}", config_value, self.current_node);
        
        // Use current node as timestamp (simplified)
        let timestamp = self.current_node as u64 * 1000;
        self.config_register.set(config_value, timestamp)?;
        
        // Show insert operation (config update)
        led_controller.indicate_operation(LedOperation::Insert, BlinkPattern::Triple);
        self.led_stats.record_operation(LedOperation::Insert);
        
        if let Some(current_config) = self.config_register.get() {
            info!("CRDT: Config updated to: {}", current_config);
        }
        
        Ok(())
    }
    
    /// Add sensor reading
    pub fn add_sensor_reading(&mut self, sensor_id: u8, value: i16, led_controller: &mut LedController) -> Result<(), CRDTError> {
        info!("CRDT: Adding sensor {} reading: {} by node {}", sensor_id, value, self.current_node);
        
        let timestamp = self.current_node as u64 * 1000 + sensor_id as u64;
        self.sensor_data.insert(sensor_id, value, timestamp)?;
        
        // Show insert operation
        led_controller.indicate_operation(LedOperation::Insert, BlinkPattern::Solid(200));
        self.led_stats.record_operation(LedOperation::Insert);
        
        info!("CRDT: Sensor data now has {} readings", self.sensor_data.len());
        
        Ok(())
    }
    
    /// Simulate receiving data from another node and merging
    pub fn simulate_node_merge(&mut self, led_controller: &mut LedController) -> Result<(), CRDTError> {
        info!("CRDT: Simulating merge from other nodes");
        
        // Create simulated data from other nodes
        let mut other_counter = GCounter::<NucleoF767Config>::new(1);
        other_counter.increment(1)?;
        other_counter.increment(1)?;
        
        let mut other_registry = ORSet::<u8, NucleoF767Config>::new(1);
        other_registry.add(100, 1500)?; // Device 100 from node 1
        other_registry.add(101, 1501)?; // Device 101 from node 1
        
        let mut other_config = LWWRegister::<u8, NucleoF767Config>::new(1);
        other_config.set(42, 2000)?; // Newer config from node 1
        
        let mut other_sensor_data = LWWMap::<u8, i16, NucleoF767Config>::new(1);
        other_sensor_data.insert(10, 250, 2000)?; // Temperature: 25.0°C
        other_sensor_data.insert(11, 65, 2001)?;  // Humidity: 65%
        
        // Merge all CRDTs
        info!("CRDT: Merging counter data...");
        self.button_counter.merge(&other_counter)?;
        led_controller.indicate_operation(LedOperation::Merge, BlinkPattern::Single);
        self.led_stats.record_operation(LedOperation::Merge);
        
        info!("CRDT: Merging device registry...");
        self.device_registry.merge(&other_registry)?;
        led_controller.indicate_operation(LedOperation::Merge, BlinkPattern::Double);
        self.led_stats.record_operation(LedOperation::Merge);
        
        info!("CRDT: Merging configuration...");
        self.config_register.merge(&other_config)?;
        led_controller.indicate_operation(LedOperation::Merge, BlinkPattern::Triple);
        self.led_stats.record_operation(LedOperation::Merge);
        
        info!("CRDT: Merging sensor data...");
        self.sensor_data.merge(&other_sensor_data)?;
        led_controller.indicate_operation(LedOperation::Merge, BlinkPattern::Solid(300));
        self.led_stats.record_operation(LedOperation::Merge);
        
        info!("CRDT: Merge complete - all data converged");
        
        Ok(())
    }
    
    /// Switch to simulate different node
    pub fn switch_node(&mut self, node_id: u8, led_controller: &mut LedController) {
        if node_id < 4 {
            self.current_node = node_id;
            info!("CRDT: Switched to node {}", node_id);
            
            // Visual indication of node switch
            for _ in 0..=node_id {
                led_controller.indicate_operation(LedOperation::Insert, BlinkPattern::Single);
            }
        } else {
            warn!("CRDT: Invalid node ID {}, must be 0-3", node_id);
        }
    }
    
    /// Run automated demo sequence
    pub fn run_demo_sequence(&mut self, led_controller: &mut LedController) -> Result<(), CRDTError> {
        info!("CRDT: Starting automated demo sequence");
        
        // Node 1 operations (avoid node 0)
        self.switch_node(1, led_controller);
        self.handle_button_press(led_controller)?;
        self.add_device(10, led_controller)?;
        self.update_config(1, led_controller)?;
        self.add_sensor_reading(1, 235, led_controller)?; // 23.5°C
        
        // Small delay between operations
        self.delay_ms(500);
        
        // Node 2 operations
        self.switch_node(2, led_controller);
        self.handle_button_press(led_controller)?;
        self.add_device(20, led_controller)?;
        self.add_sensor_reading(2, 65, led_controller)?; // 65% humidity
        
        self.delay_ms(500);
        
        // Node 3 operations
        self.switch_node(3, led_controller);
        self.add_device(30, led_controller)?;
        self.remove_device(10, led_controller)?; // Remove device 10
        self.update_config(2, led_controller)?; // Newer config
        
        self.delay_ms(500);
        
        // Simulate network merge
        self.simulate_node_merge(led_controller)?;
        
        self.delay_ms(1000);
        
        // Show final state
        self.show_final_state();
        
        info!("CRDT: Demo sequence complete");
        led_controller.success_pattern();
        
        Ok(())
    }
    
    /// Display current state of all CRDTs
    pub fn show_current_state(&self) {
        info!("=== CRDT Current State ===");
        info!("Current Node: {}", self.current_node);
        info!("Button Counter: {}", self.button_counter.value());
        info!("Device Registry: {} devices", self.device_registry.len());
        
        if let Some(config) = self.config_register.get() {
            info!("Current Config: {}", config);
        } else {
            info!("Current Config: None");
        }
        
        info!("Sensor Data: {} readings", self.sensor_data.len());
        
        // Show memory usage
        let total_memory = self.button_counter.size_bytes() 
            + self.device_registry.size_bytes()
            + self.config_register.size_bytes()
            + self.sensor_data.size_bytes();
            
        let percentage = (total_memory as f32 / NucleoF767Config::TOTAL_CRDT_MEMORY as f32) * 100.0;
        info!("Total Memory Usage: {} / {} bytes ({}%)", 
              total_memory, 
              NucleoF767Config::TOTAL_CRDT_MEMORY,
              percentage as u32);
    }
    
    /// Display final state after demo
    fn show_final_state(&self) {
        info!("=== CRDT Final Demo State ===");
        info!("Button presses (total): {}", self.button_counter.value());
        info!("Active devices: {}", self.device_registry.len());
        
        if let Some(config) = self.config_register.get() {
            info!("Final configuration: {}", config);
        }
        
        info!("Sensor readings: {}", self.sensor_data.len());
        
        info!("LED Operation Statistics:");
        info!("  Insert operations: {}", self.led_stats.insert_count);
        info!("  Delete operations: {}", self.led_stats.delete_count);
        info!("  Merge operations: {}", self.led_stats.merge_count);
        info!("  Total operations: {}", self.led_stats.total_operations());
    }
    
    /// Get LED statistics
    pub fn get_led_stats(&self) -> &LedStats {
        &self.led_stats
    }
    
    /// Reset all CRDTs and statistics
    pub fn reset(&mut self) -> Result<(), CRDTError> {
        info!("CRDT: Resetting all data structures");
        
        self.button_counter = GCounter::new(0);
        self.device_registry = ORSet::new(0);
        self.config_register = LWWRegister::new(0);
        self.sensor_data = LWWMap::new(0);
        self.current_node = 0;
        self.led_stats.reset();
        
        Ok(())
    }
    
    /// Simple delay implementation
    fn delay_ms(&self, ms: u32) {
        // Much more reasonable delay - approximately 1000 cycles per ms
        // This gives a good balance between accuracy and responsiveness
        let cycles_per_ms = 1000;
        for _ in 0..(ms * cycles_per_ms) {
            cortex_m::asm::nop();
        }
    }
}

/// Validate CRDT properties (simplified for embedded environment)
pub fn validate_crdt_properties() -> Result<(), CRDTError> {
    info!("CRDT: Validating basic CRDT functionality");
    
    // Simple validation - just test basic operations work
    let mut counter = GCounter::<NucleoF767Config>::new(1);
    counter.increment(1)?;
    
    if counter.value() == 1 {
        info!("✓ GCounter basic operation verified");
    } else {
        error!("✗ GCounter basic operation failed");
        return Err(CRDTError::InvalidMerge);
    }
    
    // Test ORSet basic operations
    let mut set = ORSet::<u8, NucleoF767Config>::new(1);
    set.add(42, 1000)?;
    
    if set.contains(&42) && set.len() == 1 {
        info!("✓ ORSet basic operation verified");
    } else {
        error!("✗ ORSet basic operation failed");
        return Err(CRDTError::InvalidMerge);
    }
    
    // Test LWWRegister basic operations
    let mut register = LWWRegister::<u8, NucleoF767Config>::new(1);
    register.set(123, 2000)?;
    
    if let Some(value) = register.get() {
        if *value == 123 {
            info!("✓ LWWRegister basic operation verified");
        } else {
            error!("✗ LWWRegister basic operation failed");
            return Err(CRDTError::InvalidMerge);
        }
    } else {
        error!("✗ LWWRegister get operation failed");
        return Err(CRDTError::InvalidMerge);
    }
    
    info!("✓ Basic CRDT functionality validated");
    Ok(())
}
