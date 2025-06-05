# STM32 NUCLEO-F767ZI CRDTosphere Demo

This demonstration showcases the CRDTosphere library running on the STM32 NUCLEO-F767ZI development board, using the three user LEDs to provide visual feedback for different CRDT operations.

## 🎯 Demo Overview

The demo uses the board's LEDs to indicate CRDT operations in real-time:

- **🟢 LED1 (Green, PB0)**: Insert operations (add elements, increment counters)
- **🔵 LED2 (Blue, PB7)**: Delete operations (remove elements, tombstones)  
- **🔴 LED3 (Red, PB14)**: Merge operations (synchronize between nodes)

## 🔧 Hardware Requirements

- **STM32 NUCLEO-F767ZI** development board
- USB cable (USB-A to USB-B micro)
- Computer with USB port

### Board Specifications
- **MCU**: STM32F767ZI (ARM Cortex-M7 @ 216MHz)
- **Memory**: 512KB SRAM, 2MB Flash
- **LEDs**: 3 user LEDs (LD1/Green, LD2/Blue, LD3/Red)
- **Button**: 1 user button (blue button, PC13)
- **Debugger**: Integrated ST-LINK/V2-1

## 🚀 Quick Start

### 1. Prerequisites

Install the required tools:

```bash
# Install Rust embedded target
rustup target add thumbv7em-none-eabihf

# Install flashing tool (choose one)
cargo install probe-rs --features cli  # Recommended
# OR
sudo apt install openocd               # Ubuntu/Debian
brew install openocd                   # macOS
# OR  
sudo apt install stlink-tools          # Ubuntu/Debian
brew install stlink                    # macOS
```

### 2. Connect Hardware

1. Connect the NUCLEO-F767ZI board to your computer via USB
2. The board should appear as a USB device (ST-LINK)
3. No additional drivers needed on Linux/macOS (Windows may need ST-LINK drivers)

### 3. Build and Flash

```bash
# Navigate to the demo directory
cd examples/stm32_nucleo_f767zi_demo

# Make scripts executable
chmod +x flash.sh debug.sh monitor.sh

# Build and flash the demo
./flash.sh
```

### 4. Monitor Output

```bash
# Monitor RTT/serial output (recommended)
./monitor.sh

# Or start a debug session
./debug.sh
```

## 🎮 Demo Interaction

### Automatic Demo Cycles

The demo automatically cycles through different scenarios every ~10 seconds:

1. **Full CRDT Sequence**: Complete demonstration of all CRDT types
2. **Device Registry**: Adding devices to the network
3. **Device Removal**: Removing devices (tombstone operations)
4. **Network Merge**: Simulating data synchronization between nodes

### Manual Interaction

- **Press the blue user button** to manually trigger button press events
- Each button press increments a distributed counter and lights the green LED
- The demo simulates multiple nodes (0-3) and shows how they synchronize

### LED Patterns

| LED | Operation | Pattern | Description |
|-----|-----------|---------|-------------|
| 🟢 Green | Insert | Single blink | Button press, add device |
| 🟢 Green | Insert | Double blink | Add device to registry |
| 🟢 Green | Insert | Triple blink | Configuration update |
| 🟢 Green | Insert | Solid (200ms) | Sensor data reading |
| 🔵 Blue | Delete | Single blink | Remove device (tombstone) |
| 🔴 Red | Merge | Single blink | Counter merge |
| 🔴 Red | Merge | Double blink | Registry merge |
| 🔴 Red | Merge | Triple blink | Configuration merge |
| 🔴 Red | Merge | Solid (300ms) | Sensor data merge |

### Startup Sequence

On power-up, the demo shows:
1. Sequential LED activation (Green → Blue → Red)
2. All LEDs off
3. Quick flash of all LEDs
4. Demo ready state

## 📊 CRDT Types Demonstrated

The demo showcases four different CRDT types:

### 1. GCounter (Grow-only Counter)
- **Purpose**: Count button presses across nodes
- **LED**: Green (single blink)
- **Properties**: Monotonically increasing, conflict-free

### 2. ORSet (Observed-Remove Set)
- **Purpose**: Device registry with add/remove capability
- **LED**: Green (add), Blue (remove)
- **Properties**: Supports both additions and removals

### 3. LWWRegister (Last-Write-Wins Register)
- **Purpose**: Configuration management
- **LED**: Green (update), Red (merge)
- **Properties**: Conflict resolution by timestamp

### 4. LWWMap (Last-Write-Wins Map)
- **Purpose**: Sensor data storage
- **LED**: Green (insert), Red (merge)
- **Properties**: Key-value storage with timestamp-based conflict resolution

## 🔍 Monitoring and Debugging

### RTT Logging (Recommended)

Using probe-rs with RTT provides the best debugging experience:

```bash
# Flash and monitor in one command
probe-rs run --chip STM32F767ZI target/thumbv7em-none-eabihf/release/stm32_nucleo_f767zi_demo

# Or attach to running target
probe-rs attach --chip STM32F767ZI --rtt
```

### Serial Monitoring

Alternative monitoring methods:

```bash
# Using minicom
minicom -D /dev/ttyACM0 -b 115200

# Using screen  
screen /dev/ttyACM0 115200

# Using picocom
picocom -b 115200 /dev/ttyACM0
```

### Debug Session

Start a GDB debug session:

```bash
./debug.sh
```

Then connect with GDB:
```bash
arm-none-eabi-gdb target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo
(gdb) target remote localhost:3333
(gdb) load
(gdb) continue
```

## 📈 Performance Characteristics

### Memory Usage
- **Total CRDT Memory Budget**: 64KB (configurable)
- **Actual Usage**: ~200-500 bytes (highly efficient)
- **Memory Efficiency**: <1% of available SRAM

### Real-time Performance
- **Clock Speed**: 216MHz (maximum for STM32F767ZI)
- **CRDT Operations**: <1000 CPU cycles each
- **LED Response Time**: <1ms
- **Button Debouncing**: Software-based

### Platform Optimizations
- **Memory Alignment**: 4-byte aligned for ARM Cortex-M7
- **Cache Optimization**: Optimized for 32-byte cache lines
- **Interrupt Safety**: Lock-free CRDT operations
- **Power Efficiency**: WFI (Wait For Interrupt) during idle

## 🛠️ Customization

### Modify Memory Configuration

Edit `src/crdt_demo.rs`:

```rust
define_memory_config! {
    name: NucleoF767Config,
    total_memory: 64 * 1024,  // Adjust total budget
    max_registers: 8,         // Adjust limits
    max_counters: 4,
    max_sets: 4,
    max_maps: 2,
    max_nodes: 4,             // Number of simulated nodes
}
```

### Add New CRDT Operations

1. Add new methods to `CrdtDemo` in `src/crdt_demo.rs`
2. Define LED patterns in `src/led_controller.rs`
3. Integrate into the main loop in `src/main.rs`

### Modify LED Patterns

Edit `src/led_controller.rs` to change blink patterns:

```rust
pub enum BlinkPattern {
    Single,           // Single blink
    Double,           // Double blink  
    Triple,           // Triple blink
    Solid(u32),       // Solid for N milliseconds
    Custom(Vec<u32>), // Custom pattern
}
```

## 🔧 Troubleshooting

### Build Issues

```bash
# Clean and rebuild
cargo clean
cargo build --release --target thumbv7em-none-eabihf

# Check target installation
rustup target list --installed | grep thumbv7em
```

### Flashing Issues

```bash
# Check board connection
lsusb | grep STMicroelectronics

# Try different flashing tool
./flash.sh  # Auto-detects available tools

# Manual flashing with specific tool
probe-rs run --chip STM32F767ZI target/thumbv7em-none-eabihf/release/stm32_nucleo_f767zi_demo
```

### No LED Activity

1. Check power (USB connection)
2. Verify flashing was successful
3. Press reset button on board
4. Check RTT/serial output for error messages

### No Serial Output

1. Try RTT monitoring: `probe-rs attach --chip STM32F767ZI --rtt`
2. Check USB connection
3. Verify correct serial port: `ls /dev/ttyACM*`
4. Try different baud rate (115200 is default)

## 📚 Educational Value

This demo illustrates key CRDT concepts:

- **Conflict-free Convergence**: Multiple nodes always converge to the same state
- **Commutativity**: Order of operations doesn't matter (A ⊔ B = B ⊔ A)
- **Associativity**: Grouping doesn't matter ((A ⊔ B) ⊔ C = A ⊔ (B ⊔ C))
- **Idempotency**: Duplicate operations are safe (A ⊔ A = A)
- **Distributed Coordination**: No central authority needed
- **Real-time Performance**: Suitable for embedded systems

## 🔗 Related Examples

- [`examples/platforms/stm32_optimization.rs`](../platforms/stm32_optimization.rs) - STM32 platform optimizations
- [`examples/automotive_ecu_network/`](../automotive_ecu_network/) - Automotive ECU network simulation
- [`examples/iot/sensor_mesh.rs`](../iot/sensor_mesh.rs) - IoT sensor mesh networking

## 📄 License

