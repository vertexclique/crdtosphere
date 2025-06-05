#![doc(
    html_logo_url = "https://github.com/vertexclique/crdtosphere/raw/master/art/crdtosphere_logo_square_trans.png"
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]

//! ![CRDTosphere](https://github.com/vertexclique/crdtosphere/raw/master/art/crdtosphere_logo_banner.png)
//!
//! **Universal Embedded CRDTs for Distributed Coordination**
//!
//! CRDTosphere is a comprehensive `no_std` Rust library implementing Conflict-free Replicated Data Types (CRDTs)
//! optimized for embedded systems. It provides ultra-efficient, configurable CRDT implementations for automotive,
//! robotics, IoT, and industrial applications across multiple platforms.
//!
//! ## Features
//!
//! - **Universal Platform Support** - AURIX, STM32, ARM Cortex-M, RISC-V
//! - **Configurable Memory** - 2KB to 1MB+ budgets with compile-time verification
//! - **Multi-Domain Ready** - Automotive, robotics, IoT, industrial applications
//! - **Safety Critical** - ISO 26262, IEC 61508, DO-178C compliance support
//! - **Ultra-Efficient** - 5-100 byte CRDT instances with hardware optimizations
//! - **No Dynamic Allocation** - Pure static allocation for deterministic behavior
//! - **Real-Time Guarantees** - Bounded execution time (<1000 CPU cycles)
//!
//! ## Feature Overview
//!
//! ### Domain-Specific Features
//! - [`automotive`] - Automotive ECU and safety-critical systems (ISO 26262, ASIL-D)
//! - [`robotics`] - Robotics and autonomous systems coordination
//! - [`iot`] - Internet of Things and sensor networks
//! - [`industrial`] - Industrial automation and control systems
//!
//! ### Platform-Specific Features - **Mostly mutually exclusive**
//! - `aurix` - AURIX TriCore automotive MCUs (multi-core, safety features)
//! - `stm32` - STM32 ARM Cortex-M MCUs (power management optimizations)
//! - `cortex-m` - Generic ARM Cortex-M platforms (memory constrained)
//! - `riscv` - RISC-V embedded processors (variable multi-core)
//!
//! ### Hardware Optimization Features
//! - `hardware` - Enable all hardware optimizations
//! - `hardware-atomic` - Hardware atomic operations for thread safety
//!
//! ### Serialization Features
//! - `serde` - Serde serialization support (no_std compatible)
//!
//! ## Platform Support Matrix
//!
//! | Feature | AURIX | STM32 | Cortex-M | RISC-V | Default |
//! |---------|-------|-------|----------|--------|---------|
//! | **Memory Alignment** | 32-byte | 4-byte | 4-byte | 8-byte | 4-byte |
//! | **Max Merge Cycles** | 500 | 200 | 100 | 300 | 150 |
//! | **Multi-core** | ✅ (3 cores) | ❌ | ❌ | ✅ (variable) | ❌ |
//! | **Safety Features** | ✅ ASIL-D | ❌ | ❌ | ❌ | ❌ |
//! | **Power Management** | ❌ | ✅ | ✅ | ❌ | ❌ |
//! | **Real-Time Bounds** | ✅ (100μs) | ✅ (50μs) | ✅ (25μs) | ✅ (30μs) | ✅ (40μs) |
//!
//! **Note**: Platform features are mutually exclusive. Choose one per build.
//!
//! ## Platform-Specific Usage Examples
//!
//! ### AURIX TriCore (Automotive Safety-Critical)
//! ```toml
//! [dependencies]
//! crdtosphere = { version = "0.1", features = ["automotive", "aurix", "hardware-atomic"] }
//! ```
//!
//! ### STM32 (IoT/Industrial)
//! ```toml
//! [dependencies]
//! crdtosphere = { version = "0.1", features = ["iot", "stm32", "serde"] }
//! ```
//!
//! ### Generic Cortex-M (Robotics)
//! ```toml
//! [dependencies]
//! crdtosphere = { version = "0.1", features = ["robotics", "cortex-m"] }
//! ```
//!
//! ### RISC-V (Research/Custom)
//! ```toml
//! [dependencies]
//! crdtosphere = { version = "0.1", features = ["industrial", "riscv", "hardware-atomic"] }
//! ```
//!
//! ## Quick Start
//!
//! ```rust
//! use crdtosphere::prelude::*;
//!
//! // Define memory configuration for your platform
//! define_memory_config! {
//!     name: MyPlatformConfig,
//!     total_memory: 32 * 1024,  // 32KB budget
//!     max_registers: 100,
//!     max_counters: 50,
//!     max_sets: 20,
//!     max_maps: 10,
//!     max_nodes: 32,
//! }
//!
//! fn example() -> Result<(), CRDTError> {
//!     // Use configurable CRDTs
//!     let mut sensor_reading = LWWRegister::<i16, MyPlatformConfig>::new(1);
//!     sensor_reading.set(42, 1000)?; // 1000 is your timestamp here.
//!     
//!     // Automatic conflict resolution
//!     let other_node_reading = LWWRegister::<i16, MyPlatformConfig>::new(2);
//!     sensor_reading.merge(&other_node_reading)?;
//!     Ok(())
//! }
//! ```
//!
//! ## CRDT Types Available
//!
//! ### Counters
//! - [`GCounter`] - Grow-only counter (increment only)
//! - [`PNCounter`] - Increment/decrement counter
//!
//! ### Registers
//! - [`LWWRegister`] - Last-Writer-Wins register
//! - [`MVRegister`] - Multi-Value register (concurrent writes preserved)
//!
//! ### Sets
//! - [`GSet`] - Grow-only set (add only)
//! - [`ORSet`] - Observed-Remove set (add and remove)
//!
//! ### Maps
//! - [`LWWMap`] - Last-Writer-Wins map
//!
//!
//! [`GCounter`]: crate::counters::GCounter
//! [`PNCounter`]: crate::counters::PNCounter
//! [`LWWRegister`]: crate::registers::LWWRegister
//! [`MVRegister`]: crate::registers::MVRegister
//! [`GSet`]: crate::sets::GSet
//! [`ORSet`]: crate::sets::ORSet
//! [`LWWMap`]: crate::maps::LWWMap

#![no_std]
#![deny(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::manual_flatten)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::explicit_counter_loop)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::const_is_empty)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::too_many_arguments)]
#![cfg_attr(test, allow(unused_mut))]

// Core infrastructure modules
pub mod clock;
pub mod error;
pub mod memory;
pub mod platform;
pub mod traits;

// Core CRDT modules (always available)
pub mod counters;
pub mod maps;
pub mod registers;
pub mod sets;

// Domain-specific CRDT modules
#[cfg(feature = "automotive")]
#[cfg_attr(docsrs, doc(cfg(feature = "automotive")))]
pub mod automotive;

#[cfg(feature = "robotics")]
#[cfg_attr(docsrs, doc(cfg(feature = "robotics")))]
pub mod robotics;

#[cfg(feature = "iot")]
#[cfg_attr(docsrs, doc(cfg(feature = "iot")))]
pub mod iot;

#[cfg(feature = "industrial")]
#[cfg_attr(docsrs, doc(cfg(feature = "industrial")))]
pub mod industrial;

// Configuration presets
pub mod configs;

/// Prelude module of CRDTosphere
///
/// Convenient re-exports for common CRDTosphere types and traits
pub mod prelude {

    // Re-export core traits
    pub use crate::traits::*;

    // Re-export memory configuration
    pub use crate::memory::{DefaultConfig, MemoryConfig, define_memory_config};

    // Re-export error types
    pub use crate::error::CRDTError;

    // Re-export clock types
    pub use crate::clock::CompactTimestamp;

    // Re-export configuration presets
    pub use crate::configs::*;

    // Re-export core CRDTs (always available)
    pub use crate::counters::{GCounter, PNCounter};
    pub use crate::maps::LWWMap;
    pub use crate::registers::{LWWRegister, MVRegister};
    pub use crate::sets::{GSet, ORSet};
}
