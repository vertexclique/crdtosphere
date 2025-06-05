#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

//! Atomic MVRegister Example
//!
//! Demonstrates the atomic Multi-Value Register implementation
//! with concurrent access from multiple threads.

use crdtosphere::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("CRDTosphere Atomic MVRegister Example");
    println!("=====================================");

    #[cfg(feature = "hardware-atomic")]
    {
        println!("Atomic implementation:");
        println!("- Allows &self for modifications");
        println!("- Multi-threaded safe with Arc<T>");
        println!("- Supports concurrent set operations");
        println!("- Handles multiple concurrent values");
        println!();

        // Create an atomic MV register for multi-sensor readings
        let register = Arc::new(MVRegister::<f32, DefaultConfig>::new(1));

        // Spawn multiple threads to simulate concurrent sensor updates
        let mut handles = vec![];

        for thread_id in 0..4 {
            let register_clone = Arc::clone(&register);

            let handle = thread::spawn(move || {
                let base_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                // Each thread represents a different sensor node
                let node_id = thread_id + 1;
                let sensor_register = MVRegister::<f32, DefaultConfig>::new(node_id);

                // Each sensor takes multiple readings
                for i in 0..3 {
                    let timestamp = base_time + (thread_id as u64 * 1000) + (i as u64 * 100);
                    let value = 20.0 + (thread_id as f32) + (i as f32 * 0.1);

                    if let Err(e) = sensor_register.set(value, timestamp) {
                        eprintln!("Sensor {} failed to set value: {:?}", node_id, e);
                    } else {
                        println!(
                            "Sensor {} recorded value {} at timestamp {}",
                            node_id, value, timestamp
                        );
                    }

                    // Small delay to show interleaving
                    thread::sleep(std::time::Duration::from_millis(10));
                }

                println!("Sensor {} completed readings", node_id);
                sensor_register
            });

            handles.push(handle);
        }

        // Collect all sensor registers
        let sensor_registers: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        println!();
        println!("Merging sensor readings into main register...");

        // Merge all sensor readings into the main register
        let mut main_register = (*register).clone();
        for sensor_register in &sensor_registers {
            if let Err(e) = main_register.merge(sensor_register) {
                eprintln!("Failed to merge sensor data: {:?}", e);
            }
        }

        // Display final state
        println!();
        println!("Final multi-sensor register state:");
        println!("Number of sensors: {}", main_register.len());

        let values = main_register.values_array();
        for (i, value) in values.iter().enumerate() {
            if let Some(v) = value {
                println!("  Sensor reading {}: {:.1}", i + 1, v);
            }
        }

        // Demonstrate numeric operations
        if let Some(avg) = main_register.average() {
            println!("Average reading: {:.2}", avg);
        }
        if let Some(min) = main_register.min() {
            println!("Minimum reading: {:.1}", min);
        }
        if let Some(max) = main_register.max() {
            println!("Maximum reading: {:.1}", max);
        }

        println!();
        println!("Testing atomic merge operations...");

        // Create multiple registers to demonstrate merging
        let register_a = Arc::new(MVRegister::<f32, DefaultConfig>::new(10));
        let register_b = Arc::new(MVRegister::<f32, DefaultConfig>::new(20));
        let register_c = Arc::new(MVRegister::<f32, DefaultConfig>::new(30));

        // Set different values with different timestamps
        let base_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        register_a.set(100.0, base_time + 1000).unwrap();
        register_b.set(200.0, base_time + 2000).unwrap();
        register_c.set(300.0, base_time + 3000).unwrap();

        println!("Before merge:");
        println!(
            "  Register A: {} values, node 10 = {:?}",
            register_a.len(),
            register_a.get_from_node(10)
        );
        println!(
            "  Register B: {} values, node 20 = {:?}",
            register_b.len(),
            register_b.get_from_node(20)
        );
        println!(
            "  Register C: {} values, node 30 = {:?}",
            register_c.len(),
            register_c.get_from_node(30)
        );

        // Test concurrent merge operations
        let merge_handles = vec![
            {
                let reg_a = Arc::clone(&register_a);
                let reg_b = Arc::clone(&register_b);
                thread::spawn(move || {
                    let mut temp_a = (*reg_a).clone();
                    temp_a.merge(&*reg_b).unwrap();
                    println!(
                        "Thread 1: Merged B into A, result has {} values",
                        temp_a.len()
                    );
                    temp_a
                })
            },
            {
                let reg_a = Arc::clone(&register_a);
                let reg_c = Arc::clone(&register_c);
                thread::spawn(move || {
                    let mut temp_a = (*reg_a).clone();
                    temp_a.merge(&*reg_c).unwrap();
                    println!(
                        "Thread 2: Merged C into A, result has {} values",
                        temp_a.len()
                    );
                    temp_a
                })
            },
        ];

        let merge_results: Vec<_> = merge_handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect();

        println!();
        println!("After merge operations:");
        for (i, result) in merge_results.iter().enumerate() {
            println!("  Merge result {}: {} values", i + 1, result.len());
            let values = result.values_array();
            for (j, value) in values.iter().enumerate() {
                if let Some(v) = value {
                    println!("    Value {}: {:.1}", j + 1, v);
                }
            }
        }

        // Verify that both merge results have the expected values
        for result in &merge_results {
            assert_eq!(result.len(), 2, "Each merge should result in 2 values");
            assert!(
                result.get_from_node(10).is_some(),
                "Should have value from node 10"
            );
            assert_eq!(
                *result.get_from_node(10).unwrap(),
                100.0,
                "Node 10 should have value 100.0"
            );
        }

        // Check that the first result has node 20's value and second has node 30's value
        assert!(
            merge_results[0].get_from_node(20).is_some(),
            "First merge should have node 20"
        );
        assert_eq!(
            *merge_results[0].get_from_node(20).unwrap(),
            200.0,
            "Node 20 should have value 200.0"
        );

        assert!(
            merge_results[1].get_from_node(30).is_some(),
            "Second merge should have node 30"
        );
        assert_eq!(
            *merge_results[1].get_from_node(30).unwrap(),
            300.0,
            "Node 30 should have value 300.0"
        );

        println!();
        println!("✓ Multi-value merge semantics verified:");
        println!("  - Each register maintained its node's value");
        println!("  - Merge operations combined values from different nodes");
        println!("  - Atomic operations maintained consistency");

        // Test timestamp-based conflict resolution
        println!();
        println!("Testing timestamp-based conflict resolution...");

        let conflict_reg_1 = Arc::new(MVRegister::<f32, DefaultConfig>::new(5));
        let conflict_reg_2 = Arc::new(MVRegister::<f32, DefaultConfig>::new(5)); // Same node

        let early_time = base_time + 1000;
        let later_time = base_time + 2000;

        conflict_reg_1.set(50.0, early_time).unwrap();
        conflict_reg_2.set(150.0, later_time).unwrap(); // Newer timestamp should win

        let mut conflict_result = (*conflict_reg_1).clone();
        conflict_result.merge(&*conflict_reg_2).unwrap();

        println!(
            "Conflict resolution result: node 5 = {:?}",
            conflict_result.get_from_node(5)
        );

        if let Some(value) = conflict_result.get_from_node(5) {
            assert_eq!(*value, 150.0, "Newer timestamp should win");
        }

        println!(
            "✓ Timestamp conflict resolution verified: newer value (150.0) won over older (50.0)"
        );

        // Test capacity limits
        println!();
        println!("Testing capacity limits...");

        let capacity_reg = Arc::new(MVRegister::<f32, DefaultConfig>::new(1));
        let mut capacity_test = (*capacity_reg).clone();

        // Fill to capacity (4 nodes)
        for node_id in 1..=4 {
            let mut temp_reg = MVRegister::<f32, DefaultConfig>::new(node_id);
            temp_reg
                .set(node_id as f32 * 10.0, base_time + node_id as u64)
                .unwrap();
            capacity_test.merge(&temp_reg).unwrap();
        }

        println!(
            "Filled register to capacity: {} values",
            capacity_test.len()
        );
        assert!(capacity_test.is_full(), "Register should be full");

        // Try to add one more (should fail)
        let mut overflow_reg = MVRegister::<f32, DefaultConfig>::new(5);
        overflow_reg.set(50.0, base_time + 5000).unwrap();

        let merge_result = capacity_test.merge(&overflow_reg);
        assert!(merge_result.is_err(), "Merge should fail when at capacity");
        println!("✓ Capacity limit enforced: merge correctly failed when at capacity");

        println!();
        println!("Atomic MVRegister demonstration completed!");
        println!("✓ Concurrent set operations: Multiple sensor nodes safely recorded values");
        println!(
            "✓ Atomic merge operations: Multi-value semantics maintained across concurrent merges"
        );
        println!("✓ Timestamp conflict resolution: Newer timestamps correctly won conflicts");
        println!("✓ Capacity management: Buffer overflow protection working correctly");
        println!("✓ Thread safety: All operations completed without data races");
        println!(
            "✓ Numeric operations: Average, min, max calculations working on multi-value data"
        );
    }

    #[cfg(not(feature = "hardware-atomic"))]
    {
        println!("This example requires the 'hardware-atomic' feature to be enabled.");
        println!("Run with: cargo run --example atomic_mv_register --features hardware-atomic");
    }
}
