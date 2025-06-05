#!/bin/bash
# STM32 NUCLEO-F767ZI CRDT Demo Flashing Script

set -e

echo "🚀 CRDTosphere STM32 NUCLEO-F767ZI Demo"
echo "======================================="

# Check if board is connected
if ! lsusb | grep -q "STMicroelectronics"; then
    echo "❌ STM32 NUCLEO board not detected!"
    echo "   Please connect your NUCLEO-F767ZI board"
    echo "   Expected USB device: STMicroelectronics ST-LINK"
    exit 1
fi

echo "✅ STM32 NUCLEO-F767ZI detected"

# Build the project (debug version for RTT support)
echo "🔨 Building project (debug version for RTT support)..."
cargo build --target thumbv7em-none-eabihf

if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Build successful"

# Flash using different methods (auto-detect available tool)
echo "📡 Detecting flashing tool..."

if command -v probe-rs &> /dev/null; then
    echo "📡 Flashing with probe-rs..."
    probe-rs download --chip STM32F767ZI target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo
    FLASH_SUCCESS=$?
    if [ $FLASH_SUCCESS -eq 0 ]; then
        echo "🔄 Resetting target..."
        probe-rs reset --chip STM32F767ZI
    fi
elif command -v openocd &> /dev/null; then
    echo "📡 Flashing with OpenOCD..."
    openocd -f interface/stlink.cfg -f target/stm32f7x.cfg \
            -c "program target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo verify reset exit"
    FLASH_SUCCESS=$?
elif command -v st-flash &> /dev/null; then
    echo "📡 Flashing with st-flash..."
    # Convert ELF to binary
    arm-none-eabi-objcopy -O binary \
        target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo \
        target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo.bin
    
    if [ $? -ne 0 ]; then
        echo "❌ Binary conversion failed!"
        exit 1
    fi
    
    st-flash write target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo.bin 0x8000000
    FLASH_SUCCESS=$?
else
    echo "❌ No flashing tool found!"
    echo ""
    echo "Please install one of the following tools:"
    echo ""
    echo "1. probe-rs (Recommended - Modern Rust-based tool):"
    echo "   cargo install probe-rs --features cli"
    echo ""
    echo "2. OpenOCD (Traditional open-source tool):"
    echo "   sudo apt install openocd  # Ubuntu/Debian"
    echo "   brew install openocd      # macOS"
    echo ""
    echo "3. stlink-tools (STMicroelectronics official):"
    echo "   sudo apt install stlink-tools  # Ubuntu/Debian"
    echo "   brew install stlink            # macOS"
    echo ""
    exit 1
fi

if [ $FLASH_SUCCESS -eq 0 ]; then
    echo "✅ Flashing complete!"
    echo ""
    echo "🎯 Demo Instructions:"
    echo "   - LED1 (Green): Insert operations (button press, add devices)"
    echo "   - LED2 (Blue): Delete operations (remove devices, tombstones)"
    echo "   - LED3 (Red): Merge operations (node synchronization)"
    echo "   - User button: Trigger manual operations"
    echo ""
    echo "📊 Monitor serial output with: ./monitor.sh"
    echo "🐛 Start debugging session with: ./debug.sh"
    echo ""
    echo "🔄 The demo will automatically cycle through different scenarios:"
    echo "   1. Full CRDT demonstration sequence"
    echo "   2. Device registry operations"
    echo "   3. Device removal operations"
    echo "   4. Network merge simulation"
    echo ""
    echo "Press the blue user button on the board to manually trigger operations!"
else
    echo "❌ Flashing failed!"
    exit 1
fi
