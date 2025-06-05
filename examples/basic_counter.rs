#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

//! Distributed Event Counter Example
//!
//! Track events across multiple nodes with a grow-only counter.

use crdtosphere::prelude::*;

fn main() -> Result<(), CRDTError> {
    // Create counters on different nodes
    let mut node1_counter = GCounter::<DefaultConfig>::new(1);
    let mut node2_counter = GCounter::<DefaultConfig>::new(2);

    // Each node tracks local events
    node1_counter.increment(1)?; // Page view
    node1_counter.increment(1)?; // Button click

    node2_counter.increment(1)?; // API call
    node2_counter.increment(1)?; // User action
    node2_counter.increment(1)?; // Background task

    println!("Node 1 events: {}", node1_counter.value());
    println!("Node 2 events: {}", node2_counter.value());

    // Merge counters to get total events
    node1_counter.merge(&node2_counter)?;
    println!("Total events: {}", node1_counter.value());

    Ok(())
}
