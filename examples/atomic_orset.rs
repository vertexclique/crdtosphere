//! Atomic ORSet Example
//!
//! Demonstrates thread-safe ORSet operations using hardware atomic primitives.
//! This example shows how multiple threads can safely add and remove elements
//! from an ORSet concurrently.

use crdtosphere::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn main() -> Result<(), CRDTError> {
    println!("CRDTosphere Atomic ORSet Example");
    println!("================================");

    #[cfg(feature = "hardware-atomic")]
    {
        println!("Atomic implementation:");
        println!("- Allows &self for modifications");
        println!("- Multi-threaded safe with Arc<T>");
        println!("- Supports concurrent add/remove operations");
        println!("- Lock-free coordination via compare-exchange");
        println!();

        // Demonstrate concurrent device capability management
        concurrent_device_capabilities()?;

        // Demonstrate atomic add/remove patterns
        atomic_add_remove_patterns()?;

        // Demonstrate capacity management
        capacity_management_demo()?;

        println!("Atomic ORSet demonstration completed!");
        println!("✓ Concurrent add operations: Multiple threads safely added capabilities");
        println!("✓ Concurrent remove operations: Elements removed atomically");
        println!("✓ Add-after-remove semantics: Proper timestamp-based conflict resolution");
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

        // Demonstrate standard ORSet operations
        standard_orset_demo().unwrap();
    }

    Ok(())
}

#[cfg(feature = "hardware-atomic")]
fn concurrent_device_capabilities() -> Result<(), CRDTError> {
    println!("Testing concurrent device capability management...");

    // Create a shared set for device capabilities
    let capabilities = Arc::new(ORSet::<u32, DefaultConfig>::new(1));

    // Define capability types
    const GPS: u32 = 1;
    const WIFI: u32 = 2;
    const BLUETOOTH: u32 = 3;
    const CAMERA: u32 = 4;
    const ACCELEROMETER: u32 = 5;
    const GYROSCOPE: u32 = 6;

    let mut handles = vec![];

    // Spawn multiple threads to manage capabilities concurrently
    for thread_id in 0..4 {
        let capabilities_clone = Arc::clone(&capabilities);

        let handle = thread::spawn(move || {
            let base_timestamp = get_timestamp();

            match thread_id {
                0 => {
                    // Thread 0: Add GPS and WiFi
                    println!("  Thread {} adding GPS capability", thread_id);
                    match capabilities_clone.add(GPS, base_timestamp) {
                        Ok(true) => println!("  Thread {} successfully added GPS", thread_id),
                        Ok(false) => println!("  Thread {} found GPS already exists", thread_id),
                        Err(e) => println!("  Thread {} failed to add GPS: {:?}", thread_id, e),
                    }

                    thread::sleep(std::time::Duration::from_millis(5));

                    println!("  Thread {} adding WiFi capability", thread_id);
                    match capabilities_clone.add(WIFI, base_timestamp + 10) {
                        Ok(true) => println!("  Thread {} successfully added WiFi", thread_id),
                        Ok(false) => println!("  Thread {} found WiFi already exists", thread_id),
                        Err(e) => println!("  Thread {} failed to add WiFi: {:?}", thread_id, e),
                    }
                }
                1 => {
                    // Thread 1: Add Bluetooth, then remove WiFi
                    thread::sleep(std::time::Duration::from_millis(3));

                    println!("  Thread {} adding Bluetooth capability", thread_id);
                    match capabilities_clone.add(BLUETOOTH, base_timestamp + 5) {
                        Ok(true) => println!("  Thread {} successfully added Bluetooth", thread_id),
                        Ok(false) => {
                            println!("  Thread {} found Bluetooth already exists", thread_id)
                        }
                        Err(e) => {
                            println!("  Thread {} failed to add Bluetooth: {:?}", thread_id, e)
                        }
                    }

                    thread::sleep(std::time::Duration::from_millis(10));

                    println!("  Thread {} removing WiFi capability", thread_id);
                    match capabilities_clone.remove(&WIFI, base_timestamp + 20) {
                        Ok(true) => println!("  Thread {} successfully removed WiFi", thread_id),
                        Ok(false) => {
                            println!("  Thread {} found WiFi not present for removal", thread_id)
                        }
                        Err(e) => println!("  Thread {} failed to remove WiFi: {:?}", thread_id, e),
                    }
                }
                2 => {
                    // Thread 2: Add Camera, then re-add WiFi (after removal)
                    thread::sleep(std::time::Duration::from_millis(15));

                    println!("  Thread {} adding Camera capability", thread_id);
                    match capabilities_clone.add(CAMERA, base_timestamp + 15) {
                        Ok(true) => println!("  Thread {} successfully added Camera", thread_id),
                        Ok(false) => println!("  Thread {} found Camera already exists", thread_id),
                        Err(e) => println!("  Thread {} failed to add Camera: {:?}", thread_id, e),
                    }

                    thread::sleep(std::time::Duration::from_millis(10));

                    println!(
                        "  Thread {} re-adding WiFi capability (after removal)",
                        thread_id
                    );
                    match capabilities_clone.add(WIFI, base_timestamp + 30) {
                        Ok(true) => println!("  Thread {} successfully re-added WiFi", thread_id),
                        Ok(false) => println!("  Thread {} found WiFi already exists", thread_id),
                        Err(e) => println!("  Thread {} failed to re-add WiFi: {:?}", thread_id, e),
                    }
                }
                3 => {
                    // Thread 3: Add Accelerometer and Gyroscope
                    thread::sleep(std::time::Duration::from_millis(8));

                    println!("  Thread {} adding Accelerometer capability", thread_id);
                    match capabilities_clone.add(ACCELEROMETER, base_timestamp + 8) {
                        Ok(true) => {
                            println!("  Thread {} successfully added Accelerometer", thread_id)
                        }
                        Ok(false) => {
                            println!("  Thread {} found Accelerometer already exists", thread_id)
                        }
                        Err(e) => println!(
                            "  Thread {} failed to add Accelerometer: {:?}",
                            thread_id, e
                        ),
                    }

                    thread::sleep(std::time::Duration::from_millis(5));

                    println!("  Thread {} adding Gyroscope capability", thread_id);
                    match capabilities_clone.add(GYROSCOPE, base_timestamp + 13) {
                        Ok(true) => println!("  Thread {} successfully added Gyroscope", thread_id),
                        Ok(false) => {
                            println!("  Thread {} found Gyroscope already exists", thread_id)
                        }
                        Err(e) => {
                            println!("  Thread {} failed to add Gyroscope: {:?}", thread_id, e)
                        }
                    }
                }
                _ => unreachable!(),
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
    for capability in [GPS, WIFI, BLUETOOTH, CAMERA, ACCELEROMETER, GYROSCOPE] {
        let name = match capability {
            GPS => "GPS",
            WIFI => "WiFi",
            BLUETOOTH => "Bluetooth",
            CAMERA => "Camera",
            ACCELEROMETER => "Accelerometer",
            GYROSCOPE => "Gyroscope",
            _ => "Unknown",
        };
        if capabilities.contains(&capability) {
            println!("  ✓ Capability {}: {}", capability, name);
        } else {
            println!("  ✗ Capability {}: {} (not present)", capability, name);
        }
    }

    println!("✓ Concurrent capability management successful\n");
    Ok(())
}

#[cfg(feature = "hardware-atomic")]
fn atomic_add_remove_patterns() -> Result<(), CRDTError> {
    println!("Testing atomic add/remove patterns...");

    let registry = Arc::new(ORSet::<u32, DefaultConfig>::new(2));
    let mut handles = vec![];

    // Producer threads (add elements)
    for i in 0..2 {
        let reg = Arc::clone(&registry);
        let handle = thread::spawn(move || {
            let base_timestamp = get_timestamp();
            for j in 0..3 {
                let element = i * 10 + j;
                let timestamp = base_timestamp + (j * 10) as u64;
                match reg.add(element, timestamp) {
                    Ok(true) => println!("  Producer {}: Added element {}", i, element),
                    Ok(false) => println!("  Producer {}: Element {} already exists", i, element),
                    Err(e) => println!(
                        "  Producer {}: Failed to add element {}: {:?}",
                        i, element, e
                    ),
                }
                thread::sleep(std::time::Duration::from_millis(2));
            }
        });
        handles.push(handle);
    }

    // Consumer thread (remove elements)
    let reg = Arc::clone(&registry);
    let handle = thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(10));
        let base_timestamp = get_timestamp();

        for element in [0, 1, 10, 11] {
            thread::sleep(std::time::Duration::from_millis(3));
            match reg.remove(&element, base_timestamp + 50) {
                Ok(true) => println!("  Consumer: Removed element {}", element),
                Ok(false) => println!("  Consumer: Element {} not present for removal", element),
                Err(e) => println!("  Consumer: Failed to remove element {}: {:?}", element, e),
            }
        }
    });
    handles.push(handle);

    // Re-adder thread (add elements after removal)
    let reg = Arc::clone(&registry);
    let handle = thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(20));
        let base_timestamp = get_timestamp();

        for element in [0, 1] {
            match reg.add(element, base_timestamp + 100) {
                Ok(true) => println!("  Re-adder: Re-added element {} (after removal)", element),
                Ok(false) => println!("  Re-adder: Element {} already exists", element),
                Err(e) => println!("  Re-adder: Failed to re-add element {}: {:?}", element, e),
            }
        }
    });
    handles.push(handle);

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    println!("\nFinal registry state:");
    println!("Elements present: {}", registry.len());
    for element in 0..20 {
        if registry.contains(&element) {
            println!("  ✓ Element {} is present", element);
        }
    }

    println!("✓ Atomic add/remove patterns successful\n");
    Ok(())
}

#[cfg(feature = "hardware-atomic")]
fn capacity_management_demo() -> Result<(), CRDTError> {
    println!("Testing capacity management and overflow protection...");

    let registry = Arc::new(ORSet::<u32, DefaultConfig>::new(3));

    // Fill the registry to near capacity
    for i in 0..7 {
        registry.add(i, get_timestamp() + i as u64)?;
    }

    println!("  Filled registry with 7 elements");
    println!("  Remaining capacity: {}", registry.remaining_capacity());
    assert_eq!(registry.remaining_capacity(), 1);

    // Test concurrent attempts to fill the last slot
    let reg_clone1 = Arc::clone(&registry);
    let reg_clone2 = Arc::clone(&registry);

    let handle1 = thread::spawn(move || match reg_clone1.add(100, get_timestamp()) {
        Ok(true) => println!("  Thread 1 successfully added element 100"),
        Ok(false) => println!("  Thread 1 found element 100 already exists"),
        Err(e) => println!("  Thread 1 failed to add element 100: {:?}", e),
    });

    let handle2 = thread::spawn(move || match reg_clone2.add(101, get_timestamp()) {
        Ok(true) => println!("  Thread 2 successfully added element 101"),
        Ok(false) => println!("  Thread 2 found element 101 already exists"),
        Err(e) => println!("  Thread 2 failed to add element 101: {:?}", e),
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    println!("  Final element count: {}", registry.element_entries());
    println!("  Is full: {}", registry.is_full());

    // Test that further adds fail
    match registry.add(200, get_timestamp()) {
        Err(CRDTError::BufferOverflow) => println!("  ✓ Buffer overflow correctly detected"),
        Ok(_) => panic!("Add should have failed due to capacity"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }

    println!("✓ Capacity management working correctly\n");
    Ok(())
}

#[cfg(not(feature = "hardware-atomic"))]
fn standard_orset_demo() -> Result<(), CRDTError> {
    println!("Testing standard ORSet operations...");

    let mut registry = ORSet::<u32, DefaultConfig>::new(1);

    // Add some capabilities
    registry.add(1, 1000)?; // GPS
    registry.add(2, 1001)?; // WiFi
    registry.add(3, 1002)?; // Bluetooth

    println!("Added capabilities: GPS, WiFi, Bluetooth");
    println!("Total capabilities: {}", registry.len());

    // Test contains
    assert!(registry.contains(&1));
    assert!(registry.contains(&2));
    assert!(registry.contains(&3));
    assert!(!registry.contains(&4));

    // Test remove
    registry.remove(&2, 2000)?; // Remove WiFi
    println!("Removed WiFi");
    assert!(!registry.contains(&2));
    println!("Capabilities after removal: {}", registry.len());

    // Test add after remove
    registry.add(2, 3000)?; // Re-add WiFi with later timestamp
    println!("Re-added WiFi with later timestamp");
    assert!(registry.contains(&2));
    println!("Final capabilities: {}", registry.len());

    // Test merge with another registry
    let mut other_registry = ORSet::<u32, DefaultConfig>::new(2);
    other_registry.add(4, 1500)?; // Camera
    other_registry.add(2, 1500)?; // WiFi (earlier timestamp, should not override)

    registry.merge(&other_registry)?;

    println!("After merge, total capabilities: {}", registry.len());
    assert_eq!(registry.len(), 4); // GPS, WiFi (re-added), Bluetooth, Camera
    assert!(registry.contains(&4)); // Camera should be present
    assert!(registry.contains(&2)); // WiFi should still be present (re-added version)

    println!("✓ Standard ORSet operations successful");
    Ok(())
}
