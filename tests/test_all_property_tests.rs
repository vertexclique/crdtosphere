//! Comprehensive property-based test runner for all CRDTs
//!
//! This module runs property tests for all CRDT implementations to verify
//! they satisfy the mathematical properties required for CRDTs:
//! - Commutativity: merge(a, b) = merge(b, a)
//! - Associativity: merge(merge(a, b), c) = merge(a, merge(b, c))
//! - Idempotence: merge(a, a) = a
//! - Eventual consistency: all replicas converge
//! - Monotonicity: values never decrease (for appropriate CRDTs)

#![allow(unused_mut)]
#![allow(special_module_name)]

use crdtosphere::counters::{GCounter, PNCounter};
use crdtosphere::maps::LWWMap;
use crdtosphere::prelude::*;
use crdtosphere::registers::{LWWRegister, MVRegister};
use crdtosphere::sets::{GSet, ORSet};
use proptest::prelude::*;

mod lib;
use lib::*;

proptest! {
    #![proptest_config(integration_config())]

    /// Integration test: verify all CRDTs work together correctly
    /// Property: different CRDT types can coexist and maintain their properties
    #[test]
    fn integration_all_crdts_work_together(
        node1 in node_id_strategy(),
        node2 in node_id_strategy(),
        counter_ops in operation_sequence_strategy(small_increment_strategy()),
        set_elements in prop::collection::vec(0u32..50, 0..8), // Smaller for GSet capacity
        register_values in prop::collection::vec(0u32..100, 0..5),
        map_entries in prop::collection::vec((0u8..10, 0u32..100), 0..5),
    ) {
        // Skip if nodes are the same
        if node1 == node2 {
            return Ok(());
        }

        // Create different CRDT types
        let mut gcounter1 = GCounter::<DefaultConfig>::new(node1);
        let mut gcounter2 = GCounter::<DefaultConfig>::new(node2);

        let mut pncounter1 = PNCounter::<DefaultConfig>::new(node1);
        let mut pncounter2 = PNCounter::<DefaultConfig>::new(node2);

        let mut lww_register1 = LWWRegister::<u32, DefaultConfig>::new(node1);
        let mut lww_register2 = LWWRegister::<u32, DefaultConfig>::new(node2);

        let mut mv_register1 = MVRegister::<f32, DefaultConfig>::new(node1);
        let mut mv_register2 = MVRegister::<f32, DefaultConfig>::new(node2);

        let mut gset1 = GSet::<u32, DefaultConfig>::new();
        let mut gset2 = GSet::<u32, DefaultConfig>::new();

        let mut orset1 = ORSet::<u32, DefaultConfig>::new(node1);
        let mut orset2 = ORSet::<u32, DefaultConfig>::new(node2);

        let mut lww_map1 = LWWMap::<u8, u32, DefaultConfig>::new(node1);
        let mut lww_map2 = LWWMap::<u8, u32, DefaultConfig>::new(node2);

        // Apply operations to node1 CRDTs
        for &op in &counter_ops {
            let _ = gcounter1.increment(op);
            let _ = pncounter1.increment(op);
        }

        for (i, &value) in register_values.iter().enumerate() {
            let _ = lww_register1.set(value, 1000 + i as u64);
            let _ = mv_register1.set(value as f32, 1000 + i as u64);
        }

        for &element in &set_elements {
            let _ = gset1.insert(element);
            let _ = orset1.add(element, 1000);
        }

        for (i, &(key, value)) in map_entries.iter().enumerate() {
            let _ = lww_map1.insert(key, value, 1000 + i as u64);
        }

        // Apply different operations to node2 CRDTs
        for &op in &counter_ops {
            let _ = gcounter2.increment(op + 1);
            let _ = pncounter2.decrement(op);
        }

        for (i, &value) in register_values.iter().enumerate() {
            let _ = lww_register2.set(value + 100, 2000 + i as u64);
            let _ = mv_register2.set((value + 100) as f32, 2000 + i as u64);
        }

        for &element in &set_elements {
            let _ = gset2.insert(element + 100);
            let _ = orset2.add(element + 100, 2000);
        }

        for (i, &(key, value)) in map_entries.iter().enumerate() {
            let _ = lww_map2.insert(key, value + 100, 2000 + i as u64);
        }

        // Test that each CRDT type maintains its properties
        prop_assert!(assert_crdt_commutativity(&gcounter1, &gcounter2));
        prop_assert!(assert_crdt_commutativity(&pncounter1, &pncounter2));
        prop_assert!(assert_crdt_commutativity(&lww_register1, &lww_register2));
        prop_assert!(assert_crdt_commutativity(&mv_register1, &mv_register2));
        prop_assert!(assert_crdt_commutativity(&gset1, &gset2));
        prop_assert!(assert_crdt_commutativity(&orset1, &orset2));
        prop_assert!(assert_crdt_commutativity(&lww_map1, &lww_map2));

        // Test that all CRDTs respect memory bounds
        prop_assert!(assert_memory_bounds(&gcounter1, 1024));
        prop_assert!(assert_memory_bounds(&pncounter1, 1024));
        prop_assert!(assert_memory_bounds(&lww_register1, 1024));
        prop_assert!(assert_memory_bounds(&mv_register1, 1024));
        prop_assert!(assert_memory_bounds(&gset1, 2048));
        prop_assert!(assert_memory_bounds(&orset1, 2048));
        prop_assert!(assert_memory_bounds(&lww_map1, 1024));

        // Test that all CRDTs respect real-time bounds
        prop_assert!(assert_realtime_bounds(&gcounter1));
        prop_assert!(assert_realtime_bounds(&pncounter1));
        prop_assert!(assert_realtime_bounds(&lww_register1));
        prop_assert!(assert_realtime_bounds(&mv_register1));
        prop_assert!(assert_realtime_bounds(&gset1));
        prop_assert!(assert_realtime_bounds(&orset1));
        prop_assert!(assert_realtime_bounds(&lww_map1));
    }

    /// Test CRDT property preservation across different configurations
    /// Property: CRDT properties hold regardless of operation order
    #[test]
    fn crdt_properties_preserved_across_configurations(
        nodes in prop::collection::vec(node_id_strategy(), 2..4),
        operations in prop::collection::vec(small_increment_strategy(), 1..10),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        if unique_nodes.len() < 2 {
            return Ok(());
        }

        // Create multiple replicas of each CRDT type
        let mut gcounters: Vec<GCounter<DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| GCounter::<DefaultConfig>::new(node))
            .collect();

        let mut pncounters: Vec<PNCounter<DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| PNCounter::<DefaultConfig>::new(node))
            .collect();

        let mut lww_registers: Vec<LWWRegister<u32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| LWWRegister::<u32, DefaultConfig>::new(node))
            .collect();

        let mut mv_registers: Vec<MVRegister<f32, DefaultConfig>> = unique_nodes
            .iter()
            .map(|&node| MVRegister::<f32, DefaultConfig>::new(node))
            .collect();

        // Apply operations to each replica
        for (i, &op) in operations.iter().enumerate() {
            let replica_idx = i % gcounters.len();
            let _ = gcounters[replica_idx].increment(op);
            let _ = pncounters[replica_idx].increment(op);
            let _ = lww_registers[replica_idx].set(op, 1000 + i as u64);
            let _ = mv_registers[replica_idx].set(op as f32, 1000 + i as u64);
        }

        // Test eventual consistency for all CRDT types
        prop_assert!(assert_eventual_consistency(&gcounters));
        prop_assert!(assert_eventual_consistency(&pncounters));
        prop_assert!(assert_eventual_consistency(&lww_registers));
        prop_assert!(assert_eventual_consistency(&mv_registers));

        // Test that all replicas are valid
        for counter in &gcounters {
            prop_assert!(counter.validate().is_ok());
        }
        for counter in &pncounters {
            prop_assert!(counter.validate().is_ok());
        }
        for register in &lww_registers {
            prop_assert!(register.validate().is_ok());
        }
        for register in &mv_registers {
            prop_assert!(register.validate().is_ok());
        }
    }

    /// Test CRDT behavior under stress conditions
    /// Property: CRDTs maintain correctness even with many operations
    #[test]
    fn crdt_stress_test(
        node in node_id_strategy(),
        large_operations in prop::collection::vec(small_increment_strategy(), 10..50),
    ) {
        let mut gcounter = GCounter::<DefaultConfig>::new(node);
        let mut pncounter = PNCounter::<DefaultConfig>::new(node);
        let mut lww_register = LWWRegister::<u32, DefaultConfig>::new(node);
        let mut mv_register = MVRegister::<f32, DefaultConfig>::new(node);
        let mut gset = GSet::<u32, DefaultConfig>::new();
        let mut orset = ORSet::<u32, DefaultConfig>::new(node);
        let mut lww_map = LWWMap::<u8, u32, DefaultConfig>::new(node);

        let mut expected_gcounter_value = 0u64;
        let mut expected_pncounter_value = 0i64;
        let mut expected_set_size = 0usize;
        let mut last_register_value = None;

        // Apply many operations
        for (i, &op) in large_operations.iter().enumerate() {
            // GCounter operations
            if gcounter.increment(op).is_ok() {
                expected_gcounter_value += op as u64;
            }

            // PNCounter operations (alternate increment/decrement)
            if i % 2 == 0 {
                if pncounter.increment(op).is_ok() {
                    expected_pncounter_value += op as i64;
                }
            } else if pncounter.decrement(op).is_ok() {
                expected_pncounter_value -= op as i64;
            }

            // Register operations
            if lww_register.set(op, 1000 + i as u64).is_ok() {
                last_register_value = Some(op);
            }
            let _ = mv_register.set(op as f32, 1000 + i as u64);

            // Set operations (limited by capacity)
            if gset.insert(i as u32).is_ok() {
                expected_set_size += 1;
            }
            let _ = orset.add(i as u32, 1000 + i as u64);

            // Map operations
            let _ = lww_map.insert((i % 8) as u8, op, 1000 + i as u64);
        }

        // Verify final states
        prop_assert_eq!(gcounter.value(), expected_gcounter_value);
        prop_assert_eq!(pncounter.value(), expected_pncounter_value);
        prop_assert_eq!(gset.len(), expected_set_size);

        if let Some(expected_value) = last_register_value {
            prop_assert_eq!(lww_register.get(), Some(&expected_value));
        }

        // Verify all CRDTs are still valid
        prop_assert!(gcounter.validate().is_ok());
        prop_assert!(pncounter.validate().is_ok());
        prop_assert!(lww_register.validate().is_ok());
        prop_assert!(mv_register.validate().is_ok());
        prop_assert!(gset.validate().is_ok());
        prop_assert!(orset.validate().is_ok());
        prop_assert!(lww_map.validate().is_ok());
    }

    /// Test cross-CRDT consistency
    /// Property: different CRDT types maintain consistency when used together
    #[test]
    fn cross_crdt_consistency(
        nodes in prop::collection::vec(node_id_strategy(), 2..3),
        operations in prop::collection::vec(small_increment_strategy(), 1..8),
    ) {
        let mut unique_nodes = nodes;
        unique_nodes.sort();
        unique_nodes.dedup();

        if unique_nodes.len() < 2 {
            return Ok(());
        }

        // Create pairs of different CRDT types
        let mut counters_and_registers: Vec<(GCounter<DefaultConfig>, LWWRegister<u32, DefaultConfig>)> =
            unique_nodes.iter().map(|&node| {
                (GCounter::<DefaultConfig>::new(node), LWWRegister::<u32, DefaultConfig>::new(node))
            }).collect();

        type SetMapPair = (GSet<u32, DefaultConfig>, LWWMap<u8, u32, DefaultConfig>);
        let mut sets_and_maps: Vec<SetMapPair> =
            unique_nodes.iter().map(|&node| {
                (GSet::<u32, DefaultConfig>::new(), LWWMap::<u8, u32, DefaultConfig>::new(node))
            }).collect();

        // Apply coordinated operations
        for (i, &op) in operations.iter().enumerate() {
            let replica_idx = i % counters_and_registers.len();

            // Coordinate counter and register operations
            let _ = counters_and_registers[replica_idx].0.increment(op);
            let _ = counters_and_registers[replica_idx].1.set(op, 1000 + i as u64);

            // Coordinate set and map operations
            let _ = sets_and_maps[replica_idx].0.insert(op);
            let _ = sets_and_maps[replica_idx].1.insert((op % 8) as u8, op, 1000 + i as u64);
        }

        // Extract individual CRDT collections for testing
        let counters: Vec<_> = counters_and_registers.iter().map(|(c, _)| c.clone()).collect();
        let registers: Vec<_> = counters_and_registers.iter().map(|(_, r)| r.clone()).collect();
        let sets: Vec<_> = sets_and_maps.iter().map(|(s, _)| s.clone()).collect();
        let maps: Vec<_> = sets_and_maps.iter().map(|(_, m)| m.clone()).collect();

        // Test eventual consistency for each type
        prop_assert!(assert_eventual_consistency(&counters));
        prop_assert!(assert_eventual_consistency(&registers));
        prop_assert!(assert_eventual_consistency(&sets));
        prop_assert!(assert_eventual_consistency(&maps));
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_all_crdt_types_basic_functionality() {
        // Test GCounter
        let mut gcounter = GCounter::<DefaultConfig>::new(1);
        gcounter.increment(10).unwrap();
        assert_eq!(gcounter.value(), 10);
        assert!(gcounter.validate().is_ok());

        // Test PNCounter
        let mut pncounter = PNCounter::<DefaultConfig>::new(1);
        pncounter.increment(10).unwrap();
        pncounter.decrement(3).unwrap();
        assert_eq!(pncounter.value(), 7);
        assert!(pncounter.validate().is_ok());

        // Test LWWRegister
        let mut lww_register = LWWRegister::<u32, DefaultConfig>::new(1);
        lww_register.set(42, 1000).unwrap();
        assert_eq!(lww_register.get(), Some(&42));
        assert!(lww_register.validate().is_ok());

        // Test MVRegister
        let mut mv_register = MVRegister::<f32, DefaultConfig>::new(1);
        mv_register.set(std::f32::consts::PI, 1000).unwrap();
        assert_eq!(mv_register.get_from_node(1), Some(&std::f32::consts::PI));
        assert!(mv_register.validate().is_ok());

        // Test GSet
        let mut gset = GSet::<u32, DefaultConfig>::new();
        gset.insert(42).unwrap();
        gset.insert(43).unwrap();
        assert_eq!(gset.len(), 2);
        assert!(gset.contains(&42));
        assert!(gset.contains(&43));
        assert!(gset.validate().is_ok());

        // Test ORSet
        let mut orset = ORSet::<u32, DefaultConfig>::new(1);
        orset.add(42, 1000).unwrap();
        orset.add(43, 1001).unwrap();
        assert!(orset.contains(&42));
        assert!(orset.contains(&43));
        assert!(orset.validate().is_ok());

        // Test LWWMap
        let mut lww_map = LWWMap::<u8, u32, DefaultConfig>::new(1);
        lww_map.insert(1, 100, 1000).unwrap();
        lww_map.insert(2, 200, 1001).unwrap();
        assert_eq!(lww_map.get(&1), Some(&100));
        assert_eq!(lww_map.get(&2), Some(&200));
        assert!(lww_map.validate().is_ok());
    }

    #[test]
    fn test_crdt_memory_bounds() {
        let gcounter = GCounter::<DefaultConfig>::new(1);
        let pncounter = PNCounter::<DefaultConfig>::new(1);
        let lww_register = LWWRegister::<u32, DefaultConfig>::new(1);
        let mv_register = MVRegister::<f32, DefaultConfig>::new(1);
        let gset = GSet::<u32, DefaultConfig>::new();
        let orset = ORSet::<u32, DefaultConfig>::new(1);
        let lww_map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        assert!(assert_memory_bounds(&gcounter, 1024));
        assert!(assert_memory_bounds(&pncounter, 1024));
        assert!(assert_memory_bounds(&lww_register, 1024));
        assert!(assert_memory_bounds(&mv_register, 1024));
        assert!(assert_memory_bounds(&gset, 2048));
        assert!(assert_memory_bounds(&orset, 2048));
        assert!(assert_memory_bounds(&lww_map, 1024));
    }

    #[test]
    fn test_crdt_real_time_bounds() {
        let gcounter = GCounter::<DefaultConfig>::new(1);
        let pncounter = PNCounter::<DefaultConfig>::new(1);
        let lww_register = LWWRegister::<u32, DefaultConfig>::new(1);
        let mv_register = MVRegister::<f32, DefaultConfig>::new(1);
        let gset = GSet::<u32, DefaultConfig>::new();
        let orset = ORSet::<u32, DefaultConfig>::new(1);
        let lww_map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        assert!(assert_realtime_bounds(&gcounter));
        assert!(assert_realtime_bounds(&pncounter));
        assert!(assert_realtime_bounds(&lww_register));
        assert!(assert_realtime_bounds(&mv_register));
        assert!(assert_realtime_bounds(&gset));
        assert!(assert_realtime_bounds(&orset));
        assert!(assert_realtime_bounds(&lww_map));
    }

    #[test]
    fn test_crdt_idempotence_all_types() {
        let gcounter = GCounter::<DefaultConfig>::new(1);
        let pncounter = PNCounter::<DefaultConfig>::new(1);
        let lww_register = LWWRegister::<u32, DefaultConfig>::new(1);
        let mv_register = MVRegister::<f32, DefaultConfig>::new(1);
        let gset = GSet::<u32, DefaultConfig>::new();
        let orset = ORSet::<u32, DefaultConfig>::new(1);
        let lww_map = LWWMap::<u8, u32, DefaultConfig>::new(1);

        assert!(assert_crdt_idempotence(&gcounter));
        assert!(assert_crdt_idempotence(&pncounter));
        assert!(assert_crdt_idempotence(&lww_register));
        assert!(assert_crdt_idempotence(&mv_register));
        assert!(assert_crdt_idempotence(&gset));
        assert!(assert_crdt_idempotence(&orset));
        assert!(assert_crdt_idempotence(&lww_map));
    }
}
