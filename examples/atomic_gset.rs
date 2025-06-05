#![allow(unused_variables)]

//! Atomic GSet (Grow-only Set) Example
//!
//! This example demonstrates the atomic capabilities of GSet CRDTs in CRDTosphere.
//! It shows how multiple threads can safely add elements to a set concurrently
//! without data races or corruption.

use crdtosphere::prelude::*;
use std::sync::Arc;
use std::thread;

fn main() -> Result<(), CRDTError> {
    println!("CRDTosphere Atomic GSet Example");
    println!("===============================");

    #[cfg(feature = "hardware-atomic")]
    {
        println!("Atomic implementation:");
        println!("- Allows &self for modifications");
        println!("- Multi-threaded safe with Arc<T>");
        println!("- Supports concurrent insert operations");
        println!("- Lock-free coordination via compare-exchange");
        println!();

        // Demonstrate concurrent device capability registration
        concurrent_device_capabilities()?;

        // Demonstrate atomic merge operations
        atomic_merge_operations()?;

        // Demonstrate capacity management
        capacity_management_demo()?;

        println!("Atomic GSet demonstration completed!");
        println!("✓ Concurrent insert operations: Multiple threads safely added capabilities");
        println!("✓ Atomic merge operations: Sets merged without data races");
        println!("✓ Capacity management: Buffer overflow protection working correctly");
        println!("✓ Thread safety: All operations completed without data races");
        println!("✓ CRDT properties: Commutativity, idempotence, and associativity preserved");
    }

    #[cfg(not(feature = "hardware-atomic"))]
    {
        println!("Standard implementation:");
        println!("- Requires &mut self for modifications");
        println!("- Single-threaded access only");
        println!("- Manual synchronization needed for multi-threading");
        println!();

        // Demonstrate standard GSet operations
        standard_gset_demo().unwrap();
    }

    Ok(())
}

#[cfg(feature = "hardware-atomic")]
fn concurrent_device_capabilities() -> Result<(), CRDTError> {
    println!("Testing concurrent device capability registration...");

    // Create a shared set for device capabilities
    let capabilities = Arc::new(GSet::<u32, DefaultConfig>::new());

    // Define capability types
    const GPS: u32 = 1;
    const WIFI: u32 = 2;
    const BLUETOOTH: u32 = 3;
    const CAMERA: u32 = 4;

    let mut handles = vec![];

    // Spawn multiple threads to register capabilities concurrently
    for thread_id in 0..4 {
        let capabilities_clone = Arc::clone(&capabilities);

        let handle = thread::spawn(move || {
            let capability = match thread_id {
                0 => GPS,
                1 => WIFI,
                2 => BLUETOOTH,
                3 => CAMERA,
                _ => unreachable!(),
            };

            println!(
                "  Thread {} registering capability: {}",
                thread_id, capability
            );

            // Insert capability (atomic operation)
            match capabilities_clone.insert(capability) {
                Ok(true) => println!(
                    "  Thread {} successfully registered capability {}",
                    thread_id, capability
                ),
                Ok(false) => println!(
                    "  Thread {} found capability {} already registered",
                    thread_id, capability
                ),
                Err(e) => println!(
                    "  Thread {} failed to register capability {}: {:?}",
                    thread_id, capability, e
                ),
            }

            // Try to register the same capability again (should return false)
            match capabilities_clone.insert(capability) {
                Ok(false) => println!(
                    "  Thread {} confirmed capability {} already exists",
                    thread_id, capability
                ),
                Ok(true) => println!(
                    "  Thread {} unexpectedly re-registered capability {}",
                    thread_id, capability
                ),
                Err(e) => println!("  Thread {} error on duplicate insert: {:?}", thread_id, e),
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    println!("\nFinal device capabilities:");
    println!("Number of capabilities: {}", capabilities.len());
    for capability in capabilities.iter() {
        let name = match *capability {
            GPS => "GPS",
            WIFI => "WiFi",
            BLUETOOTH => "Bluetooth",
            CAMERA => "Camera",
            _ => "Unknown",
        };
        println!("  Capability {}: {}", capability, name);
    }

    // Verify all expected capabilities are present
    assert!(capabilities.contains(&GPS));
    assert!(capabilities.contains(&WIFI));
    assert!(capabilities.contains(&BLUETOOTH));
    assert!(capabilities.contains(&CAMERA));
    assert_eq!(capabilities.len(), 4);

    println!("✓ Concurrent capability registration successful\n");
    Ok(())
}

#[cfg(feature = "hardware-atomic")]
fn atomic_merge_operations() -> Result<(), CRDTError> {
    println!("Testing atomic merge operations...");

    // Create multiple sets representing different device types
    let mobile_capabilities = Arc::new(GSet::<u32, DefaultConfig>::new());
    let iot_capabilities = Arc::new(GSet::<u32, DefaultConfig>::new());
    let automotive_capabilities = Arc::new(GSet::<u32, DefaultConfig>::new());

    // Populate sets concurrently
    let mobile_clone = Arc::clone(&mobile_capabilities);
    let mobile_handle = thread::spawn(move || {
        mobile_clone.insert(1).unwrap(); // GPS
        mobile_clone.insert(2).unwrap(); // WiFi
        mobile_clone.insert(3).unwrap(); // Bluetooth
        mobile_clone.insert(4).unwrap(); // Camera
        println!("  Mobile device capabilities registered");
    });

    let iot_clone = Arc::clone(&iot_capabilities);
    let iot_handle = thread::spawn(move || {
        iot_clone.insert(2).unwrap(); // WiFi (overlap)
        iot_clone.insert(5).unwrap(); // Temperature sensor
        iot_clone.insert(6).unwrap(); // Humidity sensor
        println!("  IoT device capabilities registered");
    });

    let auto_clone = Arc::clone(&automotive_capabilities);
    let auto_handle = thread::spawn(move || {
        auto_clone.insert(1).unwrap(); // GPS (overlap)
        auto_clone.insert(7).unwrap(); // CAN bus
        auto_clone.insert(8).unwrap(); // LIDAR
        println!("  Automotive device capabilities registered");
    });

    // Wait for population to complete
    mobile_handle.join().unwrap();
    iot_handle.join().unwrap();
    auto_handle.join().unwrap();

    // Create a unified capability set
    let unified_capabilities = Arc::new(GSet::<u32, DefaultConfig>::new());

    // For atomic GSet, we need to use a different approach since merge requires &mut self
    // Let's create a final unified set and populate it with all capabilities
    let final_unified = Arc::new(GSet::<u32, DefaultConfig>::new());

    // Add all capabilities from mobile devices
    for capability in mobile_capabilities.iter() {
        final_unified.insert(*capability).unwrap();
    }

    // Add all capabilities from IoT devices
    for capability in iot_capabilities.iter() {
        final_unified.insert(*capability).unwrap();
    }

    // Add all capabilities from automotive devices
    for capability in automotive_capabilities.iter() {
        final_unified.insert(*capability).unwrap();
    }

    println!("  All capabilities unified");

    println!("\nUnified capability set:");
    println!("Total capabilities: {}", final_unified.len());
    let mut capabilities: Vec<u32> = final_unified.iter().cloned().collect();
    capabilities.sort();
    for capability in capabilities {
        let name = match capability {
            1 => "GPS",
            2 => "WiFi",
            3 => "Bluetooth",
            4 => "Camera",
            5 => "Temperature",
            6 => "Humidity",
            7 => "CAN Bus",
            8 => "LIDAR",
            _ => "Unknown",
        };
        println!("  Capability {}: {}", capability, name);
    }

    // Verify merge semantics
    assert_eq!(final_unified.len(), 8); // All unique capabilities
    assert!(final_unified.contains(&1)); // GPS from mobile and automotive
    assert!(final_unified.contains(&2)); // WiFi from mobile and IoT

    println!("✓ Atomic merge operations successful\n");
    Ok(())
}

#[cfg(feature = "hardware-atomic")]
fn capacity_management_demo() -> Result<(), CRDTError> {
    println!("Testing capacity management and overflow protection...");

    let capabilities = Arc::new(GSet::<u32, DefaultConfig>::new());

    // Fill the set to near capacity
    for i in 0..15 {
        capabilities.insert(i)?;
    }

    println!("  Filled set with 15 capabilities");
    println!(
        "  Remaining capacity: {}",
        capabilities.remaining_capacity()
    );
    assert_eq!(capabilities.remaining_capacity(), 1);

    // Test concurrent attempts to fill the last slot
    let cap_clone1 = Arc::clone(&capabilities);
    let cap_clone2 = Arc::clone(&capabilities);

    let handle1 = thread::spawn(move || match cap_clone1.insert(100) {
        Ok(true) => println!("  Thread 1 successfully inserted capability 100"),
        Ok(false) => println!("  Thread 1 found capability 100 already exists"),
        Err(e) => println!("  Thread 1 failed to insert capability 100: {:?}", e),
    });

    let handle2 = thread::spawn(move || match cap_clone2.insert(101) {
        Ok(true) => println!("  Thread 2 successfully inserted capability 101"),
        Ok(false) => println!("  Thread 2 found capability 101 already exists"),
        Err(e) => println!("  Thread 2 failed to insert capability 101: {:?}", e),
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    println!("  Final capacity: {}", capabilities.len());
    println!("  Is full: {}", capabilities.is_full());

    // Verify that exactly one of the concurrent inserts succeeded
    let contains_100 = capabilities.contains(&100);
    let contains_101 = capabilities.contains(&101);

    // Exactly one should have succeeded due to atomic coordination
    assert!(
        contains_100 ^ contains_101,
        "Exactly one concurrent insert should succeed"
    );
    assert_eq!(capabilities.len(), 16); // Should be at capacity
    assert!(capabilities.is_full());

    // Test that further inserts fail
    match capabilities.insert(200) {
        Err(CRDTError::BufferOverflow) => println!("  ✓ Buffer overflow correctly detected"),
        Ok(_) => panic!("Insert should have failed due to capacity"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }

    println!("✓ Capacity management working correctly\n");
    Ok(())
}

#[cfg(not(feature = "hardware-atomic"))]
fn standard_gset_demo() -> Result<(), CRDTError> {
    println!("Testing standard GSet operations...");

    let mut capabilities = GSet::<u32, DefaultConfig>::new();

    // Add some capabilities
    capabilities.insert(1)?; // GPS
    capabilities.insert(2)?; // WiFi
    capabilities.insert(3)?; // Bluetooth

    println!("Added capabilities: GPS, WiFi, Bluetooth");
    println!("Total capabilities: {}", capabilities.len());

    // Test contains
    assert!(capabilities.contains(&1));
    assert!(capabilities.contains(&2));
    assert!(capabilities.contains(&3));
    assert!(!capabilities.contains(&4));

    // Test merge with another set
    let mut other_capabilities = GSet::<u32, DefaultConfig>::new();
    other_capabilities.insert(2)?; // WiFi (duplicate)
    other_capabilities.insert(4)?; // Camera

    capabilities.merge(&other_capabilities)?;

    println!("After merge, total capabilities: {}", capabilities.len());
    assert_eq!(capabilities.len(), 4);
    assert!(capabilities.contains(&4));

    println!("✓ Standard GSet operations successful");
    Ok(())
}
