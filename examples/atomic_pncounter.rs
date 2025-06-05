//! Example demonstrating atomic PNCounter usage
//!
//! This example shows how the hardware-atomic feature enables
//! concurrent increment/decrement operations from multiple threads.

use crdtosphere::prelude::*;
use std::sync::Arc;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CRDTosphere Atomic PNCounter Example");
    println!("====================================");

    #[cfg(not(feature = "hardware-atomic"))]
    {
        println!("Standard (non-atomic) implementation:");
        println!("- Requires &mut self for modifications");
        println!("- Single-threaded access only");

        let mut counter = PNCounter::<DefaultConfig>::new(1);
        counter.increment(10)?;
        counter.decrement(3)?;
        println!("Counter value: {}", counter.value());
    }

    #[cfg(feature = "hardware-atomic")]
    {
        println!("Atomic implementation:");
        println!("- Allows &self for modifications");
        println!("- Multi-threaded safe with Arc<T>");
        println!("- Supports concurrent increment/decrement");

        // Create an atomic counter that can be shared between threads
        let counter = Arc::new(PNCounter::<DefaultConfig>::new(1));

        // Clone the Arc for each thread
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let counter_clone = Arc::clone(&counter);
                thread::spawn(move || {
                    // Each thread performs mixed operations
                    for j in 0..10 {
                        if j % 2 == 0 {
                            counter_clone.increment(2).unwrap();
                        } else {
                            counter_clone.decrement(1).unwrap();
                        }
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
        println!("Expected: 40 (4 threads × (5×2 - 5×1) = 4 × 5 = 20)");
        println!("Positive total: {}", counter.total_positive());
        println!("Negative total: {}", counter.total_negative());

        // Demonstrate merge with another counter
        let counter2 = Arc::new(PNCounter::<DefaultConfig>::new(2));
        counter2.increment(15).unwrap();
        counter2.decrement(5).unwrap();

        // Note: merge still requires &mut self even in atomic version
        // This is because merge modifies the structure itself
        let mut counter_clone = counter.as_ref().clone();
        counter_clone.merge(&*counter2).unwrap();

        println!("After merge with counter2 (+15, -5):");
        println!("Merged counter value: {}", counter_clone.value());
        println!(
            "Expected: {} + 10 = {}",
            counter.value(),
            counter.value() + 10
        );
    }

    Ok(())
}
