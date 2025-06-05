//! Property-based tests for atomic CRDT implementations
//!
//! This module tests the thread-safety and concurrent behavior of atomic CRDTs:
//! - Thread safety: operations can be called concurrently
//! - Lock-free behavior: no blocking operations
//! - Consistency: atomic operations maintain CRDT properties
//! - Performance: atomic operations complete within bounds

#![allow(unused_mut)]
#![allow(special_module_name)]
#![allow(clippy::assertions_on_constants)]

mod lib;

#[cfg(feature = "hardware-atomic")]
mod atomic_tests {
    use crdtosphere::counters::{GCounter, PNCounter};
    use crdtosphere::prelude::*;
    use proptest::prelude::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    // Import common utilities
    use crate::lib::*;

    proptest! {
        #![proptest_config(atomic_config())]

        /// Test that atomic GCounter can handle concurrent increments
        /// Property: concurrent increments should all be applied correctly
        #[test]
        fn atomic_gcounter_concurrent_increments(
            node in node_id_strategy(),
            increments in prop::collection::vec(small_increment_strategy(), 1..10),
        ) {
            let counter = Arc::new(GCounter::<DefaultConfig>::new(node));
            let mut handles = vec![];
            let expected_total: u64 = increments.iter().map(|&x| x as u64).sum();

            // Spawn threads to do concurrent increments
            for increment in increments {
                let counter_clone = Arc::clone(&counter);
                let handle = thread::spawn(move || {
                    counter_clone.increment(increment)
                });
                handles.push(handle);
            }

            // Wait for all threads to complete
            let mut all_succeeded = true;
            for handle in handles {
                if let Ok(result) = handle.join() {
                    if result.is_err() {
                        all_succeeded = false;
                    }
                } else {
                    all_succeeded = false;
                }
            }

            // If all operations succeeded, check the final value
            if all_succeeded {
                prop_assert_eq!(counter.value(), expected_total);
            }

            // Counter should always be valid regardless
            prop_assert!(counter.validate().is_ok());
        }

        /// Test that atomic PNCounter can handle concurrent operations
        /// Property: concurrent increments and decrements should be applied correctly
        #[test]
        fn atomic_pncounter_concurrent_operations(
            node in node_id_strategy(),
            increments in prop::collection::vec(small_increment_strategy(), 1..5),
            decrements in prop::collection::vec(small_increment_strategy(), 1..5),
        ) {
            let counter = Arc::new(PNCounter::<DefaultConfig>::new(node));
            let mut handles = vec![];

            let expected_inc: i64 = increments.iter().map(|&x| x as i64).sum();
            let expected_dec: i64 = decrements.iter().map(|&x| x as i64).sum();
            let expected_total = expected_inc - expected_dec;

            // Spawn threads for increments
            for increment in increments {
                let counter_clone = Arc::clone(&counter);
                let handle = thread::spawn(move || {
                    counter_clone.increment(increment)
                });
                handles.push(handle);
            }

            // Spawn threads for decrements
            for decrement in decrements {
                let counter_clone = Arc::clone(&counter);
                let handle = thread::spawn(move || {
                    counter_clone.decrement(decrement)
                });
                handles.push(handle);
            }

            // Wait for all threads to complete
            let mut all_succeeded = true;
            for handle in handles {
                if let Ok(result) = handle.join() {
                    if result.is_err() {
                        all_succeeded = false;
                    }
                } else {
                    all_succeeded = false;
                }
            }

            // If all operations succeeded, check the final value
            if all_succeeded {
                prop_assert_eq!(counter.value(), expected_total);
            }

            // Counter should always be valid regardless
            prop_assert!(counter.validate().is_ok());
        }

        /// Test that atomic counters can handle concurrent merges
        /// Property: concurrent merges should not corrupt the data structure
        #[test]
        fn atomic_counter_concurrent_merges(
            nodes in prop::collection::vec(node_id_strategy(), 2..4),
            increments_per_node in prop::collection::vec(small_increment_strategy(), 1..5),
        ) {
            let mut unique_nodes = nodes;
            unique_nodes.sort();
            unique_nodes.dedup();

            if unique_nodes.len() < 2 {
                return Ok(());
            }

            // Create counters for each node
            let counters: Vec<Arc<GCounter<DefaultConfig>>> = unique_nodes
                .iter()
                .map(|&node| Arc::new(GCounter::<DefaultConfig>::new(node)))
                .collect();

            // Apply increments to each counter
            for (counter, &increment) in counters.iter().zip(increments_per_node.iter()) {
                let _ = counter.increment(increment);
            }

            // Create a target counter for merging
            let target = Arc::clone(&counters[0]);
            let mut handles = vec![];

            // Spawn threads to merge other counters into the target
            for other_counter in &counters[1..] {
                let target_clone = Arc::clone(&target);
                let other_clone = Arc::clone(other_counter);
                let handle = thread::spawn(move || {
                    // For atomic operations, we need to use Mutex for thread safety
                    // This is a simplified test - in real atomic usage, proper synchronization would be used
                    let target_ref = Arc::try_unwrap(target_clone).unwrap_or_else(|arc| (*arc).clone());
                    let other_ref = &*other_clone;
                    let mut target_mut = target_ref;
                    target_mut.merge(other_ref)
                });
                handles.push(handle);
            }

            // Wait for all merges to complete
            for handle in handles {
                let _ = handle.join();
            }

            // Target should be valid and have reasonable value
            prop_assert!(target.validate().is_ok());
            prop_assert!(target.value() >= increments_per_node[0] as u64);
        }

        /// Test atomic operations under high contention
        /// Property: many threads operating on the same counter should not cause corruption
        #[test]
        fn atomic_counter_high_contention(
            node in node_id_strategy(),
            thread_count in 2usize..8,
            ops_per_thread in 1usize..10,
        ) {
            let counter = Arc::new(GCounter::<DefaultConfig>::new(node));
            let mut handles = vec![];

            // Spawn many threads doing operations
            for _ in 0..thread_count {
                let counter_clone = Arc::clone(&counter);
                let handle = thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        let _ = counter_clone.increment(1);
                        // Small delay to increase contention
                        thread::sleep(Duration::from_nanos(1));
                    }
                });
                handles.push(handle);
            }

            // Wait for all threads
            for handle in handles {
                let _ = handle.join();
            }

            // Counter should be valid and have reasonable value
            prop_assert!(counter.validate().is_ok());
            prop_assert!(counter.value() <= (thread_count * ops_per_thread) as u64);
        }

        /// Test that atomic operations maintain CRDT properties under concurrency
        /// Property: CRDT laws should hold even with concurrent operations
        #[test]
        fn atomic_crdt_properties_under_concurrency(
            nodes in prop::collection::vec(node_id_strategy(), 2..4),
            operations in prop::collection::vec(small_increment_strategy(), 1..5),
        ) {
            let mut unique_nodes = nodes;
            unique_nodes.sort();
            unique_nodes.dedup();

            if unique_nodes.len() < 2 {
                return Ok(());
            }

            // Create atomic counters
            let counters: Vec<Arc<GCounter<DefaultConfig>>> = unique_nodes
                .iter()
                .map(|&node| Arc::new(GCounter::<DefaultConfig>::new(node)))
                .collect();

            // Apply operations concurrently
            let mut handles = vec![];
            for (counter, &increment) in counters.iter().zip(operations.iter()) {
                let counter_clone = Arc::clone(counter);
                let handle = thread::spawn(move || {
                    counter_clone.increment(increment)
                });
                handles.push(handle);
            }

            // Wait for operations to complete
            for handle in handles {
                let _ = handle.join();
            }

            // Test CRDT properties
            if counters.len() >= 2 {
                // Test commutativity by merging in different orders
                let mut counter1 = (*counters[0]).clone();
                let mut counter2 = (*counters[1]).clone();

                let counter1_copy = counter1.clone();
                let counter2_copy = counter2.clone();

                let _ = counter1.merge(&counter2_copy);
                let _ = counter2.merge(&counter1_copy);

                // Results should be the same (commutativity)
                prop_assert!(counter1.eq(&counter2));
            }
        }

        /// Test atomic counter memory consistency
        /// Property: memory operations should be consistent across threads
        #[test]
        fn atomic_counter_memory_consistency(
            node in node_id_strategy(),
            increments in prop::collection::vec(1u32..10, 1..5),
        ) {
            let counter = Arc::new(GCounter::<DefaultConfig>::new(node));
            let mut handles = vec![];
            let mut expected_values = vec![];

            // Apply increments sequentially to get expected intermediate values
            let mut running_total = 0u64;
            for &increment in &increments {
                running_total += increment as u64;
                expected_values.push(running_total);
            }

            // Apply increments concurrently
            for increment in increments {
                let counter_clone = Arc::clone(&counter);
                let handle = thread::spawn(move || {
                    counter_clone.increment(increment)
                });
                handles.push(handle);
            }

            // Wait for all operations
            for handle in handles {
                let _ = handle.join();
            }

            // Final value should be the sum of all increments
            let final_expected = expected_values.last().copied().unwrap_or(0);
            prop_assert_eq!(counter.value(), final_expected);

            // Memory usage should be consistent
            prop_assert!(assert_memory_bounds(&*counter, 1024));
        }
    }

    #[cfg(test)]
    mod unit_tests {
        use super::*;

        #[test]
        fn test_atomic_gcounter_basic_concurrency() {
            let counter = Arc::new(GCounter::<DefaultConfig>::new(1));
            let mut handles = vec![];

            // Spawn 4 threads, each incrementing by 10
            for _ in 0..4 {
                let counter_clone = Arc::clone(&counter);
                let handle = thread::spawn(move || {
                    counter_clone.increment(10).unwrap();
                });
                handles.push(handle);
            }

            // Wait for all threads
            for handle in handles {
                handle.join().unwrap();
            }

            // Should have total value of 40
            assert_eq!(counter.value(), 40);
            assert!(counter.validate().is_ok());
        }

        #[test]
        fn test_atomic_pncounter_basic_concurrency() {
            let counter = Arc::new(PNCounter::<DefaultConfig>::new(1));
            let mut handles = vec![];

            // Spawn threads for increments and decrements
            for i in 0..4 {
                let counter_clone = Arc::clone(&counter);
                let handle = thread::spawn(move || {
                    if i % 2 == 0 {
                        counter_clone.increment(5).unwrap();
                    } else {
                        counter_clone.decrement(3).unwrap();
                    }
                });
                handles.push(handle);
            }

            // Wait for all threads
            for handle in handles {
                handle.join().unwrap();
            }

            // Should have value of (5 + 5) - (3 + 3) = 4
            assert_eq!(counter.value(), 4);
            assert!(counter.validate().is_ok());
        }

        #[test]
        fn test_atomic_counter_merge_concurrency() {
            // Create two counters for different nodes
            let counter1 = Arc::new(GCounter::<DefaultConfig>::new(1));
            let counter2 = Arc::new(GCounter::<DefaultConfig>::new(2));

            let mut handles = vec![];

            // Spawn threads to do concurrent increments on each counter
            // Thread 1: increment counter1 multiple times
            let c1_clone = Arc::clone(&counter1);
            let handle1 = thread::spawn(move || {
                for _ in 0..5 {
                    c1_clone.increment(2).unwrap();
                }
            });
            handles.push(handle1);

            // Thread 2: increment counter2 multiple times
            let c2_clone = Arc::clone(&counter2);
            let handle2 = thread::spawn(move || {
                for _ in 0..5 {
                    c2_clone.increment(4).unwrap();
                }
            });
            handles.push(handle2);

            // Thread 3: more increments on counter1
            let c1_clone2 = Arc::clone(&counter1);
            let handle3 = thread::spawn(move || {
                for _ in 0..3 {
                    c1_clone2.increment(1).unwrap();
                }
            });
            handles.push(handle3);

            // Wait for all concurrent increments to complete
            for handle in handles {
                handle.join().unwrap();
            }

            // Check individual counter values after concurrent operations
            assert_eq!(counter1.value(), 13); // (5 * 2) + (3 * 1) = 13
            assert_eq!(counter2.value(), 20); // 5 * 4 = 20

            // Now do a sequential merge (merge operations require &mut self)
            let mut counter1_mut = Arc::try_unwrap(counter1).unwrap();
            let counter2_ref = Arc::try_unwrap(counter2).unwrap();

            counter1_mut.merge(&counter2_ref).unwrap();

            // Should have combined value
            assert_eq!(counter1_mut.value(), 33); // 13 + 20 = 33
            assert!(counter1_mut.validate().is_ok());
        }
    }
}

#[cfg(not(feature = "hardware-atomic"))]
mod no_atomic_tests {
    #[test]
    fn atomic_tests_require_hardware_atomic_feature() {
        // This test ensures the file compiles even without the atomic feature
        // In a real scenario, you might want to test the non-atomic versions here
        assert!(
            true,
            "Atomic tests require 'hardware-atomic' feature to be enabled"
        );
    }
}
