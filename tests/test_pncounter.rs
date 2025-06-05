//! Property-based tests for PNCounter CRDT
//!
//! This module tests the mathematical properties that PNCounter must satisfy:
//! - Commutativity: merge(a, b) = merge(b, a)
//! - Associativity: merge(merge(a, b), c) = merge(a, merge(b, c))
//! - Idempotence: merge(a, a) = a
//! - Increment/Decrement semantics: values can increase and decrease
//! - Eventual consistency: all replicas converge

#![allow(unused_mut)]
#![allow(special_module_name)]

use crdtosphere::counters::PNCounter;
use crdtosphere::prelude::*;
use proptest::prelude::*;

mod lib;
use lib::*;

/// Generate increment or decrement operations
#[derive(Debug, Clone)]
enum CounterOp {
    Increment(u32),
    Decrement(u32),
}

fn counter_operation_strategy() -> impl Strategy<Value = CounterOp> {
    prop_oneof![
        small_increment_strategy().prop_map(CounterOp::Increment),
        small_increment_strategy().prop_map(CounterOp::Decrement),
    ]
}

proptest! {
    #![proptest_config(crdt_config())]

    /// Test that PNCounter merge operation is commutative
    /// Property: merge(a, b) = merge(b, a)
    #[test]
    fn pncounter_merge_is_commutative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        operations1 in operation_sequence_strategy(counter_operation_strategy()),
        operations2 in operation_sequence_strategy(counter_operation_strategy()),
    ) {
        let mut counter1 = PNCounter::<DefaultConfig>::new(node1);
        let mut counter2 = PNCounter::<DefaultConfig>::new(node2);

        // Apply operations to each counter
        for op in operations1 {
            match op {
                CounterOp::Increment(amount) => { let _ = counter1.increment(amount); }
                CounterOp::Decrement(amount) => { let _ = counter1.decrement(amount); }
            }
        }
        for op in operations2 {
            match op {
                CounterOp::Increment(amount) => { let _ = counter2.increment(amount); }
                CounterOp::Decrement(amount) => { let _ = counter2.decrement(amount); }
            }
        }

        // Test commutativity
        prop_assert!(assert_crdt_commutativity(&counter1, &counter2));
    }

    /// Test that PNCounter merge operation is associative
    /// Property: merge(merge(a, b), c) = merge(a, merge(b, c))
    #[test]
    fn pncounter_merge_is_associative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        node3 in node_id_strategy(),
        operations1 in operation_sequence_strategy(counter_operation_strategy()),
        operations2 in operation_sequence_strategy(counter_operation_strategy()),
        operations3 in operation_sequence_strategy(counter_operation_strategy()),
    ) {
        let mut counter1 = PNCounter::<DefaultConfig>::new(node1);
        let mut counter2 = PNCounter::<DefaultConfig>::new(node2);
        let mut counter3 = PNCounter::<DefaultConfig>::new(node3);

        // Apply operations to each counter
        for op in operations1 {
            match op {
                CounterOp::Increment(amount) => { let _ = counter1.increment(amount); }
                CounterOp::Decrement(amount) => { let _ = counter1.decrement(amount); }
            }
        }
        for op in operations2 {
            match op {
                CounterOp::Increment(amount) => { let _ = counter2.increment(amount); }
                CounterOp::Decrement(amount) => { let _ = counter2.decrement(amount); }
            }
        }
        for op in operations3 {
            match op {
                CounterOp::Increment(amount) => { let _ = counter3.increment(amount); }
                CounterOp::Decrement(amount) => { let _ = counter3.decrement(amount); }
            }
        }

        // Test associativity
        prop_assert!(assert_crdt_associativity(&counter1, &counter2, &counter3));
    }

    /// Test that PNCounter merge operation is idempotent
    /// Property: merge(a, a) = a
    #[test]
    fn pncounter_merge_is_idempotent(
        node in node_id_strategy(),
        operations in operation_sequence_strategy(counter_operation_strategy()),
    ) {
        let mut counter = PNCounter::<DefaultConfig>::new(node);

        // Apply operations
        for op in operations {
            match op {
                CounterOp::Increment(amount) => { let _ = counter.increment(amount); }
                CounterOp::Decrement(amount) => { let _ = counter.decrement(amount); }
            }
        }

        // Test idempotence
        prop_assert!(assert_crdt_idempotence(&counter));
    }

    /// Test eventual consistency across multiple replicas
    /// Property: all replicas converge to the same state after merging
    #[test]
    fn pncounter_eventual_consistency(
        nodes in prop::collection::vec(node_id_strategy(), 2..5),
        operations in prop::collection::vec(
            (any::<usize>(), counter_operation_strategy()),
            0..20
        ),
    ) {
        // Create replicas with unique node IDs
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        if unique_nodes.len() < 2 {
            return Ok(());
        }

        let mut replicas: Vec<PNCounter<DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| PNCounter::<DefaultConfig>::new(node))
            .collect();

        // Apply operations to random replicas
        for (replica_idx, op) in operations {
            if !replicas.is_empty() {
                let idx = replica_idx % replicas.len();
                match op {
                    CounterOp::Increment(amount) => { let _ = replicas[idx].increment(amount); }
                    CounterOp::Decrement(amount) => { let _ = replicas[idx].decrement(amount); }
                }
            }
        }

        // Test eventual consistency
        prop_assert!(assert_eventual_consistency(&replicas));
    }

    /// Test that PNCounter increment/decrement operations work correctly
    /// Property: increment increases value, decrement decreases value
    #[test]
    fn pncounter_increment_decrement_semantics(
        node in node_id_strategy(),
        increment_amount in small_increment_strategy(),
        decrement_amount in small_increment_strategy(),
    ) {
        let mut counter = PNCounter::<DefaultConfig>::new(node);
        let initial_value = counter.value();

        // Test increment
        if counter.increment(increment_amount).is_ok() {
            prop_assert_eq!(counter.value(), initial_value + increment_amount as i64);

            // Test decrement
            if counter.decrement(decrement_amount).is_ok() {
                let expected = initial_value + increment_amount as i64 - decrement_amount as i64;
                prop_assert_eq!(counter.value(), expected);
            }
        }
    }

    /// Test that PNCounter respects memory bounds
    /// Property: memory usage is always within expected bounds
    #[test]
    fn pncounter_respects_memory_bounds(
        node in node_id_strategy(),
        operations in operation_sequence_strategy(counter_operation_strategy()),
    ) {
        let mut counter = PNCounter::<DefaultConfig>::new(node);

        // Apply operations
        for op in operations {
            match op {
                CounterOp::Increment(amount) => { let _ = counter.increment(amount); }
                CounterOp::Decrement(amount) => { let _ = counter.decrement(amount); }
            }
        }

        // Test memory bounds (should be well under 1KB)
        prop_assert!(assert_memory_bounds(&counter, 1024));
    }

    /// Test that PNCounter operations complete within real-time bounds
    /// Property: all operations complete within expected time
    #[test]
    fn pncounter_respects_realtime_bounds(
        node in node_id_strategy(),
        operations in operation_sequence_strategy(counter_operation_strategy()),
    ) {
        let mut counter = PNCounter::<DefaultConfig>::new(node);

        // Apply operations
        for op in operations {
            match op {
                CounterOp::Increment(amount) => { let _ = counter.increment(amount); }
                CounterOp::Decrement(amount) => { let _ = counter.decrement(amount); }
            }
        }

        // Test real-time bounds
        prop_assert!(assert_realtime_bounds(&counter));
    }

    /// Test PNCounter overflow protection
    /// Property: operations near overflow should be handled safely
    #[test]
    fn pncounter_overflow_protection(
        node in node_id_strategy(),
        large_increment in (u32::MAX - 1000)..u32::MAX,
    ) {
        let mut counter = PNCounter::<DefaultConfig>::new(node);

        // First increment to near max
        if counter.increment(u32::MAX - 100).is_ok() {
            // This should fail due to overflow protection
            let result = counter.increment(large_increment);
            prop_assert!(result.is_err());

            // Counter should still be valid
            prop_assert!(counter.validate().is_ok());
        }
    }

    /// Test that empty counters behave correctly
    /// Property: empty counters have value 0 and merge correctly
    #[test]
    fn pncounter_empty_behavior(
        nodes in prop::collection::vec(node_id_strategy(), 1..5),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        let counters: Vec<PNCounter<DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| PNCounter::<DefaultConfig>::new(node))
            .collect();

        // All empty counters should have value 0
        for counter in &counters {
            prop_assert_eq!(counter.value(), 0);
            prop_assert!(counter.is_empty());
        }

        // Merging empty counters should result in empty counter
        if counters.len() >= 2 {
            let mut merged = counters[0].clone();
            for other in &counters[1..] {
                let _ = merged.merge(other);
            }
            prop_assert_eq!(merged.value(), 0);
            prop_assert!(merged.is_empty());
        }
    }

    /// Test node value isolation for both increment and decrement
    /// Property: operations on one node don't affect other nodes directly
    #[test]
    fn pncounter_node_isolation(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        increment1 in small_increment_strategy(),
        decrement1 in small_increment_strategy(),
        increment2 in small_increment_strategy(),
        decrement2 in small_increment_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }
        let mut counter1 = PNCounter::<DefaultConfig>::new(node1);
        let mut counter2 = PNCounter::<DefaultConfig>::new(node2);

        // Apply operations
        let _ = counter1.increment(increment1);
        let _ = counter1.decrement(decrement1);
        let _ = counter2.increment(increment2);
        let _ = counter2.decrement(decrement2);

        let expected1 = increment1 as i64 - decrement1 as i64;
        let expected2 = increment2 as i64 - decrement2 as i64;

        // Check node isolation before merge
        prop_assert_eq!(counter1.value(), expected1);
        prop_assert_eq!(counter2.value(), expected2);

        // After merge, both should have combined values
        let _ = counter1.merge(&counter2);
        prop_assert_eq!(counter1.value(), expected1 + expected2);
    }

    /// Test that PNCounter can handle negative values correctly
    /// Property: decrements can make the counter negative
    #[test]
    fn pncounter_negative_values(
        node in node_id_strategy(),
        small_increment in 1u32..50,
        large_decrement in 100u32..200,
    ) {
        let mut counter = PNCounter::<DefaultConfig>::new(node);

        // First increment by a small amount
        let _ = counter.increment(small_increment);
        prop_assert_eq!(counter.value(), small_increment as i64);

        // Then decrement by a larger amount
        let _ = counter.decrement(large_decrement);
        let expected = small_increment as i64 - large_decrement as i64;
        prop_assert_eq!(counter.value(), expected);
        prop_assert!(counter.value() < 0);
    }

    /// Test increment and decrement convenience methods
    /// Property: inc() and dec() work the same as increment(1) and decrement(1)
    #[test]
    fn pncounter_convenience_methods(
        node in node_id_strategy(),
        inc_count in 1usize..10,
        dec_count in 1usize..10,
    ) {
        let mut counter1 = PNCounter::<DefaultConfig>::new(node);
        let mut counter2 = PNCounter::<DefaultConfig>::new(node);

        // Use convenience methods on counter1
        for _ in 0..inc_count {
            let _ = counter1.inc();
        }
        for _ in 0..dec_count {
            let _ = counter1.dec();
        }

        // Use explicit methods on counter2
        let _ = counter2.increment(inc_count as u32);
        let _ = counter2.decrement(dec_count as u32);

        // Both should have the same result
        prop_assert_eq!(counter1.value(), counter2.value());
        prop_assert_eq!(counter1.value(), inc_count as i64 - dec_count as i64);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_basic_pncounter_properties() {
        let mut counter1 = PNCounter::<DefaultConfig>::new(1);
        let mut counter2 = PNCounter::<DefaultConfig>::new(2);

        counter1.increment(10).unwrap();
        counter1.decrement(3).unwrap();
        counter2.increment(5).unwrap();
        counter2.decrement(2).unwrap();

        // Test basic functionality
        assert_eq!(counter1.value(), 7); // 10 - 3
        assert_eq!(counter2.value(), 3); // 5 - 2

        // Test merge
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 10); // 7 + 3

        // Test properties
        assert!(assert_crdt_idempotence(&counter1));
        assert!(assert_memory_bounds(&counter1, 1024));
        assert!(assert_realtime_bounds(&counter1));
    }

    #[test]
    fn test_pncounter_negative_values() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        counter.increment(5).unwrap();
        counter.decrement(10).unwrap();

        assert_eq!(counter.value(), -5);
        assert!(!counter.is_empty()); // Negative values are not "empty"
    }

    #[test]
    fn test_pncounter_convenience_methods() {
        let mut counter = PNCounter::<DefaultConfig>::new(1);

        counter.inc().unwrap();
        counter.inc().unwrap();
        counter.dec().unwrap();

        assert_eq!(counter.value(), 1); // 2 - 1
    }
}
