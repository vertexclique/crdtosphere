#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

//! Device Capability Registry Example
//!
//! Track device capabilities using a grow-only set.

use crdtosphere::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Capability {
    WiFi,
    Bluetooth,
    GPS,
    Camera,
    Accelerometer,
}

fn main() -> Result<(), CRDTError> {
    // Device capability sets
    let mut mobile_device = GSet::<Capability, DefaultConfig>::new();
    let mut iot_device = GSet::<Capability, DefaultConfig>::new();

    // Mobile device capabilities
    mobile_device.insert(Capability::WiFi)?;
    mobile_device.insert(Capability::Bluetooth)?;
    mobile_device.insert(Capability::GPS)?;
    mobile_device.insert(Capability::Camera)?;

    // IoT device capabilities
    iot_device.insert(Capability::WiFi)?;
    iot_device.insert(Capability::Accelerometer)?;

    // Check capabilities
    println!(
        "Mobile has GPS: {}",
        mobile_device.contains(&Capability::GPS)
    );
    println!("IoT has GPS: {}", iot_device.contains(&Capability::GPS));

    // Merge to get all capabilities in network
    mobile_device.merge(&iot_device)?;
    println!("Total capabilities: {}", mobile_device.len());

    Ok(())
}
