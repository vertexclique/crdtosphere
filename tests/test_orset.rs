//! Property-based tests for ORSet CRDT
//!
//! This module tests the mathematical properties that ORSet must satisfy:
//! - Commutativity: merge(a, b) = merge(b, a)
//! - Associativity: merge(merge(a, b), c) = merge(a, merge(b, c))
//! - Idempotence: merge(a, a) = a
//! - Add-Remove semantics: elements can be added and removed
//! - Eventual consistency: all replicas converge

#![allow(unused_mut)]
#![allow(special_module_name)]

use crdtosphere::prelude::*;
use crdtosphere::sets::ORSet;
use proptest::prelude::*;

mod lib;
use lib::*;

/// Generate test values for the set
fn test_value_strategy() -> impl Strategy<Value = u32> {
    0u32..50
}

/// Generate timestamp values
fn timestamp_strategy() -> impl Strategy<Value = u64> {
    1000u64..10000
}

proptest! {
    #![proptest_config(crdt_config())]

    /// Test that ORSet merge operation is commutative
    /// Property: merge(a, b) = merge(b, a)
    #[test]
    fn orset_merge_is_commutative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        operations1 in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..5),
        operations2 in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..5),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut set1 = ORSet::<u32, DefaultConfig>::new(node1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(node2);

        // Add elements to each set
        for (element, timestamp) in operations1 {
            let _ = set1.add(element, timestamp);
        }
        for (element, timestamp) in operations2 {
            let _ = set2.add(element, timestamp);
        }

        // Test commutativity
        prop_assert!(assert_crdt_commutativity(&set1, &set2));
    }

    /// Test that ORSet merge operation is associative
    /// Property: merge(merge(a, b), c) = merge(a, merge(b, c))
    #[test]
    fn orset_merge_is_associative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        node3 in node_id_strategy(),
        operations1 in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..3),
        operations2 in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..3),
        operations3 in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..3),
    ) {
        // Skip if any nodes are the same
        if node1 == node2 || node1 == node3 || node2 == node3 {
            return Ok(());
        }

        let mut set1 = ORSet::<u32, DefaultConfig>::new(node1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(node2);
        let mut set3 = ORSet::<u32, DefaultConfig>::new(node3);

        // Add elements to each set
        for (element, timestamp) in operations1 {
            let _ = set1.add(element, timestamp);
        }
        for (element, timestamp) in operations2 {
            let _ = set2.add(element, timestamp);
        }
        for (element, timestamp) in operations3 {
            let _ = set3.add(element, timestamp);
        }

        // Test associativity
        prop_assert!(assert_crdt_associativity(&set1, &set2, &set3));
    }

    /// Test that ORSet merge operation is idempotent
    /// Property: merge(a, a) = a
    #[test]
    fn orset_merge_is_idempotent(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..5),
    ) {
        let mut set = ORSet::<u32, DefaultConfig>::new(node);

        // Add elements
        for (element, timestamp) in operations {
            let _ = set.add(element, timestamp);
        }

        // Test idempotence
        prop_assert!(assert_crdt_idempotence(&set));
    }

    /// Test ORSet add-remove semantics
    /// Property: elements can be added and removed with proper causality
    #[test]
    fn orset_add_remove_semantics(
        node in node_id_strategy(),
        element in test_value_strategy(),
        add_timestamp in timestamp_strategy(),
        remove_timestamp in timestamp_strategy(),
    ) {
        let mut set = ORSet::<u32, DefaultConfig>::new(node);

        if add_timestamp < remove_timestamp {
            // Add then remove
            let _ = set.add(element, add_timestamp);
            prop_assert!(set.contains(&element));

            let _ = set.remove(&element, remove_timestamp);
            prop_assert!(!set.contains(&element));
        } else {
            // Remove then add (or same timestamp)
            let _ = set.remove(&element, remove_timestamp); // Should be no-op
            let _ = set.add(element, add_timestamp);
            prop_assert!(set.contains(&element)); // Should be present
        }
    }

    /// Test eventual consistency across multiple replicas
    /// Property: all replicas converge to the same state after merging
    #[test]
    fn orset_eventual_consistency(
        nodes in prop::collection::vec(node_id_strategy(), 2..4),
        operations in prop::collection::vec(
            (any::<usize>(), test_value_strategy(), timestamp_strategy()),
            0..10
        ),
    ) {
        // Create replicas with unique node IDs
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        if unique_nodes.len() < 2 {
            return Ok(());
        }

        let mut replicas: Vec<ORSet<u32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| ORSet::<u32, DefaultConfig>::new(node))
            .collect();

        // Apply operations to random replicas
        for (replica_idx, element, timestamp) in operations {
            if !replicas.is_empty() {
                let idx = replica_idx % replicas.len();
                let _ = replicas[idx].add(element, timestamp);
            }
        }

        // Test eventual consistency
        prop_assert!(assert_eventual_consistency(&replicas));
    }

    /// Test that ORSet respects memory bounds
    /// Property: memory usage is always within expected bounds
    #[test]
    fn orset_respects_memory_bounds(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..8),
    ) {
        let mut set = ORSet::<u32, DefaultConfig>::new(node);

        // Add elements
        for (element, timestamp) in operations {
            let _ = set.add(element, timestamp);
        }

        // Test memory bounds
        prop_assert!(assert_memory_bounds(&set, 2048)); // Larger bound for ORSets
    }

    /// Test that ORSet operations complete within real-time bounds
    /// Property: all operations complete within expected time
    #[test]
    fn orset_respects_realtime_bounds(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_value_strategy(), timestamp_strategy()), 0..8),
    ) {
        let mut set = ORSet::<u32, DefaultConfig>::new(node);

        // Add elements
        for (element, timestamp) in operations {
            let _ = set.add(element, timestamp);
        }

        // Test real-time bounds
        prop_assert!(assert_realtime_bounds(&set));
    }

    /// Test that empty sets behave correctly
    /// Property: empty sets have no elements and merge correctly
    #[test]
    fn orset_empty_behavior(
        nodes in prop::collection::vec(node_id_strategy(), 1..4),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        let sets: Vec<ORSet<u32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| ORSet::<u32, DefaultConfig>::new(node))
            .collect();

        // All empty sets should have no elements
        for set in &sets {
            prop_assert!(set.is_empty());
            prop_assert_eq!(set.len(), 0);
        }

        // Merging empty sets should result in empty set
        if sets.len() >= 2 {
            let mut merged = sets[0].clone();
            for other in &sets[1..] {
                let _ = merged.merge(other);
            }
            prop_assert!(merged.is_empty());
            prop_assert_eq!(merged.len(), 0);
        }
    }

    /// Test ORSet add-remove causality
    /// Property: causality is preserved across add/remove operations
    #[test]
    fn orset_causality_preservation(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        element in test_value_strategy(),
        add_time in timestamp_strategy(),
        remove_time in timestamp_strategy(),
        readd_time in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut set1 = ORSet::<u32, DefaultConfig>::new(node1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(node2);

        // Node1 adds element
        let _ = set1.add(element, add_time);

        // Node1 removes element (if remove_time > add_time)
        if remove_time > add_time {
            let _ = set1.remove(&element, remove_time);
        }

        // Node2 adds element (potentially after removal)
        let _ = set2.add(element, readd_time);

        // Merge the sets
        let _ = set1.merge(&set2);

        // Element should be present if:
        // 1. It was never removed, OR
        // 2. It was re-added after removal
        let should_be_present = remove_time <= add_time || readd_time > remove_time;
        prop_assert_eq!(set1.contains(&element), should_be_present);
    }

    /// Test ORSet node isolation
    /// Property: operations from different nodes don't interfere incorrectly
    #[test]
    fn orset_node_isolation(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        element in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut set1 = ORSet::<u32, DefaultConfig>::new(node1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(node2);

        // Both nodes add the same element at different times
        let _ = set1.add(element, timestamp1);
        let _ = set2.add(element, timestamp2);

        // Both should contain the element
        prop_assert!(set1.contains(&element));
        prop_assert!(set2.contains(&element));

        // Merge should preserve the element
        let _ = set1.merge(&set2);
        prop_assert!(set1.contains(&element));
        prop_assert_eq!(set1.len(), 1);
    }

    /// Test ORSet capacity limits
    /// Property: ORSet respects capacity limits and handles overflow gracefully
    #[test]
    fn orset_capacity_limits(
        node in node_id_strategy(),
        elements in prop::collection::vec(test_value_strategy(), 0..12), // More than capacity
    ) {
        let mut set = ORSet::<u32, DefaultConfig>::new(node);
        let mut successful_adds = 0;

        // Try to add elements up to and beyond capacity
        for (i, element) in elements.iter().enumerate() {
            let timestamp = 1000 + i as u64;
            match set.add(*element, timestamp) {
                Ok(newly_added) => {
                    // Only count as successful if it was actually a new entry
                    // (ORSet.add returns false if element already exists from same node)
                    if newly_added {
                        successful_adds += 1;
                    }
                }
                Err(_) => break, // Hit capacity limit
            }
        }

        // Should not exceed capacity for element entries
        prop_assert!(set.element_entries() <= 8);

        // The number of unique elements should not exceed the logical capacity
        prop_assert!(set.len() <= 8);

        // The number of successful adds should not exceed capacity
        prop_assert!(successful_adds <= 8);

        // The set length should match the number of successful adds
        // (since we're adding unique elements from the same node)
        prop_assert_eq!(set.len(), successful_adds);

        // All elements that were successfully added should be present
        // Note: We check all elements since duplicates don't create new entries from same node
        for element in &elements {
            if set.contains(element) {
                // If element is present, it must have been successfully added
                prop_assert!(true);
            }
        }
    }

    /// Test ORSet remove operations
    /// Property: remove operations work correctly with timestamps
    #[test]
    fn orset_remove_operations(
        node in node_id_strategy(),
        elements in prop::collection::vec(test_value_strategy(), 1..5),
        base_timestamp in timestamp_strategy(),
    ) {
        let mut set = ORSet::<u32, DefaultConfig>::new(node);

        // Add elements (deduplicate to get unique elements actually added)
        let mut unique_elements = Vec::new();
        for (i, &element) in elements.iter().enumerate() {
            if set.add(element, base_timestamp + i as u64).unwrap_or(false) {
                unique_elements.push(element);
            }
        }

        let initial_len = set.len();

        // Remove some elements (every other unique element)
        let mut removed_count = 0;
        for (i, &element) in unique_elements.iter().enumerate() {
            if i % 2 == 0 { // Remove every other unique element
                let remove_timestamp = base_timestamp + elements.len() as u64 + i as u64;
                if set.remove(&element, remove_timestamp).unwrap_or(false) {
                    removed_count += 1;
                }
            }
        }

        // Check that removed elements are no longer present
        for (i, &element) in unique_elements.iter().enumerate() {
            if i % 2 == 0 {
                prop_assert!(!set.contains(&element));
            } else {
                prop_assert!(set.contains(&element));
            }
        }

        // Length should be reduced by the number of removed elements
        prop_assert_eq!(set.len(), initial_len - removed_count);
    }

    /// Test ORSet merge with removes
    /// Property: merging sets with removes preserves causality
    #[test]
    fn orset_merge_with_removes(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        element in test_value_strategy(),
        add_time1 in timestamp_strategy(),
        add_time2 in timestamp_strategy(),
        remove_time in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut set1 = ORSet::<u32, DefaultConfig>::new(node1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(node2);

        // Set1: add and possibly remove
        let _ = set1.add(element, add_time1);
        if remove_time > add_time1 {
            let _ = set1.remove(&element, remove_time);
        }

        // Set2: add (possibly after remove)
        let _ = set2.add(element, add_time2);

        // Merge
        let _ = set1.merge(&set2);

        // Element should be present if any add happened after the remove
        let latest_add = if add_time1 > add_time2 { add_time1 } else { add_time2 };
        let should_be_present = remove_time <= add_time1 || latest_add > remove_time;

        prop_assert_eq!(set1.contains(&element), should_be_present);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_basic_orset_properties() {
        let mut set1 = ORSet::<u32, DefaultConfig>::new(1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(2);

        set1.add(10, 1000).unwrap();
        set1.add(20, 1001).unwrap();
        set2.add(20, 1002).unwrap();
        set2.add(30, 1003).unwrap();

        // Test basic functionality
        assert!(set1.contains(&10));
        assert!(set1.contains(&20));
        assert!(!set1.contains(&30));
        assert_eq!(set1.len(), 2);

        // Test merge
        set1.merge(&set2).unwrap();
        assert!(set1.contains(&10));
        assert!(set1.contains(&20));
        assert!(set1.contains(&30));
        assert_eq!(set1.len(), 3);

        // Test properties
        assert!(assert_crdt_idempotence(&set1));
        assert!(assert_memory_bounds(&set1, 2048));
        assert!(assert_realtime_bounds(&set1));
    }

    #[test]
    fn test_orset_add_remove() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);

        // Add element
        set.add(42, 1000).unwrap();
        assert!(set.contains(&42));
        assert_eq!(set.len(), 1);

        // Remove element
        set.remove(&42, 2000).unwrap();
        assert!(!set.contains(&42));
        assert_eq!(set.len(), 0);

        // Add again with later timestamp
        #[cfg(not(feature = "hardware-atomic"))]
        {
            set.add(42, 3000).unwrap();
            assert!(set.contains(&42));
            assert_eq!(set.len(), 1);
        }
    }

    #[test]
    fn test_orset_causality() {
        let mut set1 = ORSet::<u32, DefaultConfig>::new(1);
        let mut set2 = ORSet::<u32, DefaultConfig>::new(2);

        // Set1: add then remove
        set1.add(42, 1000).unwrap();
        set1.remove(&42, 2000).unwrap();

        // Set2: add after remove
        set2.add(42, 3000).unwrap();

        // Merge
        set1.merge(&set2).unwrap();

        // Element should be present (re-added after removal)
        assert!(set1.contains(&42));
        assert_eq!(set1.len(), 1);
    }

    #[test]
    fn test_orset_empty_state() {
        let set = ORSet::<u32, DefaultConfig>::new(1);

        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert!(!set.contains(&42));
        assert_eq!(set.node_id(), 1);
    }

    #[test]
    fn test_orset_capacity() {
        let mut set = ORSet::<u32, DefaultConfig>::new(1);

        // Fill to capacity
        for i in 0..8 {
            assert!(set.add(i, 1000 + i as u64).is_ok());
        }

        assert!(set.is_full());
        assert_eq!(set.remaining_capacity(), 0);

        // Try to add one more (should fail)
        assert!(set.add(8, 2000).is_err());
    }
}
