#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

//! Configuration Management Example
//!
//! Manage distributed configuration with last-writer-wins semantics.

use crdtosphere::prelude::*;

fn main() -> Result<(), CRDTError> {
    // Configuration registers on different services
    let mut service1_config = LWWRegister::<&str, DefaultConfig>::new(1);
    let mut service2_config = LWWRegister::<&str, DefaultConfig>::new(2);

    // Service 1 sets initial configuration
    service1_config.set("debug_mode=false", 1000)?;

    // Service 2 updates configuration later
    service2_config.set("debug_mode=true", 2000)?;

    // Merge configurations - latest timestamp wins
    service1_config.merge(&service2_config)?;

    if let Some(config) = service1_config.get() {
        println!("Current config: {}", config);
    }

    Ok(())
}
