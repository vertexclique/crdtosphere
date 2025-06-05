<h1 align="center"><img src="art/crdtosphere_logo_banner.png"/></h1>
<!-- <h1 align="center">CRDTosphere</h1> -->
<div align="center">
 <strong>
   CRDTosphere: Universal Embedded CRDTs for Distributed Coordination
 </strong>
</div>

<br />

<div align="center">
  <!-- Crates version -->
  <a href="https://crates.io/crates/crdtosphere">
    <img src="https://img.shields.io/crates/v/crdtosphere.svg?style=flat-square"
    alt="Crates.io version" />
  </a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/crdtosphere">
    <img src="https://img.shields.io/crates/d/crdtosphere.svg?style=flat-square"
      alt="Download" />
  </a>
  <!-- docs.rs docs -->
  <a href="https://docs.rs/crdtosphere">
    <img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square"
      alt="docs.rs docs" />
  </a>
  <!-- Build status -->
  <a href="https://github.com/vertexclique/crdtosphere/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/vertexclique/crdtosphere/ci.yml?style=flat-square"
      alt="Build Status" />
  </a>
</div>

<div align="center">
  <h3>
    <a href="https://docs.rs/crdtosphere">
      API Docs
    </a>
    <span> | </span>
    <a href="examples/">
      Examples
    </a>
    <span> | </span>
    <a href="CONTRIBUTING.md">
      Contributing
    </a>
    <span> | </span>
    <a href="https://github.com/vertexclique/crdtosphere/discussions">
      Discussions
    </a>
  </h3>
</div>

CRDTosphere is a comprehensive `no_std` Rust library implementing Conflict-free Replicated Data Types (CRDTs) optimized for embedded systems. It provides ultra-efficient, configurable CRDT implementations for automotive, robotics, IoT, and industrial applications across multiple platforms.

## IMPORTANT SAFETY DISCLAIMER

**This library is intended for NON-SAFETY-CRITICAL applications only.** While CRDTosphere includes safety-oriented features and compliance support frameworks, it should NOT be used for safety-critical functions such as:

- Primary vehicle control systems (steering, braking, acceleration)
- Life-support or medical devices
- Flight control systems
- Nuclear reactor control
- Emergency shutdown systems

**Recommended Use Cases:**
- Infotainment systems
- Telematics and connectivity features
- Non-critical sensor data aggregation
- Configuration management
- Diagnostic and monitoring systems
- User preference synchronization

The automotive examples in this library are for educational and demonstration purposes only. Any production automotive use should be limited to non-safety-critical domains such as infotainment, user preferences, and diagnostic data collection.

## Features

- **Universal Platform Support** - AURIX, STM32, ARM Cortex-M, RISC-V
- **Configurable Memory** - 2KB to 1MB+ budgets with compile-time verification
- **Multi-Domain Ready** - Automotive, robotics, IoT, industrial applications
- **Safety Critical** - ISO 26262, IEC 61508, DO-178C compliance support
- **Ultra-Efficient** - 5-100 byte CRDT instances with hardware optimizations
- **No Dynamic Allocation** - Pure static allocation for deterministic behavior
- **Real-Time Guarantees** - Bounded execution time (<1000 CPU cycles)

## Quick Start

Add CRDTosphere to your `Cargo.toml`:

```toml
[dependencies]
crdtosphere = { version = "0.1", default-features = false }

# Enable platform-specific optimizations
[features]
stm32 = ["crdtosphere/stm32"]
# OR
aurix = ["crdtosphere/aurix"]
```

Configure memory for your platform:

```rust
#![no_std]
use crdtosphere::prelude::*;

// Define memory configuration for your platform
define_memory_config! {
    name: MyPlatformConfig,
    total_memory: 32 * 1024,  // 32KB budget
    max_registers: 100,
    max_counters: 50,
    max_sets: 20,
    max_maps: 10,
    max_nodes: 32,
}

// Use configurable CRDTs
let mut sensor_reading = LWWRegister::<i16, MyPlatformConfig>::new();
sensor_reading.set(42, clock.now());

// Automatic conflict resolution
sensor_reading.merge(&other_node_reading)?;
```

## Platform Support

| Platform | Architecture | Memory | Use Cases |
|----------|-------------|---------|-----------|
| **AURIX TC3xx/TC4xx** | TriCore/ARM Cortex-R52 | 240KB-1MB | Automotive ECUs, safety systems |
| **STM32 Series** | ARM Cortex-M0/M3/M4/M7 | 4KB-2MB | General embedded, IoT, robotics |
| **ARM Cortex-M** | M0/M0+/M3/M4/M7 | 2KB-1MB+ | IoT devices, sensor networks |
| **RISC-V** | RV32I/M/A/C | 32KB-8MB+ | Edge computing, custom applications |

## Domain Applications

### Automotive
```rust
// Multi-ECU sensor fusion with ISO 26262 compliance
let mut temp_fusion = AutomotiveSensorFusion::<i16, AutomotiveConfig>::new();
temp_fusion.add_reading(85, ecu_1_clock, SENSOR_RELIABILITY_HIGH);
temp_fusion.add_reading(87, ecu_2_clock, SENSOR_RELIABILITY_MEDIUM);
let consensus_temp = temp_fusion.consensus_value();
```

### Robotics
```rust
// Multi-robot task allocation
let mut task_allocation = RobotTaskAllocation::<RoboticsConfig>::new();
task_allocation.assign_task(task_id, robot_id, clock.now());
```

### IoT
```rust
// Device mesh coordination
let mut device_mesh = DeviceMesh::<IoTConfig>::new();
device_mesh.add_device(device_id, capabilities, clock.now());
```

### Industrial
```rust
// Equipment health monitoring
let mut equipment_health = EquipmentHealth::<IndustrialConfig>::new();
equipment_health.record_vibration(machine_id, level, clock.now());
```

## Memory Configurations

Pre-configured setups for common platforms:

```rust
// High-performance automotive ECU
use crdtosphere::configs::AutomotiveECUConfig;  // 128KB budget

// General embedded device
use crdtosphere::configs::STM32F4Config;        // 32KB budget

// Constrained IoT sensor
use crdtosphere::configs::IoTSensorConfig;      // 4KB budget

// Industrial controller
use crdtosphere::configs::IndustrialConfig;     // 256KB budget
```

## Examples

- **[Automotive](examples/automotive/)** - ECU coordination, sensor fusion, safety systems
- **[Robotics](examples/robotics/)** - Swarm coordination, task allocation, SLAM
- **[IoT](examples/iot/)** - Device mesh, sensor networks, low-power coordination
- **[Industrial](examples/industrial/)** - Production monitoring, predictive maintenance
- **[Platforms](examples/platforms/)** - Platform-specific optimizations

## CRDT Types

| Type | Description | Memory | Use Case |
|------|-------------|---------|----------|
| **LWWRegister** | Last-writer-wins register | 5-16 bytes | Sensor readings, configuration |
| **GCounter** | Grow-only counter | 8-32 bytes | Event counting, telemetry |
| **ORSet** | Observed-remove set | 6-64 bytes | Feature flags, device lists |
| **LWWMap** | Last-writer-wins map | Variable | Key-value configuration |

## Safety & Compliance

- **ISO 26262** (Automotive) - ASIL-A through ASIL-D support
- **IEC 61508** (Industrial) - SIL-1 through SIL-4 support  
- **DO-178C** (Aerospace) - DAL-A through DAL-E support
- **Deterministic behavior** with mathematical convergence guarantees
- **Bounded execution time** for real-time systems

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

- **Bug Reports** - [GitHub Issues](https://github.com/vertexclique/crdtosphere/issues)
- **Feature Requests** - [GitHub Discussions](https://github.com/vertexclique/crdtosphere/discussions)
- **Documentation** - Help improve our docs
- **Testing** - Add tests for new platforms or use cases

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

---

<div align="center">
  <strong>Built with ❤️ for the embedded systems community by vertexclique</strong>
</div>
