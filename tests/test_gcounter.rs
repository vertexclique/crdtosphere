//! Property-based tests for GCounter CRDT
//!
//! This module tests the mathematical properties that GCounter must satisfy:
//! - Commutativity: merge(a, b) = merge(b, a)
//! - Associativity: merge(merge(a, b), c) = merge(a, merge(b, c))
//! - Idempotence: merge(a, a) = a
//! - Monotonicity: values never decrease
//! - Eventual consistency: all replicas converge

#![allow(unused_mut)]
#![allow(special_module_name)]

use crdtosphere::counters::GCounter;
use crdtosphere::prelude::*;
use proptest::prelude::*;

mod lib;
use lib::*;

proptest! {
    #![proptest_config(crdt_config())]

    /// Test that GCounter merge operation is commutative
    /// Property: merge(a, b) = merge(b, a)
    #[test]
    fn gcounter_merge_is_commutative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        increments1 in operation_sequence_strategy(small_increment_strategy()),
        increments2 in operation_sequence_strategy(small_increment_strategy()),
    ) {
        let mut counter1 = GCounter::<DefaultConfig>::new(node1);
        let mut counter2 = GCounter::<DefaultConfig>::new(node2);

        // Apply increments to each counter
        for inc in increments1 {
            let _ = counter1.increment(inc);
        }
        for inc in increments2 {
            let _ = counter2.increment(inc);
        }

        // Test commutativity
        prop_assert!(assert_crdt_commutativity(&counter1, &counter2));
    }

    /// Test that GCounter merge operation is associative
    /// Property: merge(merge(a, b), c) = merge(a, merge(b, c))
    #[test]
    fn gcounter_merge_is_associative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        node3 in node_id_strategy(),
        increments1 in operation_sequence_strategy(small_increment_strategy()),
        increments2 in operation_sequence_strategy(small_increment_strategy()),
        increments3 in operation_sequence_strategy(small_increment_strategy()),
    ) {
        let mut counter1 = GCounter::<DefaultConfig>::new(node1);
        let mut counter2 = GCounter::<DefaultConfig>::new(node2);
        let mut counter3 = GCounter::<DefaultConfig>::new(node3);

        // Apply increments to each counter
        for inc in increments1 {
            let _ = counter1.increment(inc);
        }
        for inc in increments2 {
            let _ = counter2.increment(inc);
        }
        for inc in increments3 {
            let _ = counter3.increment(inc);
        }

        // Test associativity
        prop_assert!(assert_crdt_associativity(&counter1, &counter2, &counter3));
    }

    /// Test that GCounter merge operation is idempotent
    /// Property: merge(a, a) = a
    #[test]
    fn gcounter_merge_is_idempotent(
        node in node_id_strategy(),
        increments in operation_sequence_strategy(small_increment_strategy()),
    ) {
        let mut counter = GCounter::<DefaultConfig>::new(node);

        // Apply increments
        for inc in increments {
            let _ = counter.increment(inc);
        }

        // Test idempotence
        prop_assert!(assert_crdt_idempotence(&counter));
    }

    /// Test that GCounter values are monotonic (never decrease)
    /// Property: after any operation, value >= previous value
    #[test]
    fn gcounter_is_monotonic(
        node in node_id_strategy(),
        increments in operation_sequence_strategy(small_increment_strategy()),
    ) {
        let mut counter = GCounter::<DefaultConfig>::new(node);
        let mut previous_value = counter.value();

        // Apply increments and check monotonicity
        for inc in increments {
            if counter.increment(inc).is_ok() {
                let current_value = counter.value();
                prop_assert!(current_value >= previous_value);
                previous_value = current_value;
            }
        }
    }

    /// Test eventual consistency across multiple replicas
    /// Property: all replicas converge to the same state after merging
    #[test]
    fn gcounter_eventual_consistency(
        nodes in prop::collection::vec(node_id_strategy(), 2..5),
        operations in prop::collection::vec(
            (any::<usize>(), small_increment_strategy()),
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

        let mut replicas: Vec<GCounter<DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| GCounter::<DefaultConfig>::new(node))
            .collect();

        // Apply operations to random replicas
        for (replica_idx, increment) in operations {
            if !replicas.is_empty() {
                let idx = replica_idx % replicas.len();
                let _ = replicas[idx].increment(increment);
            }
        }

        // Test eventual consistency
        prop_assert!(assert_eventual_consistency(&replicas));
    }

    /// Test that GCounter respects memory bounds
    /// Property: memory usage is always within expected bounds
    #[test]
    fn gcounter_respects_memory_bounds(
        node in node_id_strategy(),
        increments in operation_sequence_strategy(small_increment_strategy()),
    ) {
        let mut counter = GCounter::<DefaultConfig>::new(node);

        // Apply increments
        for inc in increments {
            let _ = counter.increment(inc);
        }

        // Test memory bounds (should be well under 1KB)
        prop_assert!(assert_memory_bounds(&counter, 1024));
    }

    /// Test that GCounter operations complete within real-time bounds
    /// Property: all operations complete within expected time
    #[test]
    fn gcounter_respects_realtime_bounds(
        node in node_id_strategy(),
        increments in operation_sequence_strategy(small_increment_strategy()),
    ) {
        let mut counter = GCounter::<DefaultConfig>::new(node);

        // Apply increments
        for inc in increments {
            let _ = counter.increment(inc);
        }

        // Test real-time bounds
        prop_assert!(assert_realtime_bounds(&counter));
    }

    /// Test GCounter overflow protection
    /// Property: operations near overflow should be handled safely
    #[test]
    fn gcounter_overflow_protection(
        node in node_id_strategy(),
        large_increment in (u32::MAX - 1000)..u32::MAX,
    ) {
        let mut counter = GCounter::<DefaultConfig>::new(node);

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
    fn gcounter_empty_behavior(
        nodes in prop::collection::vec(node_id_strategy(), 1..5),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        let counters: Vec<GCounter<DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| GCounter::<DefaultConfig>::new(node))
            .collect();

        // All empty counters should have value 0
        for counter in &counters {
            prop_assert_eq!(counter.value(), 0);
            prop_assert!(counter.is_empty());
            prop_assert_eq!(counter.active_nodes(), 0);
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

    /// Test node value isolation
    /// Property: increments on one node don't affect other nodes directly
    #[test]
    fn gcounter_node_isolation(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        increment1 in small_increment_strategy(),
        increment2 in small_increment_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }
        let mut counter1 = GCounter::<DefaultConfig>::new(node1);
        let mut counter2 = GCounter::<DefaultConfig>::new(node2);

        // Apply increments
        let _ = counter1.increment(increment1);
        let _ = counter2.increment(increment2);

        // Check node isolation before merge
        prop_assert_eq!(counter1.node_value(node1), increment1 as u64);
        prop_assert_eq!(counter1.node_value(node2), 0);
        prop_assert_eq!(counter2.node_value(node1), 0);
        prop_assert_eq!(counter2.node_value(node2), increment2 as u64);

        // After merge, both should have both values
        let _ = counter1.merge(&counter2);
        prop_assert_eq!(counter1.node_value(node1), increment1 as u64);
        prop_assert_eq!(counter1.node_value(node2), increment2 as u64);
        prop_assert_eq!(counter1.value(), (increment1 + increment2) as u64);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_basic_gcounter_properties() {
        let mut counter1 = GCounter::<DefaultConfig>::new(1);
        let mut counter2 = GCounter::<DefaultConfig>::new(2);

        counter1.increment(10).unwrap();
        counter2.increment(5).unwrap();

        // Test basic functionality
        assert_eq!(counter1.value(), 10);
        assert_eq!(counter2.value(), 5);

        // Test merge
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 15);

        // Test properties
        assert!(assert_crdt_idempotence(&counter1));
        assert!(assert_memory_bounds(&counter1, 1024));
        assert!(assert_realtime_bounds(&counter1));
    }
}
