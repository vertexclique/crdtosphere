//! Property-based tests for MVRegister CRDT
//!
//! This module tests the mathematical properties that MVRegister must satisfy:
//! - Commutativity: merge(a, b) = merge(b, a)
//! - Associativity: merge(merge(a, b), c) = merge(a, merge(b, c))
//! - Idempotence: merge(a, a) = a
//! - Multi-value semantics: concurrent writes are preserved
//! - Eventual consistency: all replicas converge

#![allow(unused_mut)]
#![allow(special_module_name)]

use crdtosphere::prelude::*;
use crdtosphere::registers::MVRegister;
use proptest::prelude::*;

mod lib;
use lib::*;

/// Generate test values for the register
fn test_value_strategy() -> impl Strategy<Value = f32> {
    -100.0f32..100.0
}

/// Generate timestamp values
fn timestamp_strategy() -> impl Strategy<Value = u64> {
    1000u64..10000
}

proptest! {
    #![proptest_config(crdt_config())]

    /// Test that MVRegister merge operation is commutative
    /// Property: merge(a, b) = merge(b, a)
    #[test]
    fn mvregister_merge_is_commutative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut register1 = MVRegister::<f32, DefaultConfig>::new(node1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(node2);

        // Set values
        let _ = register1.set(value1, timestamp1);
        let _ = register2.set(value2, timestamp2);

        // Test commutativity
        prop_assert!(assert_crdt_commutativity(&register1, &register2));
    }

    /// Test that MVRegister merge operation is associative
    /// Property: merge(merge(a, b), c) = merge(a, merge(b, c))
    #[test]
    fn mvregister_merge_is_associative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        node3 in node_id_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        value3 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
        timestamp3 in timestamp_strategy(),
    ) {
        // Skip if any nodes are the same
        if node1 == node2 || node1 == node3 || node2 == node3 {
            return Ok(());
        }

        let mut register1 = MVRegister::<f32, DefaultConfig>::new(node1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(node2);
        let mut register3 = MVRegister::<f32, DefaultConfig>::new(node3);

        // Set values
        let _ = register1.set(value1, timestamp1);
        let _ = register2.set(value2, timestamp2);
        let _ = register3.set(value3, timestamp3);

        // Test associativity
        prop_assert!(assert_crdt_associativity(&register1, &register2, &register3));
    }

    /// Test that MVRegister merge operation is idempotent
    /// Property: merge(a, a) = a
    #[test]
    fn mvregister_merge_is_idempotent(
        node in node_id_strategy(),
        value in test_value_strategy(),
        timestamp in timestamp_strategy(),
    ) {
        let mut register = MVRegister::<f32, DefaultConfig>::new(node);

        // Set value
        let _ = register.set(value, timestamp);

        // Test idempotence
        prop_assert!(assert_crdt_idempotence(&register));
    }

    /// Test MVRegister timestamp ordering
    /// Property: newer timestamps should override older ones from the same node
    #[test]
    fn mvregister_timestamp_ordering(
        node in node_id_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        let mut register = MVRegister::<f32, DefaultConfig>::new(node);

        if timestamp1 < timestamp2 {
            // Set older value first, then newer
            let _ = register.set(value1, timestamp1);
            let _ = register.set(value2, timestamp2);

            // Should have the newer value
            prop_assert_eq!(register.get_from_node(node), Some(&value2));
            prop_assert_eq!(register.len(), 1);
        } else if timestamp2 < timestamp1 {
            // Set newer value first, then older
            let _ = register.set(value2, timestamp2);
            let _ = register.set(value1, timestamp1);

            // Should have the newer value
            prop_assert_eq!(register.get_from_node(node), Some(&value1));
            prop_assert_eq!(register.len(), 1);
        } else {
            // Same timestamp - either value is acceptable
            let _ = register.set(value1, timestamp1);
            let _ = register.set(value2, timestamp2);

            prop_assert_eq!(register.len(), 1);
            let stored_value = register.get_from_node(node);
            prop_assert!(stored_value == Some(&value1) || stored_value == Some(&value2));
        }
    }

    /// Test eventual consistency across multiple replicas
    /// Property: all replicas converge to the same state after merging
    #[test]
    fn mvregister_eventual_consistency(
        nodes in prop::collection::vec(node_id_strategy(), 2..4),
        operations in prop::collection::vec(
            (any::<usize>(), test_value_strategy(), timestamp_strategy()),
            0..8
        ),
    ) {
        // Create replicas with unique node IDs
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        if unique_nodes.len() < 2 {
            return Ok(());
        }

        let mut replicas: Vec<MVRegister<f32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| MVRegister::<f32, DefaultConfig>::new(node))
            .collect();

        // Apply operations to random replicas
        for (replica_idx, value, timestamp) in operations {
            if !replicas.is_empty() {
                let idx = replica_idx % replicas.len();
                let _ = replicas[idx].set(value, timestamp);
            }
        }

        // Test eventual consistency
        prop_assert!(assert_eventual_consistency(&replicas));
    }

    /// Test that MVRegister respects memory bounds
    /// Property: memory usage is always within expected bounds
    #[test]
    fn mvregister_respects_memory_bounds(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..5),
    ) {
        let mut register = MVRegister::<f32, DefaultConfig>::new(node);

        // Apply operations
        for (value, timestamp) in operations {
            let _ = register.set(value, timestamp);
        }

        // Test memory bounds
        prop_assert!(assert_memory_bounds(&register, 1024));
    }

    /// Test that MVRegister operations complete within real-time bounds
    /// Property: all operations complete within expected time
    #[test]
    fn mvregister_respects_realtime_bounds(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..5),
    ) {
        let mut register = MVRegister::<f32, DefaultConfig>::new(node);

        // Apply operations
        for (value, timestamp) in operations {
            let _ = register.set(value, timestamp);
        }

        // Test real-time bounds
        prop_assert!(assert_realtime_bounds(&register));
    }

    /// Test that empty registers behave correctly
    /// Property: empty registers have no values and merge correctly
    #[test]
    fn mvregister_empty_behavior(
        nodes in prop::collection::vec(node_id_strategy(), 1..4),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        let registers: Vec<MVRegister<f32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| MVRegister::<f32, DefaultConfig>::new(node))
            .collect();

        // All empty registers should have no values
        for register in &registers {
            prop_assert!(register.is_empty());
            prop_assert_eq!(register.len(), 0);
        }

        // Merging empty registers should result in empty register
        if registers.len() >= 2 {
            let mut merged = registers[0].clone();
            for other in &registers[1..] {
                let _ = merged.merge(other);
            }
            prop_assert!(merged.is_empty());
            prop_assert_eq!(merged.len(), 0);
        }
    }

    /// Test MVRegister multi-value semantics
    /// Property: concurrent values from different nodes are preserved
    #[test]
    fn mvregister_multi_value_semantics(
        nodes in prop::collection::vec(node_id_strategy(), 2..4),
        values in prop::collection::vec(test_value_strategy(), 2..4),
        base_timestamp in timestamp_strategy(),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        if unique_nodes.len() < 2 || values.len() < 2 {
            return Ok(());
        }

        let mut register = MVRegister::<f32, DefaultConfig>::new(unique_nodes[0]);

        // Set values from different nodes at the same timestamp (concurrent)
        for (i, (&node, &value)) in unique_nodes.iter().zip(values.iter()).enumerate() {
            if i == 0 {
                // First node sets directly
                let _ = register.set(value, base_timestamp);
            } else {
                // Other nodes merge in
                let mut other = MVRegister::<f32, DefaultConfig>::new(node);
                let _ = other.set(value, base_timestamp);
                let _ = register.merge(&other);
            }
        }

        // Should have multiple values
        let expected_count = core::cmp::min(unique_nodes.len(), values.len());
        prop_assert_eq!(register.len(), expected_count);

        // All values should be present
        for (&node, &value) in unique_nodes.iter().zip(values.iter()) {
            prop_assert_eq!(register.get_from_node(node), Some(&value));
        }
    }

    /// Test MVRegister capacity limits
    /// Property: MVRegister respects capacity limits and handles overflow gracefully
    #[test]
    fn mvregister_capacity_limits(
        nodes in prop::collection::vec(node_id_strategy(), 0..8), // More than capacity
        values in prop::collection::vec(test_value_strategy(), 0..8),
        base_timestamp in timestamp_strategy(),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        if unique_nodes.is_empty() || values.is_empty() {
            return Ok(());
        }

        let mut register = MVRegister::<f32, DefaultConfig>::new(unique_nodes[0]);
        let mut successful_merges = 1; // First node is already in the register

        // Set initial value
        if !values.is_empty() {
            let _ = register.set(values[0], base_timestamp);
        }

        // Try to merge values from other nodes
        for (i, &node) in unique_nodes.iter().skip(1).enumerate() {
            if i + 1 < values.len() {
                let mut other = MVRegister::<f32, DefaultConfig>::new(node);
                let _ = other.set(values[i + 1], base_timestamp + i as u64 + 1);

                match register.merge(&other) {
                    Ok(_) => successful_merges += 1,
                    Err(_) => break, // Hit capacity limit
                }
            }
        }

        // Should not exceed capacity
        prop_assert!(successful_merges <= 4);
        prop_assert!(register.len() <= 4);

        // All successfully merged values should be present
        for (i, &node) in unique_nodes.iter().take(successful_merges).enumerate() {
            if i < values.len() {
                prop_assert!(register.get_from_node(node).is_some());
            }
        }
    }

    /// Test MVRegister numeric operations (for f32)
    /// Property: numeric operations work correctly with multiple values
    #[test]
    fn mvregister_numeric_operations(
        nodes in prop::collection::vec(node_id_strategy(), 1..4),
        values in prop::collection::vec(1.0f32..100.0, 1..4),
        base_timestamp in timestamp_strategy(),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        if unique_nodes.is_empty() || values.is_empty() {
            return Ok(());
        }

        let mut register = MVRegister::<f32, DefaultConfig>::new(unique_nodes[0]);
        let mut expected_values = vec![];

        // Add values from different nodes
        for (i, (&node, &value)) in unique_nodes.iter().zip(values.iter()).enumerate() {
            if i == 0 {
                let _ = register.set(value, base_timestamp);
                expected_values.push(value);
            } else {
                let mut other = MVRegister::<f32, DefaultConfig>::new(node);
                let _ = other.set(value, base_timestamp + i as u64);
                if register.merge(&other).is_ok() {
                    expected_values.push(value);
                }
            }
        }

        if !expected_values.is_empty() {
            // Test numeric operations
            let expected_avg = expected_values.iter().sum::<f32>() / expected_values.len() as f32;
            let expected_min = expected_values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let expected_max = expected_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

            if let Some(avg) = register.average() {
                prop_assert!((avg - expected_avg).abs() < 0.001); // Float comparison with tolerance
            }

            if let Some(min) = register.min() {
                prop_assert!((min - expected_min).abs() < 0.001);
            }

            if let Some(max) = register.max() {
                prop_assert!((max - expected_max).abs() < 0.001);
            }
        }
    }

    /// Test MVRegister node isolation
    /// Property: operations from different nodes don't interfere incorrectly
    #[test]
    fn mvregister_node_isolation(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut register1 = MVRegister::<f32, DefaultConfig>::new(node1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(node2);

        // Both nodes set values
        let _ = register1.set(value1, timestamp1);
        let _ = register2.set(value2, timestamp2);

        // Both should have their respective values
        prop_assert_eq!(register1.get_from_node(node1), Some(&value1));
        prop_assert_eq!(register2.get_from_node(node2), Some(&value2));

        // Merge should preserve both values
        let _ = register1.merge(&register2);
        prop_assert_eq!(register1.get_from_node(node1), Some(&value1));
        prop_assert_eq!(register1.get_from_node(node2), Some(&value2));
        prop_assert_eq!(register1.len(), 2);
    }

    /// Test MVRegister merge with updates
    /// Property: merging registers with updates preserves latest values
    #[test]
    fn mvregister_merge_with_updates(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        value1_old in test_value_strategy(),
        value1_new in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp_old in timestamp_strategy(),
        timestamp_new in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        // Ensure timestamp_new > timestamp_old
        let (ts_old, ts_new) = if timestamp_old < timestamp_new {
            (timestamp_old, timestamp_new)
        } else {
            (timestamp_new, timestamp_old)
        };

        let mut register1 = MVRegister::<f32, DefaultConfig>::new(node1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(node2);

        // Register1: set old value, then update to new value
        let _ = register1.set(value1_old, ts_old);
        let _ = register1.set(value1_new, ts_new);

        // Register2: set its own value
        let _ = register2.set(value2, timestamp2);

        // Merge
        let _ = register1.merge(&register2);

        // Should have the newer value from node1 and value from node2
        prop_assert_eq!(register1.get_from_node(node1), Some(&value1_new));
        prop_assert_eq!(register1.get_from_node(node2), Some(&value2));
        prop_assert_eq!(register1.len(), 2);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_basic_mvregister_properties() {
        let mut register1 = MVRegister::<f32, DefaultConfig>::new(1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(2);

        register1.set(10.0, 1000).unwrap();
        register2.set(20.0, 1001).unwrap();

        // Test basic functionality
        assert_eq!(register1.get_from_node(1), Some(&10.0));
        assert_eq!(register1.len(), 1);

        // Test merge
        register1.merge(&register2).unwrap();
        assert_eq!(register1.get_from_node(1), Some(&10.0));
        assert_eq!(register1.get_from_node(2), Some(&20.0));
        assert_eq!(register1.len(), 2);

        // Test properties
        assert!(assert_crdt_idempotence(&register1));
        assert!(assert_memory_bounds(&register1, 1024));
        assert!(assert_realtime_bounds(&register1));
    }

    #[test]
    fn test_mvregister_timestamp_ordering() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);

        // Set older value first
        register.set(10.0, 1000).unwrap();
        assert_eq!(register.get_from_node(1), Some(&10.0));

        // Update with newer timestamp
        register.set(20.0, 2000).unwrap();
        assert_eq!(register.get_from_node(1), Some(&20.0));
        assert_eq!(register.len(), 1);

        // Try to update with older timestamp (should be ignored)
        register.set(30.0, 500).unwrap();
        assert_eq!(register.get_from_node(1), Some(&20.0)); // Still 20.0
    }

    #[test]
    fn test_mvregister_multi_values() {
        let mut register1 = MVRegister::<f32, DefaultConfig>::new(1);
        let mut register2 = MVRegister::<f32, DefaultConfig>::new(2);
        let mut register3 = MVRegister::<f32, DefaultConfig>::new(3);

        register1.set(10.0, 1000).unwrap();
        register2.set(20.0, 1001).unwrap();
        register3.set(30.0, 1002).unwrap();

        // Merge all
        register1.merge(&register2).unwrap();
        register1.merge(&register3).unwrap();

        assert_eq!(register1.len(), 3);
        assert_eq!(register1.get_from_node(1), Some(&10.0));
        assert_eq!(register1.get_from_node(2), Some(&20.0));
        assert_eq!(register1.get_from_node(3), Some(&30.0));

        // Test numeric operations
        assert_eq!(register1.average(), Some(20.0)); // (10+20+30)/3
        assert_eq!(register1.min(), Some(10.0));
        assert_eq!(register1.max(), Some(30.0));
    }

    #[test]
    fn test_mvregister_empty_state() {
        let register = MVRegister::<f32, DefaultConfig>::new(1);

        assert!(register.is_empty());
        assert_eq!(register.len(), 0);
        assert_eq!(register.get_from_node(1), None);
        assert_eq!(register.node_id(), 1);

        // Numeric operations on empty register
        assert_eq!(register.average(), None);
        assert_eq!(register.min(), None);
        assert_eq!(register.max(), None);
    }

    #[test]
    fn test_mvregister_capacity() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);

        // Fill to capacity
        for i in 1..=4 {
            let mut other = MVRegister::<f32, DefaultConfig>::new(i);
            other.set(i as f32 * 10.0, 1000 + i as u64).unwrap();
            assert!(register.merge(&other).is_ok());
        }

        assert!(register.is_full());
        assert_eq!(register.len(), 4);

        // Try to add one more (should fail)
        let mut other5 = MVRegister::<f32, DefaultConfig>::new(5);
        other5.set(50.0, 2000).unwrap();
        assert!(register.merge(&other5).is_err());
    }

    #[test]
    fn test_mvregister_values_array() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);
        register.set(42.0, 1000).unwrap();

        let values = register.values_array();
        assert!(values[0].is_some());
        assert_eq!(values[0].unwrap(), 42.0);
        assert!(values[1].is_none());
        assert!(values[2].is_none());
        assert!(values[3].is_none());
    }

    #[test]
    fn test_mvregister_iter() {
        let mut register = MVRegister::<f32, DefaultConfig>::new(1);
        register.set(10.0, 1000).unwrap();

        let mut other = MVRegister::<f32, DefaultConfig>::new(2);
        other.set(20.0, 2000).unwrap();
        register.merge(&other).unwrap();

        let mut count = 0;
        let mut found_10 = false;
        let mut found_20 = false;

        for (value, _, _) in register.iter() {
            count += 1;
            if *value == 10.0 {
                found_10 = true;
            }
            if *value == 20.0 {
                found_20 = true;
            }
        }

        assert_eq!(count, 2);
        assert!(found_10);
        assert!(found_20);
    }
}
