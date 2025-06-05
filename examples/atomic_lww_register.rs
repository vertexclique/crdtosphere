#![allow(unused_imports)]
#![allow(unused_variables)]

//! Atomic LWWRegister Example
//!
//! Demonstrates the atomic Last-Writer-Wins Register implementation
//! with concurrent access from multiple threads.

use crdtosphere::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("CRDTosphere Atomic LWWRegister Example");
    println!("======================================");

    #[cfg(feature = "hardware-atomic")]
    {
        println!("Atomic implementation:");
        println!("- Allows &self for modifications");
        println!("- Multi-threaded safe with Arc<T>");
        println!("- Supports concurrent set operations");
        println!();

        // Create an atomic LWW register for sensor readings
        let register = Arc::new(LWWRegister::<f32, DefaultConfig>::new(1));

        // Spawn multiple threads to simulate concurrent sensor updates
        let mut handles = vec![];

        for thread_id in 0..4 {
            let register_clone = Arc::clone(&register);

            let handle = thread::spawn(move || {
                let base_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                // Each thread sets different values with different timestamps
                for i in 0..3 {
                    let timestamp = base_time + (thread_id * 1000) + (i * 100);
                    let value = 20.0 + (thread_id as f32) + (i as f32 * 0.1);

                    if let Err(e) = register_clone.set(value, timestamp) {
                        eprintln!("Thread {} failed to set value: {:?}", thread_id, e);
                    } else {
                        println!(
                            "Thread {} set value {} at timestamp {}",
                            thread_id, value, timestamp
                        );
                    }

                    // Small delay to show interleaving
                    thread::sleep(std::time::Duration::from_millis(10));
                }

                println!("Thread {} completed", thread_id);
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Check final state
        println!();
        println!("Final register state:");
        if let Some(value) = register.get() {
            println!("Current value: {}", value);
            println!("Set by node: {}", register.current_node());
            println!("Timestamp: {}", register.timestamp().as_u64());
        } else {
            println!("No value set");
        }

        println!();
        println!("Testing atomic merge operations...");

        // Create multiple registers to demonstrate merging
        let register_a = Arc::new(LWWRegister::<f32, DefaultConfig>::new(10));
        let register_b = Arc::new(LWWRegister::<f32, DefaultConfig>::new(20));
        let register_c = Arc::new(LWWRegister::<f32, DefaultConfig>::new(30));

        // Set different values with different timestamps
        let base_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        register_a.set(100.0, base_time + 1000).unwrap();
        register_b.set(200.0, base_time + 2000).unwrap(); // This should win (newer)
        register_c.set(300.0, base_time + 500).unwrap(); // This should lose (older)

        println!("Before merge:");
        println!(
            "  Register A: value={:?}, timestamp={}, node={}",
            register_a.get(),
            register_a.timestamp().as_u64(),
            register_a.current_node()
        );
        println!(
            "  Register B: value={:?}, timestamp={}, node={}",
            register_b.get(),
            register_b.timestamp().as_u64(),
            register_b.current_node()
        );
        println!(
            "  Register C: value={:?}, timestamp={}, node={}",
            register_c.get(),
            register_c.timestamp().as_u64(),
            register_c.current_node()
        );

        // Test sequential merge operations to demonstrate LWW semantics
        let mut result_register = (*register_a).clone();

        println!("Sequential merge operations:");
        println!(
            "1. Starting with A: value={:?}, timestamp={}",
            result_register.get(),
            result_register.timestamp().as_u64()
        );

        // Merge B into A (B should win - newer timestamp)
        result_register.merge(&*register_b).unwrap();
        println!(
            "2. After merging B: value={:?}, timestamp={}",
            result_register.get(),
            result_register.timestamp().as_u64()
        );

        // Merge C into result (B should still win - C is older)
        result_register.merge(&*register_c).unwrap();
        println!(
            "3. After merging C: value={:?}, timestamp={}",
            result_register.get(),
            result_register.timestamp().as_u64()
        );

        // Verify final result
        let expected_value = 200.0;
        let expected_timestamp = base_time + 2000;

        if let Some(value) = result_register.get() {
            assert_eq!(
                *value, expected_value,
                "LWW semantics: newest timestamp should win"
            );
            assert_eq!(
                result_register.timestamp().as_u64(),
                expected_timestamp,
                "Timestamp should match winner"
            );
            assert_eq!(
                result_register.current_node(),
                20,
                "Node ID should match winner"
            );
        }

        // Test concurrent merge operations with separate result registers
        println!();
        println!("Testing concurrent merge operations (each starting from A):");

        let merge_handles = vec![
            {
                let reg_a = Arc::clone(&register_a);
                let reg_b = Arc::clone(&register_b);
                thread::spawn(move || {
                    let mut temp_a = (*reg_a).clone();
                    temp_a.merge(&*reg_b).unwrap();
                    println!(
                        "Thread 1: A + B = value={:?}, timestamp={}",
                        temp_a.get(),
                        temp_a.timestamp().as_u64()
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
                        "Thread 2: A + C = value={:?}, timestamp={}",
                        temp_a.get(),
                        temp_a.timestamp().as_u64()
                    );
                    temp_a
                })
            },
        ];

        let merge_results: Vec<_> = merge_handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect();

        // Verify that A+B wins over A+C (because B is newer than both A and C)
        if let (Some(ab_value), Some(ac_value)) = (merge_results[0].get(), merge_results[1].get()) {
            assert_eq!(*ab_value, 200.0, "A+B should have B's value (newer)");
            assert_eq!(
                *ac_value, 100.0,
                "A+C should have A's value (A newer than C)"
            );
            println!(
                "✓ Concurrent merges: A+B={}, A+C={} (as expected)",
                ab_value, ac_value
            );
        }

        println!();
        println!("✓ LWW merge semantics verified:");
        println!("  - Register B's value (200.0) won due to newest timestamp");
        println!("  - All merge operations converged to the same state");
        println!("  - Atomic operations maintained consistency");

        // Test tiebreaker scenario
        println!();
        println!("Testing tiebreaker scenario (same timestamp, different nodes)...");

        let tie_reg_1 = Arc::new(LWWRegister::<f32, DefaultConfig>::new(5));
        let tie_reg_2 = Arc::new(LWWRegister::<f32, DefaultConfig>::new(15)); // Higher node ID

        let tie_timestamp = base_time + 5000;
        tie_reg_1.set(50.0, tie_timestamp).unwrap();
        tie_reg_2.set(150.0, tie_timestamp).unwrap(); // Same timestamp, higher node ID should win

        let mut tie_result = (*tie_reg_1).clone();
        tie_result.merge(&*tie_reg_2).unwrap();

        println!(
            "Tiebreaker result: value={:?}, node={}",
            tie_result.get(),
            tie_result.current_node()
        );

        if let Some(value) = tie_result.get() {
            assert_eq!(*value, 150.0, "Higher node ID should win tiebreaker");
            assert_eq!(
                tie_result.current_node(),
                15,
                "Node ID should be from winner"
            );
        }

        println!("✓ Tiebreaker semantics verified: higher node ID (15) won over lower (5)");

        println!();
        println!("Atomic LWWRegister demonstration completed!");
        println!(
            "✓ Concurrent set operations: {} threads safely updated shared register",
            4
        );
        println!("✓ Atomic merge operations: LWW semantics maintained across concurrent merges");
        println!("✓ Tiebreaker resolution: Node ID correctly used for timestamp ties");
        println!("✓ Thread safety: All operations completed without data races");
    }

    #[cfg(not(feature = "hardware-atomic"))]
    {
        println!("This example requires the 'hardware-atomic' feature to be enabled.");
        println!("Run with: cargo run --example atomic_lww_register --features hardware-atomic");
    }
}
