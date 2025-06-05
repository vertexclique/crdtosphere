//! Property-based tests for LWWRegister CRDT
//!
//! This module tests the mathematical properties that LWWRegister must satisfy:
//! - Commutativity: merge(a, b) = merge(b, a)
//! - Associativity: merge(merge(a, b), c) = merge(a, merge(b, c))
//! - Idempotence: merge(a, a) = a
//! - Last-Writer-Wins semantics: later timestamps win
//! - Eventual consistency: all replicas converge

#![allow(unused_mut)]
#![allow(special_module_name)]

use crdtosphere::prelude::*;
use crdtosphere::registers::LWWRegister;
use proptest::prelude::*;

mod lib;
use lib::*;

/// Generate test values for the register
fn test_value_strategy() -> impl Strategy<Value = u32> {
    0u32..1000
}

/// Generate timestamps for testing
fn timestamp_strategy() -> impl Strategy<Value = u64> {
    0u64..1000000
}

proptest! {
    #![proptest_config(crdt_config())]

    /// Test that LWWRegister merge operation is commutative
    /// Property: merge(a, b) = merge(b, a)
    #[test]
    fn lww_register_merge_is_commutative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        let mut register1 = LWWRegister::<u32, DefaultConfig>::new(node1);
        let mut register2 = LWWRegister::<u32, DefaultConfig>::new(node2);

        // Set values with specific timestamps
        let _ = register1.set(value1, timestamp1);
        let _ = register2.set(value2, timestamp2);

        // Test commutativity
        prop_assert!(assert_crdt_commutativity(&register1, &register2));
    }

    /// Test that LWWRegister merge operation is associative
    /// Property: merge(merge(a, b), c) = merge(a, merge(b, c))
    #[test]
    fn lww_register_merge_is_associative(
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
        let mut register1 = LWWRegister::<u32, DefaultConfig>::new(node1);
        let mut register2 = LWWRegister::<u32, DefaultConfig>::new(node2);
        let mut register3 = LWWRegister::<u32, DefaultConfig>::new(node3);

        // Set values with specific timestamps
        let _ = register1.set(value1, timestamp1);
        let _ = register2.set(value2, timestamp2);
        let _ = register3.set(value3, timestamp3);

        // Test associativity
        prop_assert!(assert_crdt_associativity(&register1, &register2, &register3));
    }

    /// Test that LWWRegister merge operation is idempotent
    /// Property: merge(a, a) = a
    #[test]
    fn lww_register_merge_is_idempotent(
        node in node_id_strategy(),
        value in test_value_strategy(),
        timestamp in timestamp_strategy(),
    ) {
        let mut register = LWWRegister::<u32, DefaultConfig>::new(node);
        let _ = register.set(value, timestamp);

        // Test idempotence
        prop_assert!(assert_crdt_idempotence(&register));
    }

    /// Test last-writer-wins semantics
    /// Property: later timestamps should always win
    #[test]
    fn lww_register_last_writer_wins(
        node in node_id_strategy(),
        early_value in test_value_strategy(),
        late_value in test_value_strategy(),
        early_timestamp in 1u64..500000,
        late_timestamp in 500001u64..1000000,
    ) {
        let mut register1 = LWWRegister::<u32, DefaultConfig>::new(node);
        let mut register2 = LWWRegister::<u32, DefaultConfig>::new(node);

        // Set early value in register1
        let _ = register1.set(early_value, early_timestamp);

        // Set late value in register2
        let _ = register2.set(late_value, late_timestamp);

        // Merge register2 into register1
        let _ = register1.merge(&register2);

        // Should have the later value
        prop_assert_eq!(register1.get(), Some(&late_value));
        prop_assert!(register1.timestamp().as_u64() == late_timestamp);
    }

    /// Test eventual consistency across multiple replicas
    /// Property: all replicas converge to the same state after merging
    #[test]
    fn lww_register_eventual_consistency(
        nodes in prop::collection::vec(node_id_strategy(), 2..5),
        operations in prop::collection::vec(
            (any::<usize>(), test_value_strategy(), timestamp_strategy()),
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

        let mut replicas: Vec<LWWRegister<u32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| LWWRegister::<u32, DefaultConfig>::new(node))
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

    /// Test that LWWRegister respects memory bounds
    /// Property: memory usage is always within expected bounds
    #[test]
    fn lww_register_respects_memory_bounds(
        node in node_id_strategy(),
        operations in prop::collection::vec(
            (test_value_strategy(), timestamp_strategy()),
            0..20
        ),
    ) {
        let mut register = LWWRegister::<u32, DefaultConfig>::new(node);

        // Apply operations
        for (value, timestamp) in operations {
            let _ = register.set(value, timestamp);
        }

        // Test memory bounds
        prop_assert!(assert_memory_bounds(&register, 1024));
    }

    /// Test that LWWRegister operations complete within real-time bounds
    /// Property: all operations complete within expected time
    #[test]
    fn lww_register_respects_realtime_bounds(
        node in node_id_strategy(),
        operations in prop::collection::vec(
            (test_value_strategy(), timestamp_strategy()),
            0..20
        ),
    ) {
        let mut register = LWWRegister::<u32, DefaultConfig>::new(node);

        // Apply operations
        for (value, timestamp) in operations {
            let _ = register.set(value, timestamp);
        }

        // Test real-time bounds
        prop_assert!(assert_realtime_bounds(&register));
    }

    /// Test that empty registers behave correctly
    /// Property: empty registers have no value and merge correctly
    #[test]
    fn lww_register_empty_behavior(
        nodes in prop::collection::vec(node_id_strategy(), 1..5),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        let registers: Vec<LWWRegister<u32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| LWWRegister::<u32, DefaultConfig>::new(node))
            .collect();

        // All empty registers should have no value
        for register in &registers {
            prop_assert_eq!(register.get(), None);
            prop_assert!(register.is_empty());
            prop_assert_eq!(register.timestamp().as_u64(), 0);
        }

        // Merging empty registers should result in empty register
        if registers.len() >= 2 {
            let mut merged = registers[0].clone();
            for other in &registers[1..] {
                let _ = merged.merge(other);
            }
            prop_assert_eq!(merged.get(), None);
            prop_assert!(merged.is_empty());
        }
    }

    /// Test timestamp ordering with concurrent updates
    /// Property: higher timestamps should always win regardless of merge order
    #[test]
    fn lww_register_timestamp_ordering(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        base_timestamp in 1u64..100000,
        timestamp_offset in 1u64..1000,
    ) {
        let mut register1 = LWWRegister::<u32, DefaultConfig>::new(node1);
        let mut register2 = LWWRegister::<u32, DefaultConfig>::new(node2);

        let early_timestamp = base_timestamp;
        let late_timestamp = base_timestamp + timestamp_offset;

        // Set values with different timestamps
        let _ = register1.set(value1, early_timestamp);
        let _ = register2.set(value2, late_timestamp);

        // Test merge in both directions
        let mut merged1 = register1.clone();
        let mut merged2 = register2.clone();

        let _ = merged1.merge(&register2);
        let _ = merged2.merge(&register1);

        // Both should have the later value
        prop_assert_eq!(merged1.get(), Some(&value2));
        prop_assert_eq!(merged2.get(), Some(&value2));
        prop_assert_eq!(merged1.timestamp().as_u64(), late_timestamp);
        prop_assert_eq!(merged2.timestamp().as_u64(), late_timestamp);
    }

    /// Test node ID tie-breaking for same timestamps
    /// Property: when timestamps are equal, higher node ID should win
    #[test]
    fn lww_register_node_id_tiebreaking(
        node1 in 0u8..8,
        node2 in 8u8..16,  // Ensure node2 > node1
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp in timestamp_strategy(),
    ) {
        let mut register1 = LWWRegister::<u32, DefaultConfig>::new(node1);
        let mut register2 = LWWRegister::<u32, DefaultConfig>::new(node2);

        // Set same timestamp but different values
        let _ = register1.set(value1, timestamp);
        let _ = register2.set(value2, timestamp);

        // Merge register2 into register1
        let _ = register1.merge(&register2);

        // Should have value from higher node ID (node2)
        prop_assert_eq!(register1.get(), Some(&value2));
        prop_assert_eq!(register1.timestamp().as_u64(), timestamp);
    }

    /// Test value updates and retrieval
    /// Property: the register should always return the most recent value
    #[test]
    fn lww_register_value_updates(
        node in node_id_strategy(),
        initial_value in test_value_strategy(),
        updates in prop::collection::vec(test_value_strategy(), 1..10),
    ) {
        let mut register = LWWRegister::<u32, DefaultConfig>::new(node);
        let mut current_timestamp = 1000u64;

        // Set initial value
        let _ = register.set(initial_value, current_timestamp);
        prop_assert_eq!(register.get(), Some(&initial_value));
        prop_assert!(!register.is_empty());

        // Apply updates with increasing timestamps
        for update_value in updates {
            current_timestamp += 1;
            let _ = register.set(update_value, current_timestamp);
            prop_assert_eq!(register.get(), Some(&update_value));
        }
    }

    /// Test register with different value types
    /// Property: LWWRegister should work with different data types
    #[test]
    fn lww_register_different_types(
        node in node_id_strategy(),
        string_values in prop::collection::vec("[a-z]{1,10}", 1..5),
        timestamps in prop::collection::vec(timestamp_strategy(), 1..5),
    ) {
        let mut register = LWWRegister::<String, DefaultConfig>::new(node);

        // Apply string values with timestamps
        for (value, timestamp) in string_values.iter().zip(timestamps.iter()) {
            let _ = register.set(value.clone(), *timestamp);
        }

        // Should have some value if any operations succeeded
        if !string_values.is_empty() {
            prop_assert!(register.get().is_some());
            prop_assert!(!register.is_empty());
        }

        // Test memory bounds
        prop_assert!(assert_memory_bounds(&register, 2048)); // Larger bound for strings
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_basic_lww_register_properties() {
        let mut register1 = LWWRegister::<u32, DefaultConfig>::new(1);
        let mut register2 = LWWRegister::<u32, DefaultConfig>::new(2);

        register1.set(100, 10).unwrap();
        register2.set(200, 20).unwrap();

        // Test basic functionality
        assert_eq!(register1.get(), Some(&100));
        assert_eq!(register2.get(), Some(&200));

        // Test merge (later timestamp wins)
        register1.merge(&register2).unwrap();
        assert_eq!(register1.get(), Some(&200));
        assert_eq!(register1.timestamp().as_u64(), 20);

        // Test properties
        assert!(assert_crdt_idempotence(&register1));
        assert!(assert_memory_bounds(&register1, 1024));
        assert!(assert_realtime_bounds(&register1));
    }

    #[test]
    fn test_lww_register_last_writer_wins() {
        let mut register = LWWRegister::<u32, DefaultConfig>::new(1);

        // Set value with early timestamp
        register.set(100, 10).unwrap();
        assert_eq!(register.get(), Some(&100));

        // Set value with later timestamp
        register.set(200, 20).unwrap();
        assert_eq!(register.get(), Some(&200));

        // Try to set value with earlier timestamp (should be ignored)
        register.set(300, 5).unwrap();
        assert_eq!(register.get(), Some(&200)); // Should still be 200
        assert_eq!(register.timestamp().as_u64(), 20);
    }

    #[test]
    fn test_lww_register_node_id_tiebreaking() {
        let mut register1 = LWWRegister::<u32, DefaultConfig>::new(1);
        let mut register2 = LWWRegister::<u32, DefaultConfig>::new(2);

        // Set same timestamp, different values
        register1.set(100, 10).unwrap();
        register2.set(200, 10).unwrap();

        // Merge - higher node ID should win
        register1.merge(&register2).unwrap();
        assert_eq!(register1.get(), Some(&200));
        assert_eq!(register1.timestamp().as_u64(), 10);
    }

    #[test]
    fn test_lww_register_empty_state() {
        let register = LWWRegister::<u32, DefaultConfig>::new(1);

        assert_eq!(register.get(), None);
        assert!(register.is_empty());
        assert_eq!(register.timestamp().as_u64(), 0);
    }
}
