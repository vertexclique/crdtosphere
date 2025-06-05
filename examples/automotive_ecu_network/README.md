# Automotive ECU Network Example

This example demonstrates a realistic automotive ECU network using CRDTosphere for distributed coordination between multiple STM32-based ECUs. It showcases real-world automotive safety systems with ISO 26262 compliance and can be simulated using Renode.

## IMPORTANT SAFETY DISCLAIMER

**This example is for EDUCATIONAL and DEMONSTRATION purposes only.** It should NOT be used for safety-critical automotive functions such as:

- Primary vehicle control systems (steering, braking, acceleration)
- Safety-critical engine management
- Life-safety systems
- Production automotive ECUs

**This example is suitable for NON-SAFETY-CRITICAL automotive applications such as:**
- Infotainment systems
- Telematics and connectivity features
- Non-critical sensor data aggregation
- Configuration management
- Diagnostic and monitoring systems
- User preference synchronization

Any production automotive use should be limited to non-safety-critical domains and must undergo proper automotive qualification and certification processes.

## System Overview

### Architecture

The system consists of 4 ECUs connected via CAN bus, each with different safety levels according to ISO 26262:

- **Engine Control Unit (ASIL-D)**: Engine parameters, temperature monitoring, emergency shutdown
- **Brake Control Unit (ASIL-D)**: ABS, brake pressure, emergency braking coordination  
- **Steering Control Unit (ASIL-C)**: Power steering, stability control
- **Gateway ECU (ASIL-B)**: Central coordination, diagnostics, system monitoring

### CRDTs Used

- **SafetyCRDT**: Emergency brake commands with ISO 26262 safety prioritization
- **SensorFusion**: Multi-sensor temperature readings with reliability weighting
- **LWWRegister**: System configuration parameters
- **GCounter**: Error counters and diagnostic data

### Real-World Features

- **CAN Bus Communication**: Proper message prioritization and real-time constraints
- **Safety-Critical Coordination**: Emergency brake propagation with ASIL-level enforcement
- **Multi-Sensor Fusion**: Temperature readings from multiple sources with outlier detection
- **Fault Tolerance**: ECU failures don't break the system
- **ISO 26262 Compliance**: ASIL-level safety prioritization and validation
- **Real-Time Performance**: Sub-millisecond CRDT operations
- **Memory Efficiency**: Fixed memory usage suitable for embedded systems

## Building and Running

### Prerequisites

1. **Rust Toolchain**:
   ```bash
   rustup target add thumbv7em-none-eabihf
   ```

2. **Renode Simulation Framework**:
   ```bash
   # Install Renode (see https://renode.io)
   # On Ubuntu/Debian:
   wget https://github.com/renode/renode/releases/latest/download/renode_*_amd64.deb
   sudo dpkg -i renode_*_amd64.deb
   ```

3. **ARM GCC Toolchain** (for debugging):
   ```bash
   sudo apt install gcc-arm-none-eabi
   ```

### Building the Example

```bash
# Navigate to the example directory
cd examples/automotive_ecu_network

# Build all ECU binaries
cargo build --release --target thumbv7em-none-eabihf

# Build specific ECU
cargo build --release --target thumbv7em-none-eabihf --bin engine_ecu
```

### Running the Simulation

#### Option 1: Using the Test Script (Recommended)

The easiest way to test the automotive scenarios is using the provided test script:

```bash
# Make the script executable (if not already)
chmod +x test_scenarios.sh

# Show available commands
./test_scenarios.sh help

# Build the project
./test_scenarios.sh build

# Run specific scenarios with precise duration control
./test_scenarios.sh emergency 15        # Emergency brake for exactly 15 seconds
./test_scenarios.sh normal 10           # Normal operation for exactly 10 seconds
./test_scenarios.sh fusion 8            # Sensor fusion for exactly 8 seconds
./test_scenarios.sh failure 12          # ECU failure test for exactly 12 seconds

# Interactive mode with real-time CRDT monitoring
./test_scenarios.sh interactive
```

**Key Features:**
- **Precise Duration Control**: Runs for exactly the specified number of seconds
- **Complete CRDT State Dump**: Shows actual hex values from all ECUs
- **Pause/Resume Control**: All ECUs can be paused and resumed simultaneously
- **Memory-Mapped Access**: Direct access to sensor and CAN regions
- **Real-time Monitoring**: Live CRDT state inspection during simulation

#### Option 2: Manual Renode Execution

1. **Start Renode**:
   ```bash
   renode
   ```

2. **Load the simulation**:
   ```
   (monitor) include @renode/simulation.resc
   ```

3. **The simulation will automatically start with all ECUs running**

## Demonstration Scenarios

### Using the Test Script

The `test_scenarios.sh` script provides easy access to all scenarios:

| Command | Description | Example |
|---------|-------------|---------|
| `./test_scenarios.sh normal [duration]` | Normal operation | `./test_scenarios.sh normal 10` |
| `./test_scenarios.sh emergency [duration]` | Emergency brake test | `./test_scenarios.sh emergency 15` |
| `./test_scenarios.sh fusion [duration]` | Sensor fusion test | `./test_scenarios.sh fusion 10` |
| `./test_scenarios.sh partition [duration]` | Network partition test | `./test_scenarios.sh partition 12` |
| `./test_scenarios.sh failure [duration]` | ECU failure test | `./test_scenarios.sh failure 8` |
| `./test_scenarios.sh performance [duration]` | Performance test | `./test_scenarios.sh performance 5` |
| `./test_scenarios.sh safety [duration]` | Safety validation | `./test_scenarios.sh safety 10` |
| `./test_scenarios.sh all [duration]` | Run all scenarios | `./test_scenarios.sh all 5` |
| `./test_scenarios.sh interactive` | Interactive mode | `./test_scenarios.sh interactive` |

### Scenario Details

#### 1. Normal Operation (`normal`)
- All ECUs start and begin normal operation
- Temperature readings are shared via CAN bus
- Sensor fusion operates across multiple ECUs
- System configuration is synchronized

#### 2. Emergency Brake Scenario (`emergency`)
- Engine ECU detects critical temperature (115°C)
- Emergency brake command is triggered with ASIL-D priority
- All ECUs receive and respond to emergency brake
- Engine shuts down, brake systems activate

#### 3. Sensor Fusion Scenario (`fusion`)
- Multiple ECUs generate temperature readings
- Sensor fusion combines readings with reliability weighting
- Outlier detection identifies faulty sensors
- System maintains accurate temperature estimates

#### 4. Network Partition Scenario (`partition`)
- Steering ECU is disconnected from CAN bus
- Remaining ECUs continue operation
- Steering ECU reconnects and synchronizes state
- Demonstrates fault tolerance

#### 5. ECU Failure Scenario (`failure`)
- Brake ECU fails (simulated by stopping)
- Other ECUs detect failure and adapt
- Brake ECU recovers and rejoins network
- State synchronization occurs automatically

#### 6. Performance Test (`performance`)
- High-frequency CAN message generation
- Tests real-time performance under load
- Validates sub-millisecond CRDT operations
- Measures network throughput

#### 7. Safety Validation Test (`safety`)
- Tests ISO 26262 ASIL-level enforcement
- Low-priority ECU attempts emergency brake (rejected)
- High-priority ECU sends emergency brake (accepted)
- Validates safety hierarchy compliance

### Manual Renode Commands

If using Renode directly, these commands are available:
```
start_normal_operation
emergency_brake_scenario
sensor_fusion_scenario
network_partition_scenario
ecu_failure_scenario
performance_test
safety_validation_test
```

## Monitoring and Analysis

### Real-Time Monitoring

The simulation provides comprehensive monitoring:

- **CAN Traffic Analysis**: All CAN messages are logged with timestamps
- **Temperature Trends**: Multi-ECU temperature fusion visualization
- **Safety Events**: Emergency brake activations and safety violations
- **Performance Metrics**: CRDT operation timing and memory usage
- **Network Health**: ECU connectivity and message success rates

### Key Metrics Tracked

- **Messages Transmitted/Received**: CAN bus utilization
- **CRDT Merge Operations**: Distributed state synchronization
- **Safety Violations**: ISO 26262 compliance monitoring
- **Sensor Readings**: Multi-sensor data quality
- **Emergency Activations**: Safety-critical event frequency
- **Error Counts**: System fault detection

## Technical Implementation

### ECU State Management

Each ECU maintains a complete CRDT state:

```rust
pub struct ECUState {
    pub emergency_brake: SafetyCRDT<BrakeCommand, DefaultConfig>,
    pub temperature_fusion: SensorFusion<f32, DefaultConfig>,
    pub system_config: LWWRegister<SystemConfig, DefaultConfig>,
    pub error_counter: GCounter<DefaultConfig>,
    // ... additional state
}
```

### CAN Protocol Integration

CRDTs are serialized into CAN frames with proper prioritization:

```rust
// Emergency brake (highest priority)
CANMessageId::EmergencyBrake = 0x100,

// Sensor fusion
CANMessageId::TemperatureFusion = 0x200,

// Configuration
CANMessageId::EngineConfig = 0x300,

// Diagnostics  
CANMessageId::ErrorCounts = 0x400,
```

### Safety Level Enforcement

ISO 26262 ASIL levels are enforced in CRDT operations:

```rust
impl ECUNodeId {
    pub fn safety_level(self) -> SafetyLevel {
        match self {
            ECUNodeId::Engine => SafetyLevel::automotive(ASILLevel::ASIL_D),
            ECUNodeId::Brake => SafetyLevel::automotive(ASILLevel::ASIL_D),
            ECUNodeId::Steering => SafetyLevel::automotive(ASILLevel::ASIL_C),
            ECUNodeId::Gateway => SafetyLevel::automotive(ASILLevel::ASIL_B),
        }
    }
}
```

### Memory Usage

The system uses fixed memory allocation suitable for embedded systems:

- **Engine ECU**: ~2KB total CRDT state
- **Brake ECU**: ~1.5KB total CRDT state  
- **Steering ECU**: ~1KB total CRDT state
- **Gateway ECU**: ~1KB total CRDT state

## 🧪 Testing and Validation

### Unit Tests

```bash
# Run all tests
cargo test

# Run specific ECU tests
cargo test --bin engine_ecu
```

### Integration Tests

The simulation includes comprehensive integration tests:

- **CRDT Convergence**: Verify eventual consistency
- **Safety Compliance**: Validate ISO 26262 enforcement
- **Fault Tolerance**: Test network partition recovery
- **Performance**: Measure real-time constraints
- **Memory Safety**: Validate bounded memory usage

### Property-Based Testing

Key properties validated:

1. **Eventual Consistency**: All ECUs converge to same state
2. **Safety Monotonicity**: Higher ASIL levels always win
3. **Bounded Execution**: All operations complete within time limits
4. **Memory Bounds**: No dynamic allocation or unbounded growth
5. **Fault Tolerance**: System survives ECU failures

## 🚀 Real-World Deployment

### Hardware Requirements

- **STM32F407** or compatible ARM Cortex-M4 microcontroller
- **CAN Transceiver** (e.g., TJA1050)
- **Temperature Sensors** (analog or digital)
- **Pressure Sensors** for brake/oil monitoring
- **Status LEDs** for visual feedback

### Production Considerations

1. **Watchdog Timers**: Ensure ECU responsiveness
2. **CRC Validation**: Data integrity on CAN bus
3. **Backup Power**: Critical state retention
4. **Diagnostic Interfaces**: OBD-II compliance
5. **Security**: CAN message authentication
6. **Certification**: ISO 26262 functional safety assessment

### Performance Characteristics

- **CRDT Operations**: < 100 CPU cycles
- **CAN Message Processing**: < 50 microseconds
- **Emergency Response**: < 1 millisecond
- **Memory Usage**: < 4KB per ECU
- **Network Bandwidth**: < 10% CAN utilization

## 📚 Educational Value

This example demonstrates:

### Distributed Systems Concepts
- **Conflict-Free Replicated Data Types (CRDTs)**
- **Eventual consistency in embedded systems**
- **Network partition tolerance**
- **Byzantine fault tolerance basics**

### Automotive Engineering
- **ISO 26262 functional safety**
- **CAN bus communication protocols**
- **Multi-ECU coordination**
- **Real-time embedded systems**

### Safety-Critical Systems
- **Safety level prioritization**
- **Fault detection and response**
- **Graceful degradation**
- **Emergency response coordination**

## 🔍 Troubleshooting

### Common Issues

1. **Build Errors**:
   ```bash
   # Ensure correct target
   rustup target add thumbv7em-none-eabihf
   
   # Check dependencies
   cargo check --target thumbv7em-none-eabihf
   ```

2. **Renode Simulation Issues**:
   ```bash
   # Check Renode installation
   renode --version
   
   # Verify platform file
   renode -e "include @renode/platform.repl"
   ```

3. **CAN Communication Problems**:
   - Check CAN bus connections in simulation
   - Verify message ID conflicts
   - Monitor CAN error counters

### Debug Commands

```bash
# Monitor CAN traffic
(monitor) emulation CreateCANHub "debug_can"
(monitor) automotive_can_bus AttachTo debug_can

# Check ECU status
(monitor) mach set "engine_ecu"
(monitor) sysbus.cpu PC

# Memory inspection
(monitor) sysbus ReadDoubleWord 0x20000000
```

## 🤝 Contributing

To extend this example:

1. **Add New ECUs**: Create additional ECU binaries
2. **Implement New CRDTs**: Add domain-specific data types
3. **Enhance Safety Features**: Implement additional ISO 26262 patterns
4. **Add Sensors**: Integrate more automotive sensors
5. **Improve Simulation**: Add more realistic vehicle dynamics

## 📄 License

This example is part of the CRDTosphere project and follows the same license terms.

---

This example showcases the power of CRDTs in safety-critical automotive systems, demonstrating how distributed coordination can be achieved without central points of failure while maintaining real-time performance and ISO 26262 compliance.
