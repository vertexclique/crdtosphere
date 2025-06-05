//! Property-based tests for LWWMap CRDT
//!
//! This module tests the mathematical properties that LWWMap must satisfy:
//! - Commutativity: merge(a, b) = merge(b, a)
//! - Associativity: merge(merge(a, b), c) = merge(a, merge(b, c))
//! - Idempotence: merge(a, a) = a
//! - Last-Writer-Wins semantics: later timestamps win per key
//! - Eventual consistency: all replicas converge

#![allow(unused_mut)]
#![allow(special_module_name)]

use crdtosphere::maps::LWWMap;
use crdtosphere::prelude::*;
use proptest::prelude::*;

mod lib;
use lib::*;

/// Generate test keys for the map
fn test_key_strategy() -> impl Strategy<Value = u8> {
    0u8..20
}

/// Generate test values for the map
fn test_value_strategy() -> impl Strategy<Value = u32> {
    0u32..1000
}

/// Generate timestamp values
fn timestamp_strategy() -> impl Strategy<Value = u64> {
    1000u64..10000
}

proptest! {
    #![proptest_config(crdt_config())]

    /// Test that LWWMap merge operation is commutative
    /// Property: merge(a, b) = merge(b, a)
    #[test]
    fn lwwmap_merge_is_commutative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        operations1 in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..5),
        operations2 in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..5),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(node1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(node2);

        // Apply operations to each map
        for (key, value, timestamp) in operations1 {
            let _ = map1.insert(key, value, timestamp);
        }
        for (key, value, timestamp) in operations2 {
            let _ = map2.insert(key, value, timestamp);
        }

        // Test commutativity
        prop_assert!(assert_crdt_commutativity(&map1, &map2));
    }

    /// Test that LWWMap merge operation is associative
    /// Property: merge(merge(a, b), c) = merge(a, merge(b, c))
    #[test]
    fn lwwmap_merge_is_associative(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        node3 in node_id_strategy(),
        operations1 in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..3),
        operations2 in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..3),
        operations3 in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..3),
    ) {
        // Skip if any nodes are the same
        if node1 == node2 || node1 == node3 || node2 == node3 {
            return Ok(());
        }

        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(node1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(node2);
        let mut map3 = LWWMap::<u8, u32, DefaultConfig>::new(node3);

        // Apply operations to each map
        for (key, value, timestamp) in operations1 {
            let _ = map1.insert(key, value, timestamp);
        }
        for (key, value, timestamp) in operations2 {
            let _ = map2.insert(key, value, timestamp);
        }
        for (key, value, timestamp) in operations3 {
            let _ = map3.insert(key, value, timestamp);
        }

        // Test associativity
        prop_assert!(assert_crdt_associativity(&map1, &map2, &map3));
    }

    /// Test that LWWMap merge operation is idempotent
    /// Property: merge(a, a) = a
    #[test]
    fn lwwmap_merge_is_idempotent(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..5),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);

        // Apply operations
        for (key, value, timestamp) in operations {
            let _ = map.insert(key, value, timestamp);
        }

        // Test idempotence
        prop_assert!(assert_crdt_idempotence(&map));
    }

    /// Test LWWMap last-writer-wins semantics
    /// Property: newer timestamps should override older ones
    #[test]
    fn lwwmap_last_writer_wins(
        node in node_id_strategy(),
        key in test_key_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);

        if timestamp1 < timestamp2 {
            // Set older value first, then newer
            let _ = map.insert(key, value1, timestamp1);
            let _ = map.insert(key, value2, timestamp2);

            // Should have the newer value
            prop_assert_eq!(map.get(&key), Some(&value2));
            prop_assert_eq!(map.len(), 1);
        } else if timestamp2 < timestamp1 {
            // Set newer value first, then older
            let _ = map.insert(key, value2, timestamp2);
            let _ = map.insert(key, value1, timestamp1);

            // Should have the newer value
            prop_assert_eq!(map.get(&key), Some(&value1));
            prop_assert_eq!(map.len(), 1);
        } else {
            // Same timestamp - node ID tiebreaker
            let _ = map.insert(key, value1, timestamp1);
            let _ = map.insert(key, value2, timestamp2);

            prop_assert_eq!(map.len(), 1);
            // With same timestamp and same node, last write wins
            prop_assert_eq!(map.get(&key), Some(&value2));
        }
    }

    /// Test eventual consistency across multiple replicas
    /// Property: all replicas converge to the same state after merging
    #[test]
    fn lwwmap_eventual_consistency(
        nodes in prop::collection::vec(node_id_strategy(), 2..4),
        operations in prop::collection::vec(
            (any::<usize>(), test_key_strategy(), test_value_strategy(), timestamp_strategy()),
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

        let mut replicas: Vec<LWWMap<u8, u32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| LWWMap::<u8, u32, DefaultConfig>::new(node))
            .collect();

        // Apply operations to random replicas
        for (replica_idx, key, value, timestamp) in operations {
            if !replicas.is_empty() {
                let idx = replica_idx % replicas.len();
                let _ = replicas[idx].insert(key, value, timestamp);
            }
        }

        // Test eventual consistency
        prop_assert!(assert_eventual_consistency(&replicas));
    }

    /// Test that LWWMap respects memory bounds
    /// Property: memory usage is always within expected bounds
    #[test]
    fn lwwmap_respects_memory_bounds(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..8),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);

        // Apply operations
        for (key, value, timestamp) in operations {
            let _ = map.insert(key, value, timestamp);
        }

        // Test memory bounds
        prop_assert!(assert_memory_bounds(&map, 1024));
    }

    /// Test that LWWMap operations complete within real-time bounds
    /// Property: all operations complete within expected time
    #[test]
    fn lwwmap_respects_realtime_bounds(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..8),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);

        // Apply operations
        for (key, value, timestamp) in operations {
            let _ = map.insert(key, value, timestamp);
        }

        // Test real-time bounds
        prop_assert!(assert_realtime_bounds(&map));
    }

    /// Test that empty maps behave correctly
    /// Property: empty maps have no entries and merge correctly
    #[test]
    fn lwwmap_empty_behavior(
        nodes in prop::collection::vec(node_id_strategy(), 1..4),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        let maps: Vec<LWWMap<u8, u32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| LWWMap::<u8, u32, DefaultConfig>::new(node))
            .collect();

        // All empty maps should have no entries
        for map in &maps {
            prop_assert!(map.is_empty());
            prop_assert_eq!(map.len(), 0);
        }

        // Merging empty maps should result in empty map
        if maps.len() >= 2 {
            let mut merged = maps[0].clone();
            for other in &maps[1..] {
                let _ = merged.merge(other);
            }
            prop_assert!(merged.is_empty());
            prop_assert_eq!(merged.len(), 0);
        }
    }

    /// Test LWWMap node tiebreaker semantics
    /// Property: with same timestamps, higher node ID wins
    #[test]
    fn lwwmap_node_tiebreaker(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        key in test_key_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(node1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(node2);

        // Both nodes set the same key with the same timestamp
        let _ = map1.insert(key, value1, timestamp);
        let _ = map2.insert(key, value2, timestamp);

        // Merge map2 into map1
        let _ = map1.merge(&map2);

        // Higher node ID should win
        let expected_value = if node1 > node2 { &value1 } else { &value2 };
        prop_assert_eq!(map1.get(&key), Some(expected_value));
        prop_assert_eq!(map1.len(), 1);
    }

    /// Test LWWMap capacity limits
    /// Property: LWWMap respects capacity limits and handles overflow gracefully
    #[test]
    fn lwwmap_capacity_limits(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 0..12), // More than capacity
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);
        let mut successful_inserts = 0;
        let mut unique_keys = std::collections::HashSet::new();

        // Try to insert operations up to and beyond capacity
        for (key, value, timestamp) in operations {
            match map.insert(key, value, timestamp) {
                Ok(is_new) => {
                    if is_new {
                        unique_keys.insert(key);
                        successful_inserts += 1;
                    }
                }
                Err(_) => break, // Hit capacity limit
            }
        }

        // Should not exceed capacity
        prop_assert!(successful_inserts <= 8);
        prop_assert!(map.len() <= 8);

        // All successfully inserted keys should be present
        for &key in &unique_keys {
            prop_assert!(map.contains_key(&key));
        }
    }

    /// Test LWWMap merge with conflicts
    /// Property: merging maps with conflicting keys preserves LWW semantics
    #[test]
    fn lwwmap_merge_with_conflicts(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        key in test_key_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(node1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(node2);

        // Both maps set the same key with different values and timestamps
        let _ = map1.insert(key, value1, timestamp1);
        let _ = map2.insert(key, value2, timestamp2);

        // Merge map2 into map1
        let _ = map1.merge(&map2);

        // The value with the newer timestamp should win
        let expected_value = if timestamp1 > timestamp2 {
            &value1
        } else if timestamp2 > timestamp1 {
            &value2
        } else {
            // Same timestamp - higher node ID wins
            if node1 > node2 { &value1 } else { &value2 }
        };

        prop_assert_eq!(map1.get(&key), Some(expected_value));
        prop_assert_eq!(map1.len(), 1);
    }

    /// Test LWWMap key operations
    /// Property: key operations work correctly
    #[test]
    fn lwwmap_key_operations(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 1..8),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);
        let mut expected_keys = std::collections::HashSet::new();

        // Apply operations
        for (key, value, timestamp) in operations {
            if map.insert(key, value, timestamp).is_ok() {
                expected_keys.insert(key);
            }
        }

        // Check that all expected keys are present
        for &key in &expected_keys {
            prop_assert!(map.contains_key(&key));
            prop_assert!(map.get(&key).is_some());
            prop_assert!(map.get_timestamp(&key).is_some());
            prop_assert_eq!(map.get_node_id(&key), Some(node));
        }

        // Check that map length matches unique keys
        prop_assert_eq!(map.len(), expected_keys.len());
    }

    /// Test LWWMap iterators
    /// Property: iterators return all entries correctly
    #[test]
    fn lwwmap_iterators(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 1..8),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);
        let mut expected_entries = std::collections::HashMap::new();

        // Apply operations (later operations may override earlier ones)
        for (key, value, timestamp) in operations {
            if map.insert(key, value, timestamp).is_ok() {
                expected_entries.insert(key, value);
            }
        }

        // Test key-value iterator
        let mut found_entries = std::collections::HashMap::new();
        for (&k, &v) in map.iter() {
            found_entries.insert(k, v);
        }
        prop_assert_eq!(found_entries.len(), expected_entries.len());

        // Test keys iterator
        let mut found_keys = std::collections::HashSet::new();
        for &k in map.keys() {
            found_keys.insert(k);
        }
        prop_assert_eq!(found_keys.len(), expected_entries.len());

        // Test values iterator
        let found_values_count = map.values().count();
        prop_assert_eq!(found_values_count, expected_entries.len());
    }

    /// Test LWWMap node isolation
    /// Property: operations from different nodes don't interfere incorrectly
    #[test]
    fn lwwmap_node_isolation(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        key1 in test_key_strategy(),
        key2 in test_key_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(node1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(node2);

        // Each node sets different keys
        let _ = map1.insert(key1, value1, timestamp1);
        let _ = map2.insert(key2, value2, timestamp2);

        // Merge should preserve both entries (if keys are different)
        let _ = map1.merge(&map2);

        if key1 != key2 {
            prop_assert_eq!(map1.get(&key1), Some(&value1));
            prop_assert_eq!(map1.get(&key2), Some(&value2));
            prop_assert_eq!(map1.len(), 2);
        } else {
            // Same key - LWW semantics apply
            prop_assert_eq!(map1.len(), 1);
            prop_assert!(map1.contains_key(&key1));
        }
    }

    /// Test LWWMap update semantics
    /// Property: updates to existing keys work correctly
    #[test]
    fn lwwmap_update_semantics(
        node in node_id_strategy(),
        key in test_key_strategy(),
        values in prop::collection::vec(test_value_strategy(), 2..5),
        base_timestamp in timestamp_strategy(),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);

        // Apply updates with increasing timestamps
        for (i, &value) in values.iter().enumerate() {
            let timestamp = base_timestamp + i as u64;
            let is_new = map.insert(key, value, timestamp).unwrap_or(false);

            if i == 0 {
                prop_assert!(is_new); // First insert should be new
            } else {
                prop_assert!(!is_new); // Subsequent inserts should be updates
            }
        }

        // Should have only one entry with the last value
        prop_assert_eq!(map.len(), 1);
        prop_assert_eq!(map.get(&key), values.last());

        // Timestamp should be the latest
        if let Some(timestamp) = map.get_timestamp(&key) {
            let expected_timestamp = base_timestamp + (values.len() - 1) as u64;
            prop_assert_eq!(timestamp.as_u64(), expected_timestamp);
        }
    }

    /// Test LWWMap remove operation
    /// Property: remove operations work correctly and free capacity
    #[test]
    fn lwwmap_remove_basic(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 1..8),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);
        let mut expected_keys = std::collections::HashSet::new();

        // Insert operations
        for (key, value, timestamp) in operations {
            if map.insert(key, value, timestamp).is_ok() {
                expected_keys.insert(key);
            }
        }

        let initial_len = map.len();
        let initial_capacity = map.remaining_capacity();

        // Remove a key that exists
        if let Some(&key_to_remove) = expected_keys.iter().next() {
            let removed_value = map.remove(&key_to_remove);
            prop_assert!(removed_value.is_some());
            prop_assert!(!map.contains_key(&key_to_remove));
            prop_assert_eq!(map.len(), initial_len - 1);
            prop_assert_eq!(map.remaining_capacity(), initial_capacity + 1);
        }

        // Remove a key that doesn't exist
        let non_existent_key = 99u8; // Assuming this key wasn't in our test range
        let removed_value = map.remove(&non_existent_key);
        prop_assert_eq!(removed_value, None);
    }

    /// Test LWWMap remove and reinsert
    /// Property: removing and reinserting keys works correctly
    #[test]
    fn lwwmap_remove_and_reinsert(
        node in node_id_strategy(),
        key in test_key_strategy(),
        value1 in test_value_strategy(),
        value2 in test_value_strategy(),
        timestamp1 in timestamp_strategy(),
        timestamp2 in timestamp_strategy(),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);

        // Insert initial value
        let _ = map.insert(key, value1, timestamp1);
        prop_assert!(map.contains_key(&key));
        prop_assert_eq!(map.len(), 1);

        // Remove the key
        let removed = map.remove(&key);
        prop_assert_eq!(removed, Some(value1));
        prop_assert!(!map.contains_key(&key));
        prop_assert_eq!(map.len(), 0);

        // Reinsert with different value
        let _ = map.insert(key, value2, timestamp2);
        prop_assert!(map.contains_key(&key));
        prop_assert_eq!(map.get(&key), Some(&value2));
        prop_assert_eq!(map.len(), 1);
    }

    /// Test LWWMap remove capacity management
    /// Property: removing entries frees up capacity for new insertions
    #[test]
    fn lwwmap_remove_capacity_management(
        node in node_id_strategy(),
        keys in prop::collection::vec(test_key_strategy(), 8..12), // More than capacity
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);
        let mut inserted_keys = std::collections::HashSet::new();

        // Fill to capacity with unique keys
        for (i, &key) in keys.iter().enumerate() {
            if inserted_keys.len() >= 8 {
                break; // Stop when we reach capacity
            }
            if map.insert(key, key as u32 * 10, 1000 + i as u64).is_ok() {
                inserted_keys.insert(key);
            }
        }

        // Only proceed if we actually filled the map
        if inserted_keys.len() < 8 {
            return Ok(()); // Skip test if we don't have enough unique keys
        }

        prop_assert!(map.is_full());
        prop_assert_eq!(map.remaining_capacity(), 0);

        // Try to insert beyond capacity (should fail)
        if keys.len() > 8 {
            let overflow_key = keys[8];
            if !inserted_keys.contains(&overflow_key) {
                let result = map.insert(overflow_key, 999, 2000);
                prop_assert!(result.is_err());
            }
        }

        // Remove some entries
        let keys_to_remove = inserted_keys.len() / 2;
        let keys_vec: std::collections::BTreeSet<_> = inserted_keys.iter().cloned().collect();
        for &key in keys_vec.iter().take(keys_to_remove) {
            map.remove(&key);
        }

        prop_assert!(!map.is_full());
        prop_assert_eq!(map.remaining_capacity(), keys_to_remove);

        // Should now be able to insert new entries
        if keys.len() > 8 {
            for (i, &key) in keys.iter().skip(8).take(keys_to_remove).enumerate() {
                if !inserted_keys.contains(&key) {
                    let result = map.insert(key, key as u32 * 100, 3000 + i as u64);
                    prop_assert!(result.is_ok());
                }
            }
        }
    }

    /// Test LWWMap remove all entries
    /// Property: removing all entries results in empty map
    #[test]
    fn lwwmap_remove_all_entries(
        node in node_id_strategy(),
        operations in prop::collection::vec((test_key_strategy(), test_value_strategy(), timestamp_strategy()), 1..8),
    ) {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(node);
        let mut inserted_keys = Vec::new();

        // Insert operations
        for (key, value, timestamp) in operations {
            if map.insert(key, value, timestamp).is_ok() && !inserted_keys.contains(&key) {
                inserted_keys.push(key);
            }
        }

        let initial_len = map.len();
        prop_assert!(initial_len > 0);

        // Remove all entries
        for &key in &inserted_keys {
            map.remove(&key);
        }

        // Map should be empty
        prop_assert!(map.is_empty());
        prop_assert_eq!(map.len(), 0);
        prop_assert_eq!(map.remaining_capacity(), 8);

        // All keys should be gone
        for &key in &inserted_keys {
            prop_assert!(!map.contains_key(&key));
            prop_assert_eq!(map.get(&key), None);
        }
    }

    /// Test LWWMap remove order independence
    /// Property: order of removal doesn't affect final state
    #[test]
    fn lwwmap_remove_order_independence(
        node in node_id_strategy(),
        keys in prop::collection::vec(test_key_strategy(), 3..6),
        values in prop::collection::vec(test_value_strategy(), 3..6),
        base_timestamp in timestamp_strategy(),
    ) {
        if keys.len() != values.len() || keys.len() < 3 {
            return Ok(());
        }

        // Create two identical maps
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(node);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(node);

        // Insert same data in both maps
        for (i, (&key, &value)) in keys.iter().zip(values.iter()).enumerate() {
            let timestamp = base_timestamp + i as u64;
            let _ = map1.insert(key, value, timestamp);
            let _ = map2.insert(key, value, timestamp);
        }

        // Remove keys in different orders
        if keys.len() >= 2 {
            // Map1: remove first, then second
            map1.remove(&keys[0]);
            map1.remove(&keys[1]);

            // Map2: remove second, then first
            map2.remove(&keys[1]);
            map2.remove(&keys[0]);

            // Both maps should have the same final state
            prop_assert_eq!(map1.len(), map2.len());
            for &key in &keys[2..] {
                prop_assert_eq!(map1.get(&key), map2.get(&key));
            }
            prop_assert!(!map1.contains_key(&keys[0]));
            prop_assert!(!map1.contains_key(&keys[1]));
            prop_assert!(!map2.contains_key(&keys[0]));
            prop_assert!(!map2.contains_key(&keys[1]));
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_basic_lwwmap_properties() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

        map1.insert(1, 10, 1000).unwrap();
        map1.insert(2, 20, 1001).unwrap();
        map2.insert(2, 25, 2000).unwrap(); // Newer value for key 2
        map2.insert(3, 30, 2001).unwrap();

        // Test basic functionality
        assert_eq!(map1.get(&1), Some(&10));
        assert_eq!(map1.get(&2), Some(&20));
        assert_eq!(map1.len(), 2);

        // Test merge
        map1.merge(&map2).unwrap();
        assert_eq!(map1.get(&1), Some(&10)); // Unchanged
        assert_eq!(map1.get(&2), Some(&25)); // Updated to newer value
        assert_eq!(map1.get(&3), Some(&30)); // New entry
        assert_eq!(map1.len(), 3);

        // Test properties
        assert!(assert_crdt_idempotence(&map1));
        assert!(assert_memory_bounds(&map1, 1024));
        assert!(assert_realtime_bounds(&map1));
    }

    #[test]
    fn test_lwwmap_last_writer_wins() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Insert initial value
        map.insert(1, 10, 1000).unwrap();
        assert_eq!(map.get(&1), Some(&10));

        // Update with newer timestamp
        map.insert(1, 20, 2000).unwrap();
        assert_eq!(map.get(&1), Some(&20));
        assert_eq!(map.len(), 1);

        // Try to update with older timestamp (should be ignored)
        map.insert(1, 30, 500).unwrap();
        assert_eq!(map.get(&1), Some(&20)); // Still 20
    }

    #[test]
    fn test_lwwmap_node_tiebreaker() {
        let mut map1 = LWWMap::<u8, u32, DefaultConfig>::new(1);
        let mut map2 = LWWMap::<u8, u32, DefaultConfig>::new(2);

        // Both set same key with same timestamp
        map1.insert(1, 10, 1000).unwrap();
        map2.insert(1, 20, 1000).unwrap();

        // Merge - higher node ID should win
        map1.merge(&map2).unwrap();
        assert_eq!(map1.get(&1), Some(&20)); // Node 2 > Node 1
        assert_eq!(map1.len(), 1);
    }

    #[test]
    fn test_lwwmap_empty_state() {
        let map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert_eq!(map.get(&1), None);
        assert!(!map.contains_key(&1));
        assert_eq!(map.node_id(), 1);
    }

    #[test]
    fn test_lwwmap_capacity() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        // Fill to capacity
        for i in 0..8 {
            assert!(map.insert(i, i as u32 * 10, 1000 + i as u64).is_ok());
        }

        assert!(map.is_full());
        assert_eq!(map.remaining_capacity(), 0);

        // Try to add one more (should fail)
        assert!(map.insert(8, 80, 2000).is_err());
    }

    #[test]
    fn test_lwwmap_iterators() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        map.insert(1, 10, 1000).unwrap();
        map.insert(3, 30, 1001).unwrap();
        map.insert(2, 20, 1002).unwrap();

        // Test key-value iterator
        let mut pairs = [(0u8, 0u32); 3];
        for (i, (&k, &v)) in map.iter().enumerate() {
            pairs[i] = (k, v);
        }
        pairs.sort_by_key(|&(k, _)| k);
        assert_eq!(pairs, [(1, 10), (2, 20), (3, 30)]);

        // Test keys iterator
        let mut keys = [0u8; 3];
        for (i, &k) in map.keys().enumerate() {
            keys[i] = k;
        }
        keys.sort();
        assert_eq!(keys, [1, 2, 3]);

        // Test values iterator
        let mut values = [0u32; 3];
        for (i, &v) in map.values().enumerate() {
            values[i] = v;
        }
        values.sort();
        assert_eq!(values, [10, 20, 30]);
    }

    #[test]
    fn test_lwwmap_metadata() {
        let mut map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        map.insert(1, 10, 1000).unwrap();

        assert_eq!(map.get_timestamp(&1).unwrap().as_u64(), 1000);
        assert_eq!(map.get_node_id(&1), Some(1));
        assert_eq!(map.get_timestamp(&2), None);
        assert_eq!(map.get_node_id(&2), None);
    }
}
