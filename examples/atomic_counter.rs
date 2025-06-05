//! Example demonstrating atomic GCounter usage
//!
//! This example shows how the hardware-atomic feature enables
//! concurrent access to CRDTs from multiple threads.

use crdtosphere::prelude::*;
use std::sync::Arc;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CRDTosphere Atomic Counter Example");
    println!("==================================");

    #[cfg(not(feature = "hardware-atomic"))]
    {
        println!("Standard (non-atomic) implementation:");
        println!("- Requires &mut self for modifications");
        println!("- Single-threaded access only");

        let mut counter = GCounter::<DefaultConfig>::new(1);
        counter.increment(5)?;
        println!("Counter value: {}", counter.value());
    }

    #[cfg(feature = "hardware-atomic")]
    {
        println!("Atomic implementation:");
        println!("- Allows &self for modifications");
        println!("- Multi-threaded safe with Arc<T>");

        // Create an atomic counter that can be shared between threads
        let counter = Arc::new(GCounter::<DefaultConfig>::new(1));

        // Clone the Arc for each thread
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let counter_clone = Arc::clone(&counter);
                thread::spawn(move || {
                    // Each thread can increment concurrently
                    for _ in 0..10 {
                        counter_clone.increment(1).unwrap();
                    }
                    println!("Thread {} completed", i);
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        println!("Final counter value: {}", counter.value());
        println!("Expected: 40 (4 threads × 10 increments)");
    }

    Ok(())
}
