#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

//! IoT Sensor Network Example
//!
//! Collect and aggregate data from distributed IoT sensors using the IoT domain types.

use crdtosphere::iot::{ReadingQuality, SensorNetwork, SensorType};
use crdtosphere::prelude::*;

fn main() -> Result<(), CRDTError> {
    // Create sensor networks for different gateways
    let mut gateway1 = SensorNetwork::<DefaultConfig>::new(1);
    let mut gateway2 = SensorNetwork::<DefaultConfig>::new(2);

    // Gateway 1 sensor readings
    gateway1.add_reading(
        42, // sensor ID
        SensorType::Temperature,
        2550, // 25.5°C (scaled by 100)
        ReadingQuality::Good,
        1000, // timestamp
        1,    // location ID
    )?;

    gateway1.add_reading(
        43,
        SensorType::Humidity,
        6000, // 60% (scaled by 100)
        ReadingQuality::Excellent,
        1001,
        1,
    )?;

    // Gateway 2 sensor readings (some overlap)
    gateway2.add_reading(
        44,
        SensorType::Temperature,
        2610, // 26.1°C (newer reading)
        ReadingQuality::Good,
        1002,
        1,
    )?;

    gateway2.add_reading_with_vitals(
        45,
        SensorType::Pressure,
        101325, // 1013.25 hPa (scaled by 100)
        ReadingQuality::Excellent,
        1003,
        1,
        85,  // battery level
        220, // signal strength
    )?;

    // Merge sensor data from both gateways
    gateway1.merge(&gateway2)?;

    println!("Aggregated IoT sensor data:");
    println!("Total readings: {}", gateway1.reading_count());

    // Display readings by type
    for reading in gateway1.readings_by_type(SensorType::Temperature) {
        println!(
            "Temperature: {:.1}°C (sensor {}, quality: {:?})",
            reading.value as f32 / 100.0,
            reading.sensor_id,
            reading.quality
        );
    }

    for reading in gateway1.readings_by_type(SensorType::Humidity) {
        println!(
            "Humidity: {:.1}% (sensor {}, quality: {:?})",
            reading.value as f32 / 100.0,
            reading.sensor_id,
            reading.quality
        );
    }

    for reading in gateway1.readings_by_type(SensorType::Pressure) {
        println!(
            "Pressure: {:.2} hPa (sensor {}, battery: {}%, signal: {}%)",
            reading.value as f32 / 100.0,
            reading.sensor_id,
            reading.battery_level * 100 / 255,
            reading.signal_strength * 100 / 255
        );
    }

    // Calculate average temperature for location 1
    if let Some(avg_temp) = gateway1.average_value(SensorType::Temperature, 1, 10000, 2000) {
        println!("Average temperature: {:.1}°C", avg_temp / 100.0);
    }

    // Check for sensors with low battery
    let low_battery_count = gateway1.low_battery_sensors(50).count();
    if low_battery_count > 0 {
        println!("Warning: {} sensors have low battery", low_battery_count);
    }

    Ok(())
}
