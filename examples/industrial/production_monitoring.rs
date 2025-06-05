#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

//! Industrial Production Monitoring Example
//!
//! Manufacturing line monitoring using industrial domain types for equipment coordination.

use crdtosphere::industrial::{EquipmentRegistry, EquipmentStatus, MaintenanceState};
use crdtosphere::prelude::*;

fn main() -> Result<(), CRDTError> {
    // Create equipment registries for different production controllers
    let mut controller1 = EquipmentRegistry::<DefaultConfig>::new(1);
    let mut controller2 = EquipmentRegistry::<DefaultConfig>::new(2);
    let mut controller3 = EquipmentRegistry::<DefaultConfig>::new(3);

    // Production counters for tracking output
    let mut station1_count = GCounter::<DefaultConfig>::new(1);
    let mut station2_count = GCounter::<DefaultConfig>::new(2);
    let mut station3_count = GCounter::<DefaultConfig>::new(3);

    // Register production equipment
    // Station 1: Motor (type 0x2001)
    controller1.register_equipment(101, 0x2001, 1000)?;
    controller1.update_equipment_status(101, EquipmentStatus::Running, 1001)?;

    // Station 2: Pump (type 0x2002)
    controller2.register_equipment(102, 0x2002, 1000)?;
    controller2.update_equipment_status(102, EquipmentStatus::Running, 1001)?;

    // Station 3: Conveyor (type 0x2003)
    controller3.register_equipment(103, 0x2003, 1000)?;
    controller3.update_equipment_status(103, EquipmentStatus::Running, 1001)?;

    // Update equipment operating metrics
    controller1.update_equipment_metrics(101, 1200, 5000, 1002)?; // 1200 hours, 5000 cycles
    controller2.update_equipment_metrics(102, 800, 3500, 1002)?; // 800 hours, 3500 cycles
    controller3.update_equipment_metrics(103, 1500, 7500, 1002)?; // 1500 hours, 7500 cycles

    // Schedule maintenance
    controller1.schedule_maintenance(101, 1000 + 86400000, 1003)?; // 24 hours from now
    controller2.schedule_maintenance(102, 1000 + 172800000, 1003)?; // 48 hours from now

    // Station 3 equipment needs maintenance
    controller3.update_maintenance_state(103, MaintenanceState::PreventiveDue, 1004)?;

    // Simulate production counts
    // Station 1 produces 50 units
    for _ in 0..50 {
        station1_count.increment(1)?;
    }

    // Station 2 produces 35 units
    for _ in 0..35 {
        station2_count.increment(1)?;
    }

    // Station 3 produces 42 units
    for _ in 0..42 {
        station3_count.increment(1)?;
    }

    // Merge all equipment registries for plant-wide view
    controller1.merge(&controller2)?;
    controller1.merge(&controller3)?;

    // Merge production counts
    station1_count.merge(&station2_count)?;
    station1_count.merge(&station3_count)?;

    println!("Industrial Production Monitoring Report");
    println!("======================================");

    println!("\nProduction Summary:");
    println!("Station 1 produced: {} units", station1_count.node_value(1));
    println!("Station 2 produced: {} units", station1_count.node_value(2));
    println!("Station 3 produced: {} units", station1_count.node_value(3));
    println!("Total production: {} units", station1_count.value());

    println!("\nEquipment Status:");
    for equipment in controller1.all_equipment() {
        println!(
            "Equipment {}: {:?} - Type: 0x{:04X}",
            equipment.equipment_id, equipment.status, equipment.equipment_type
        );
        println!(
            "  Operating hours: {}, Cycles: {}",
            equipment.operating_hours, equipment.cycle_count
        );
        println!("  Maintenance: {:?}", equipment.maintenance_state);
    }

    // Check running equipment
    let running_count = controller1.running_equipment().count();
    println!(
        "\nRunning equipment: {}/{}",
        running_count,
        controller1.equipment_count()
    );

    // Check equipment requiring attention
    let attention_needed: Vec<_> = controller1.equipment_requiring_attention().collect();
    if !attention_needed.is_empty() {
        println!("\nEquipment requiring attention:");
        for equipment in attention_needed {
            println!(
                "  Equipment {}: {:?}",
                equipment.equipment_id, equipment.status
            );
        }
    }

    // Check maintenance requirements
    let maintenance_needed: Vec<_> = controller1.equipment_requiring_maintenance().collect();
    if !maintenance_needed.is_empty() {
        println!("\nEquipment requiring maintenance:");
        for equipment in maintenance_needed {
            println!(
                "  Equipment {}: {:?}",
                equipment.equipment_id, equipment.maintenance_state
            );
        }
    }

    // Check overdue maintenance (simulate current time as 2 days later)
    let current_time = 1000 + 172800000; // 48 hours later
    let overdue: Vec<_> = controller1
        .equipment_with_overdue_maintenance(current_time)
        .collect();
    if !overdue.is_empty() {
        println!("\nEquipment with overdue maintenance:");
        for equipment in overdue {
            let overdue_hours = (current_time - equipment.next_maintenance_due.as_u64()) / 3600000;
            println!(
                "  Equipment {}: {} hours overdue",
                equipment.equipment_id, overdue_hours
            );
        }
    }

    // Equipment efficiency calculation
    println!("\nEquipment Efficiency:");
    for equipment in controller1.all_equipment() {
        if equipment.status.is_operational() {
            // Simple efficiency based on cycles per hour
            let efficiency = if equipment.operating_hours > 0 {
                equipment.cycle_count as f32 / equipment.operating_hours as f32
            } else {
                0.0
            };
            println!(
                "  Equipment {}: {:.1} cycles/hour",
                equipment.equipment_id, efficiency
            );
        }
    }

    Ok(())
}
