//! Configuration presets module
//!
//! This module provides pre-defined memory configurations for common platforms and use cases.

use crate::memory::define_memory_config;

// Automotive configurations
define_memory_config! {
    name: AutomotiveECUConfig,
    total_memory: 128 * 1024,  // 128KB for automotive ECUs
    max_registers: 500,
    max_counters: 100,
    max_sets: 50,
    max_maps: 30,
    max_nodes: 64,
    max_set_elements: 32,
    max_map_entries: 64,
    max_history_size: 8,
    clock_memory_budget: 1024,
    error_buffer_size: 512,
    memory_alignment: 4,
    cache_line_size: 32,
}

define_memory_config! {
    name: AutomotiveSensorConfig,
    total_memory: 16 * 1024,  // 16KB for sensor nodes
    max_registers: 50,
    max_counters: 20,
    max_sets: 10,
    max_maps: 5,
    max_nodes: 16,
    max_set_elements: 16,
    max_map_entries: 16,
    max_history_size: 4,
    clock_memory_budget: 256,
    error_buffer_size: 128,
    memory_alignment: 4,
    cache_line_size: 32,
}

// STM32 configurations
define_memory_config! {
    name: STM32F4Config,
    total_memory: 32 * 1024,  // 32KB for STM32F4
    max_registers: 100,
    max_counters: 50,
    max_sets: 20,
    max_maps: 15,
    max_nodes: 32,
    max_set_elements: 32,
    max_map_entries: 32,
    max_history_size: 4,
    clock_memory_budget: 512,
    error_buffer_size: 256,
    memory_alignment: 4,
    cache_line_size: 32,
}

define_memory_config! {
    name: STM32F0Config,
    total_memory: 8 * 1024,  // 8KB for STM32F0
    max_registers: 25,
    max_counters: 15,
    max_sets: 8,
    max_maps: 5,
    max_nodes: 16,
    max_set_elements: 16,
    max_map_entries: 16,
    max_history_size: 2,
    clock_memory_budget: 256,
    error_buffer_size: 128,
    memory_alignment: 4,
    cache_line_size: 32,
}

// IoT configurations
define_memory_config! {
    name: IoTSensorConfig,
    total_memory: 4 * 1024,  // 4KB for IoT sensors
    max_registers: 20,
    max_counters: 10,
    max_sets: 5,
    max_maps: 3,
    max_nodes: 16,
    max_set_elements: 8,
    max_map_entries: 8,
    max_history_size: 2,
    clock_memory_budget: 128,
    error_buffer_size: 64,
    memory_alignment: 4,
    cache_line_size: 32,
}

define_memory_config! {
    name: IoTGatewayConfig,
    total_memory: 64 * 1024,  // 64KB for IoT gateways
    max_registers: 200,
    max_counters: 100,
    max_sets: 50,
    max_maps: 25,
    max_nodes: 128,
    max_set_elements: 64,
    max_map_entries: 64,
    max_history_size: 8,
    clock_memory_budget: 1024,
    error_buffer_size: 512,
    memory_alignment: 4,
    cache_line_size: 32,
}

// Robotics configurations
define_memory_config! {
    name: RoboticsControllerConfig,
    total_memory: 256 * 1024,  // 256KB for robotics controllers
    max_registers: 1000,
    max_counters: 200,
    max_sets: 100,
    max_maps: 50,
    max_nodes: 32,
    max_set_elements: 64,
    max_map_entries: 128,
    max_history_size: 16,
    clock_memory_budget: 2048,
    error_buffer_size: 1024,
    memory_alignment: 8,
    cache_line_size: 64,
}

define_memory_config! {
    name: RoboticsSensorConfig,
    total_memory: 32 * 1024,  // 32KB for robotics sensors
    max_registers: 100,
    max_counters: 50,
    max_sets: 25,
    max_maps: 15,
    max_nodes: 16,
    max_set_elements: 32,
    max_map_entries: 32,
    max_history_size: 4,
    clock_memory_budget: 512,
    error_buffer_size: 256,
    memory_alignment: 4,
    cache_line_size: 32,
}

// Industrial configurations
define_memory_config! {
    name: IndustrialPLCConfig,
    total_memory: 512 * 1024,  // 512KB for industrial PLCs
    max_registers: 2000,
    max_counters: 500,
    max_sets: 200,
    max_maps: 100,
    max_nodes: 64,
    max_set_elements: 64,
    max_map_entries: 256,
    max_history_size: 32,
    clock_memory_budget: 4096,
    error_buffer_size: 2048,
    memory_alignment: 8,
    cache_line_size: 64,
}

define_memory_config! {
    name: IndustrialSensorConfig,
    total_memory: 16 * 1024,  // 16KB for industrial sensors
    max_registers: 50,
    max_counters: 25,
    max_sets: 15,
    max_maps: 10,
    max_nodes: 32,
    max_set_elements: 16,
    max_map_entries: 16,
    max_history_size: 4,
    clock_memory_budget: 256,
    error_buffer_size: 128,
    memory_alignment: 4,
    cache_line_size: 32,
}

// Testing configurations
define_memory_config! {
    name: TestingMinimalConfig,
    total_memory: 2 * 1024,  // 2KB minimal config for testing
    max_registers: 10,
    max_counters: 5,
    max_sets: 3,
    max_maps: 2,
    max_nodes: 8,
    max_set_elements: 8,
    max_map_entries: 8,
    max_history_size: 2,
    clock_memory_budget: 64,
    error_buffer_size: 32,
    memory_alignment: 4,
    cache_line_size: 32,
}

define_memory_config! {
    name: TestingMaximalConfig,
    total_memory: 1024 * 1024,  // 1MB maximal config for testing
    max_registers: 10000,
    max_counters: 5000,
    max_sets: 1000,
    max_maps: 500,
    max_nodes: 255,
    max_set_elements: 64,
    max_map_entries: 512,
    max_history_size: 64,
    clock_memory_budget: 8192,
    error_buffer_size: 4096,
    memory_alignment: 8,
    cache_line_size: 64,
}
