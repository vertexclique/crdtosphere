#!/bin/bash
# Debug session for STM32 NUCLEO-F767ZI

echo "🐛 Starting debug session for STM32 NUCLEO-F767ZI..."

# Check if board is connected
if ! lsusb | grep -q "STMicroelectronics"; then
    echo "❌ STM32 NUCLEO board not detected!"
    echo "   Please connect your NUCLEO-F767ZI board"
    exit 1
fi

echo "✅ STM32 NUCLEO-F767ZI detected"

# Build debug version first
echo "🔨 Building debug version..."
cargo build --target thumbv7em-none-eabihf

if [ $? -ne 0 ]; then
    echo "❌ Debug build failed!"
    exit 1
fi

echo "✅ Debug build successful"

if command -v probe-rs &> /dev/null; then
    echo "🔍 Using probe-rs for debugging..."
    echo ""
    echo "Available probe-rs debug commands:"
    echo "  probe-rs attach --chip STM32F767ZI                    # Attach to running target"
    echo "  probe-rs run --chip STM32F767ZI <binary>              # Flash and run with RTT"
    echo "  probe-rs gdb --chip STM32F767ZI <binary>              # Start GDB server"
    echo ""
    echo "Starting RTT monitoring session..."
    probe-rs attach --chip STM32F767ZI --rtt-scan-memory target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo
    
elif command -v openocd &> /dev/null; then
    echo "🔍 Using OpenOCD for debugging..."
    
    # Start OpenOCD in background
    echo "Starting OpenOCD debug server..."
    openocd -f interface/stlink.cfg -f target/stm32f7x.cfg &
    OPENOCD_PID=$!
    
    # Give OpenOCD time to start
    sleep 2
    
    echo ""
    echo "OpenOCD debug server started (PID: $OPENOCD_PID)"
    echo ""
    echo "Connect with GDB using:"
    echo "  arm-none-eabi-gdb target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo"
    echo "  (gdb) target remote localhost:3333"
    echo "  (gdb) monitor reset halt"
    echo "  (gdb) load"
    echo "  (gdb) continue"
    echo ""
    echo "Or use your favorite GDB frontend (e.g., VS Code, CLion, etc.)"
    echo ""
    echo "OpenOCD commands available:"
    echo "  monitor reset halt    # Reset and halt the target"
    echo "  monitor reset run     # Reset and run the target"
    echo "  monitor halt          # Halt the target"
    echo "  monitor resume        # Resume execution"
    echo ""
    
    # Wait for user to finish debugging
    echo "Press Enter to stop OpenOCD debug server..."
    read
    
    echo "Stopping OpenOCD..."
    kill $OPENOCD_PID 2>/dev/null || true
    
elif command -v st-util &> /dev/null; then
    echo "🔍 Using st-util for debugging..."
    
    # Start st-util in background
    echo "Starting st-util debug server..."
    st-util &
    STUTIL_PID=$!
    
    # Give st-util time to start
    sleep 2
    
    echo ""
    echo "st-util debug server started (PID: $STUTIL_PID)"
    echo ""
    echo "Connect with GDB using:"
    echo "  arm-none-eabi-gdb target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo"
    echo "  (gdb) target remote localhost:4242"
    echo "  (gdb) load"
    echo "  (gdb) continue"
    echo ""
    
    # Wait for user to finish debugging
    echo "Press Enter to stop st-util debug server..."
    read
    
    echo "Stopping st-util..."
    kill $STUTIL_PID 2>/dev/null || true
    
else
    echo "❌ No debug tool found!"
    echo ""
    echo "Please install one of the following debug tools:"
    echo ""
    echo "1. probe-rs (Recommended - Modern Rust-based tool with RTT):"
    echo "   cargo install probe-rs --features cli"
    echo ""
    echo "2. OpenOCD (Traditional open-source tool):"
    echo "   sudo apt install openocd  # Ubuntu/Debian"
    echo "   brew install openocd      # macOS"
    echo ""
    echo "3. st-util (Part of stlink-tools):"
    echo "   sudo apt install stlink-tools  # Ubuntu/Debian"
    echo "   brew install stlink            # macOS"
    echo ""
    echo "For the best debugging experience with RTT logging, use probe-rs!"
    exit 1
fi

echo "🏁 Debug session ended"
