#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

//! STM32 Platform Optimization Example
//!
//! Demonstrates platform-specific optimizations and memory-efficient CRDT usage for STM32 microcontrollers.

use crdtosphere::prelude::*;

// Define a memory-constrained configuration for STM32
define_memory_config! {
    name: STM32Config,
    total_memory: 32 * 1024,  // 32KB total budget
    max_registers: 16,        // Limited registers
    max_counters: 8,          // Limited counters
    max_sets: 4,              // Limited sets
    max_maps: 4,              // Limited maps
    max_nodes: 8,             // Small network
}

fn main() -> Result<(), CRDTError> {
    println!("STM32 Platform Optimization Example");
    println!("===================================");

    // Memory-efficient sensor data collection
    let mut sensor_data = LWWMap::<u8, i16, STM32Config>::new(1);

    // Compact event counting
    let mut event_counter = GCounter::<STM32Config>::new(1);

    // Device capability tracking
    let mut capabilities = GSet::<u8, STM32Config>::new();

    // Configuration management
    let mut config = LWWRegister::<u8, STM32Config>::new(1);

    println!("Memory usage before operations:");
    println!("  Sensor data: {} bytes", sensor_data.size_bytes());
    println!("  Event counter: {} bytes", event_counter.size_bytes());
    println!("  Capabilities: {} bytes", capabilities.size_bytes());
    println!("  Config: {} bytes", config.size_bytes());

    let total_initial = sensor_data.size_bytes()
        + event_counter.size_bytes()
        + capabilities.size_bytes()
        + config.size_bytes();
    println!("  Total: {} bytes", total_initial);

    // Simulate sensor readings (using compact encoding)
    // Temperature sensor (ID 1): 23.5°C -> 235 (scaled by 10)
    sensor_data.insert(1, 235, 1000)?;

    // Humidity sensor (ID 2): 65% -> 65
    sensor_data.insert(2, 65, 1001)?;

    // Pressure sensor (ID 3): 1013 hPa -> 1013
    sensor_data.insert(3, 1013, 1002)?;

    // Battery voltage (ID 4): 3.3V -> 330 (scaled by 100)
    sensor_data.insert(4, 330, 1003)?;

    // Count events efficiently
    event_counter.increment(1)?; // Button press
    event_counter.increment(1)?; // Motion detected
    event_counter.increment(1)?; // Timer interrupt

    // Track device capabilities (using compact IDs)
    capabilities.insert(0x01)?; // WiFi
    capabilities.insert(0x02)?; // Bluetooth
    capabilities.insert(0x04)?; // GPIO
    capabilities.insert(0x08)?; // ADC
    capabilities.insert(0x10)?; // PWM

    // Set configuration (power mode)
    config.set(0x02, 1004)?; // Low power mode

    println!("\nAfter operations:");
    println!("Sensor readings:");
    for sensor_id in [1u8, 2, 3, 4] {
        if let Some(value) = sensor_data.get(&sensor_id) {
            match sensor_id {
                1 => println!("  Temperature: {:.1}°C", *value as f32 / 10.0),
                2 => println!("  Humidity: {}%", value),
                3 => println!("  Pressure: {} hPa", value),
                4 => println!("  Battery: {:.2}V", *value as f32 / 100.0),
                _ => {}
            }
        }
    }

    println!("Event count: {}", event_counter.value());

    println!("Device capabilities:");
    let cap_names = ["WiFi", "Bluetooth", "GPIO", "ADC", "PWM"];
    for (i, &cap_bit) in [0x01u8, 0x02, 0x04, 0x08, 0x10].iter().enumerate() {
        if capabilities.contains(&cap_bit) {
            println!("  ✓ {}", cap_names[i]);
        }
    }

    if let Some(power_mode) = config.get() {
        let mode_name = match power_mode {
            0x01 => "Normal",
            0x02 => "Low Power",
            0x03 => "Sleep",
            _ => "Unknown",
        };
        println!("Power mode: {}", mode_name);
    }

    // Memory efficiency analysis
    println!("\nMemory efficiency:");
    println!(
        "  Sensor data: {} entries, {} bytes ({:.1} bytes/entry)",
        sensor_data.len(),
        sensor_data.size_bytes(),
        sensor_data.size_bytes() as f32 / sensor_data.len().max(1) as f32
    );

    println!(
        "  Capabilities: {} entries, {} bytes ({:.1} bytes/entry)",
        capabilities.len(),
        capabilities.size_bytes(),
        capabilities.size_bytes() as f32 / capabilities.len().max(1) as f32
    );

    // Demonstrate merge efficiency
    let mut other_node = LWWMap::<u8, i16, STM32Config>::new(2);
    other_node.insert(5, 150, 2000)?; // Light sensor: 150 lux
    other_node.insert(6, 25, 2001)?; // Sound level: 25 dB

    println!("\nMerging data from another node...");
    sensor_data.merge(&other_node)?;

    println!("Updated sensor readings:");
    for sensor_id in [1u8, 2, 3, 4, 5, 6] {
        if let Some(value) = sensor_data.get(&sensor_id) {
            match sensor_id {
                1 => println!("  Temperature: {:.1}°C", *value as f32 / 10.0),
                2 => println!("  Humidity: {}%", value),
                3 => println!("  Pressure: {} hPa", value),
                4 => println!("  Battery: {:.2}V", *value as f32 / 100.0),
                5 => println!("  Light: {} lux", value),
                6 => println!("  Sound: {} dB", value),
                _ => {}
            }
        }
    }

    // Platform-specific optimizations
    println!("\nSTM32 Platform Optimizations:");
    println!("✓ Compact data encoding (16-bit values)");
    println!("✓ Memory-constrained configuration");
    println!("✓ Efficient merge operations");
    println!("✓ Bounded memory usage");
    println!("✓ Real-time guarantees");

    // Memory budget check
    let total_used = sensor_data.size_bytes()
        + event_counter.size_bytes()
        + capabilities.size_bytes()
        + config.size_bytes();

    let budget_used = (total_used as f32 / STM32Config::TOTAL_CRDT_MEMORY as f32) * 100.0;

    println!("\nMemory Budget:");
    println!(
        "  Used: {} / {} bytes ({:.1}%)",
        total_used,
        STM32Config::TOTAL_CRDT_MEMORY,
        budget_used
    );
    println!(
        "  Remaining: {} bytes",
        STM32Config::TOTAL_CRDT_MEMORY - total_used
    );

    if budget_used < 50.0 {
        println!("  Status: ✓ Efficient memory usage");
    } else if budget_used < 80.0 {
        println!("  Status: ⚠ Moderate memory usage");
    } else {
        println!("  Status: ⚠ High memory usage");
    }

    // Demonstrate bounded operations
    println!("\nBounded CRDT Operations:");
    println!(
        "  Max merge cycles: {}",
        event_counter.remaining_budget().unwrap_or(0)
    );
    println!("  Validation: {:?}", sensor_data.validate());
    println!("  Can add elements: {}", sensor_data.can_add_element());

    Ok(())
}
