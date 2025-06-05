#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

//! Robot Swarm Coordination Example
//!
//! Multi-robot coordination using the robotics domain types for status sharing and coordination.

use crdtosphere::prelude::*;
use crdtosphere::robotics::{BatteryLevel, OperationalMode, Position3D, RobotStatus};

fn main() -> Result<(), CRDTError> {
    // Create robot status coordinators for different robots
    let mut robot1_status = RobotStatus::<DefaultConfig>::new(1);
    let mut robot2_status = RobotStatus::<DefaultConfig>::new(2);
    let mut robot3_status = RobotStatus::<DefaultConfig>::new(3);

    // Robot 1 updates its status
    robot1_status.update_status(
        OperationalMode::Active,
        Position3D::new(10000, 20000, 0), // 10m, 20m position (in mm)
        BatteryLevel::High,
        1000,
    )?;

    // Robot 2 updates its status
    robot2_status.update_status(
        OperationalMode::Active,
        Position3D::new(15000, 25000, 0), // 15m, 25m position
        BatteryLevel::Medium,
        1001,
    )?;

    // Robot 3 updates its status (low battery)
    robot3_status.update_status(
        OperationalMode::Idle,
        Position3D::new(5000, 30000, 0), // 5m, 30m position
        BatteryLevel::Low,
        1002,
    )?;

    // Merge all robot statuses for swarm coordination
    robot1_status.merge(&robot2_status)?;
    robot1_status.merge(&robot3_status)?;

    println!("Robot Swarm Coordination Status:");
    println!("Total robots in swarm: {}", robot1_status.robot_count());
    println!("Operational robots: {}", robot1_status.operational_count());

    // Display all robot statuses
    for robot in robot1_status.all_robots() {
        println!(
            "Robot {}: {:?} at ({:.1}m, {:.1}m) - Battery: {:?}",
            robot.robot_id,
            robot.mode,
            robot.position.x as f32 / 1000.0,
            robot.position.y as f32 / 1000.0,
            robot.battery
        );
    }

    // Check for robots needing attention
    let attention_needed: Vec<_> = robot1_status.robots_needing_attention().collect();
    if !attention_needed.is_empty() {
        println!("\nRobots needing attention:");
        for robot in attention_needed {
            println!(
                "  Robot {}: {:?} - Battery: {:?}",
                robot.robot_id, robot.mode, robot.battery
            );
        }
    }

    // Find nearest robot to a target position
    let target_position = Position3D::new(12000, 22000, 0); // 12m, 22m
    if let Some(nearest) = robot1_status.nearest_robot(&target_position) {
        let distance_mm = (nearest.position.distance_squared(&target_position) as f64).sqrt();
        println!(
            "\nNearest robot to target (12m, 22m): Robot {} at {:.1}m distance",
            nearest.robot_id,
            distance_mm / 1000.0
        );
    }

    // Find robots within 10m of target
    let nearby_robots: Vec<_> = robot1_status
        .robots_within_distance(&target_position, 10_000_000) // 10m squared in mm²
        .collect();

    println!("Robots within 10m of target: {}", nearby_robots.len());
    for robot in nearby_robots {
        println!(
            "  Robot {} at ({:.1}m, {:.1}m)",
            robot.robot_id,
            robot.position.x as f32 / 1000.0,
            robot.position.y as f32 / 1000.0
        );
    }

    Ok(())
}
