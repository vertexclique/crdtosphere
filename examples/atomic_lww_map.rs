//! Atomic Last-Writer-Wins Map Example
//!
//! This example demonstrates the atomic LWWMap CRDT for concurrent configuration management.
//! The atomic version allows thread-safe operations without external synchronization.

use crdtosphere::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuration keys for a distributed system
#[derive(Debug, Clone, Copy, PartialEq)]
enum ConfigKey {
    MaxConnections = 1,
    TimeoutMs = 2,
    RetryCount = 3,
    BufferSize = 4,
    LogLevel = 5,
}

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn main() -> Result<(), CRDTError> {
    println!("CRDTosphere Atomic LWWMap Example");
    println!("======================================");

    // Create a shared configuration map
    let config_map = Arc::new(LWWMap::<ConfigKey, u32, DefaultConfig>::new(1));

    println!("\n📝 Initial Configuration Setup");
    println!("------------------------------");

    // Set initial configuration values (atomic version takes &self, not &mut self)
    config_map.insert(ConfigKey::MaxConnections, 100, get_timestamp())?;
    config_map.insert(ConfigKey::TimeoutMs, 5000, get_timestamp())?;
    config_map.insert(ConfigKey::RetryCount, 3, get_timestamp())?;
    config_map.insert(ConfigKey::BufferSize, 8192, get_timestamp())?;

    println!("Initial config:");
    for key in [
        ConfigKey::MaxConnections,
        ConfigKey::TimeoutMs,
        ConfigKey::RetryCount,
        ConfigKey::BufferSize,
    ] {
        if let Some(value) = config_map.get(&key) {
            println!("  {:?}: {}", key, value);
        }
    }

    println!("\n🔄 Concurrent Configuration Updates");
    println!("----------------------------------");

    // Spawn multiple threads to update configuration concurrently
    let mut handles = vec![];

    // Thread 1: Performance tuning
    let config_clone1 = Arc::clone(&config_map);
    handles.push(thread::spawn(move || {
        println!("🚀 Performance thread: Increasing connection limits");

        // Simulate performance optimization
        thread::sleep(std::time::Duration::from_millis(10));
        config_clone1
            .insert(ConfigKey::MaxConnections, 200, get_timestamp())
            .unwrap();

        thread::sleep(std::time::Duration::from_millis(20));
        config_clone1
            .insert(ConfigKey::BufferSize, 16384, get_timestamp())
            .unwrap();

        println!("   ✅ Performance updates applied");
    }));

    // Thread 2: Reliability tuning
    let config_clone2 = Arc::clone(&config_map);
    handles.push(thread::spawn(move || {
        println!("🛡️  Reliability thread: Adjusting timeout and retries");

        // Simulate reliability improvements
        thread::sleep(std::time::Duration::from_millis(15));
        config_clone2
            .insert(ConfigKey::TimeoutMs, 10000, get_timestamp())
            .unwrap();

        thread::sleep(std::time::Duration::from_millis(25));
        config_clone2
            .insert(ConfigKey::RetryCount, 5, get_timestamp())
            .unwrap();

        println!("   ✅ Reliability updates applied");
    }));

    // Thread 3: Monitoring configuration
    let config_clone3 = Arc::clone(&config_map);
    handles.push(thread::spawn(move || {
        println!("📊 Monitoring thread: Adding logging configuration");

        // Simulate monitoring setup
        thread::sleep(std::time::Duration::from_millis(30));
        config_clone3
            .insert(ConfigKey::LogLevel, 2, get_timestamp())
            .unwrap(); // Info level

        println!("   ✅ Monitoring configuration added");
    }));

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    println!("\n📋 Final Configuration State");
    println!("----------------------------");

    // Display final configuration
    println!("Final config after concurrent updates:");
    for key in [
        ConfigKey::MaxConnections,
        ConfigKey::TimeoutMs,
        ConfigKey::RetryCount,
        ConfigKey::BufferSize,
        ConfigKey::LogLevel,
    ] {
        if let Some(value) = config_map.get(&key) {
            println!("  {:?}: {}", key, value);
        }
    }

    println!("\n📊 Map Statistics");
    println!("-----------------");
    println!("Total entries: {}", config_map.len());
    println!("Capacity: {}", config_map.capacity());
    println!("Remaining capacity: {}", config_map.remaining_capacity());
    println!("Is empty: {}", config_map.is_empty());
    println!("Is full: {}", config_map.is_full());

    println!("\n🔍 Configuration Merging Example");
    println!("--------------------------------");

    // Create another configuration map from a different node
    let remote_config = LWWMap::<ConfigKey, u32, DefaultConfig>::new(2);

    // Add some remote configuration
    remote_config.insert(ConfigKey::MaxConnections, 150, get_timestamp() + 1000)?; // Newer timestamp
    remote_config.insert(ConfigKey::TimeoutMs, 7500, get_timestamp() - 1000)?; // Older timestamp

    println!("Remote config before merge:");
    for key in [ConfigKey::MaxConnections, ConfigKey::TimeoutMs] {
        if let Some(value) = remote_config.get(&key) {
            println!("  {:?}: {}", key, value);
        }
    }

    // Create a mutable clone for merging (atomic maps need special handling for merge)
    let mut local_config = (*config_map).clone();

    // Merge remote configuration
    local_config.merge(&remote_config)?;

    println!("\nAfter merging remote config:");
    for key in [
        ConfigKey::MaxConnections,
        ConfigKey::TimeoutMs,
        ConfigKey::RetryCount,
        ConfigKey::BufferSize,
        ConfigKey::LogLevel,
    ] {
        if let Some(value) = local_config.get(&key) {
            println!("  {:?}: {}", key, value);
        }
    }

    println!("\n🗑️  Remove Operation Example");
    println!("---------------------------");

    // Demonstrate remove functionality
    println!(
        "Before removal - LogLevel: {:?}",
        local_config.get(&ConfigKey::LogLevel)
    );
    println!("Map length: {}", local_config.len());

    // Remove a configuration entry
    let removed_value = local_config.remove(&ConfigKey::LogLevel);
    println!("Removed LogLevel: {:?}", removed_value);
    println!(
        "After removal - LogLevel: {:?}",
        local_config.get(&ConfigKey::LogLevel)
    );
    println!("Map length: {}", local_config.len());
    println!("Remaining capacity: {}", local_config.remaining_capacity());

    // Demonstrate that removed entries free up capacity
    println!("\nAdding new configuration after removal:");
    local_config.insert(ConfigKey::LogLevel, 1, get_timestamp())?; // Debug level
    println!(
        "Re-added LogLevel: {:?}",
        local_config.get(&ConfigKey::LogLevel)
    );
    println!("Map length: {}", local_config.len());

    println!("\n✨ Atomic LWWMap Features Demonstrated:");
    println!("  • Thread-safe concurrent updates without locks");
    println!("  • Last-writer-wins conflict resolution");
    println!("  • Timestamp-based ordering");
    println!("  • Node ID tiebreaking for same timestamps");
    println!("  • Deterministic merge operations");
    println!("  • Fixed memory allocation");
    println!("  • Remove operations that free capacity");
    println!("  • Remove and re-insert functionality");

    Ok(())
}
