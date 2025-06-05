#!/bin/bash
# Serial monitor for STM32 NUCLEO-F767ZI demo

echo "📊 Starting serial monitor for CRDT demo..."

# Check if board is connected
if ! lsusb | grep -q "STMicroelectronics"; then
    echo "❌ STM32 NUCLEO board not detected!"
    echo "   Please connect your NUCLEO-F767ZI board"
    exit 1
fi

echo "✅ STM32 NUCLEO-F767ZI detected"

# Try different serial monitoring tools
if command -v probe-rs &> /dev/null; then
    echo "📡 Using probe-rs for RTT monitoring..."
    echo ""
    echo "RTT (Real-Time Transfer) provides high-speed logging without UART overhead."
    echo "This is the preferred method for monitoring the CRDT demo output."
    echo ""
    
    # Check if binary exists, if not try to build it
    # Use debug build for better RTT/defmt support
    BINARY_PATH="target/thumbv7em-none-eabihf/debug/stm32_nucleo_f767zi_demo"
    if [ ! -f "$BINARY_PATH" ]; then
        echo "🔨 Binary not found, building debug version for RTT monitoring..."
        cargo build --target thumbv7em-none-eabihf
        if [ $? -ne 0 ]; then
            echo "❌ Build failed! Cannot monitor without binary."
            exit 1
        fi
        echo "✅ Build successful"
    fi
    
    echo "Press Ctrl+C to exit monitoring"
    echo "================================"
    probe-rs attach --chip STM32F767ZI --rtt-scan-memory "$BINARY_PATH"
    
elif command -v minicom &> /dev/null; then
    # Find the STM32 serial port
    SERIAL_PORT=$(ls /dev/ttyACM* 2>/dev/null | head -1)
    if [ -n "$SERIAL_PORT" ]; then
        echo "📡 Using minicom on $SERIAL_PORT..."
        echo ""
        echo "Serial communication settings:"
        echo "  Port: $SERIAL_PORT"
        echo "  Baud rate: 115200"
        echo "  Data bits: 8"
        echo "  Stop bits: 1"
        echo "  Parity: None"
        echo ""
        echo "Minicom commands:"
        echo "  Ctrl+A Z  - Help menu"
        echo "  Ctrl+A X  - Exit minicom"
        echo "  Ctrl+A C  - Clear screen"
        echo ""
        echo "Press Ctrl+A X to exit monitoring"
        echo "=================================="
        minicom -D $SERIAL_PORT -b 115200
    else
        echo "❌ No serial port found (expected /dev/ttyACM*)"
        echo "   Make sure the STM32 board is connected and recognized"
        exit 1
    fi
    
elif command -v screen &> /dev/null; then
    SERIAL_PORT=$(ls /dev/ttyACM* 2>/dev/null | head -1)
    if [ -n "$SERIAL_PORT" ]; then
        echo "📡 Using screen on $SERIAL_PORT..."
        echo ""
        echo "Serial communication settings:"
        echo "  Port: $SERIAL_PORT"
        echo "  Baud rate: 115200"
        echo ""
        echo "Screen commands:"
        echo "  Ctrl+A K  - Kill session (exit)"
        echo "  Ctrl+A C  - Clear screen"
        echo "  Ctrl+A H  - Toggle logging to file"
        echo ""
        echo "Press Ctrl+A K to exit monitoring"
        echo "================================="
        screen $SERIAL_PORT 115200
    else
        echo "❌ No serial port found (expected /dev/ttyACM*)"
        echo "   Make sure the STM32 board is connected and recognized"
        exit 1
    fi
    
elif command -v picocom &> /dev/null; then
    SERIAL_PORT=$(ls /dev/ttyACM* 2>/dev/null | head -1)
    if [ -n "$SERIAL_PORT" ]; then
        echo "📡 Using picocom on $SERIAL_PORT..."
        echo ""
        echo "Serial communication settings:"
        echo "  Port: $SERIAL_PORT"
        echo "  Baud rate: 115200"
        echo ""
        echo "Picocom commands:"
        echo "  Ctrl+A Ctrl+X  - Exit picocom"
        echo "  Ctrl+A Ctrl+C  - Toggle local echo"
        echo "  Ctrl+A Ctrl+Q  - Quit without reset"
        echo ""
        echo "Press Ctrl+A Ctrl+X to exit monitoring"
        echo "======================================"
        picocom -b 115200 $SERIAL_PORT
    else
        echo "❌ No serial port found (expected /dev/ttyACM*)"
        echo "   Make sure the STM32 board is connected and recognized"
        exit 1
    fi
    
else
    echo "❌ No serial monitor found!"
    echo ""
    echo "Please install one of the following serial monitoring tools:"
    echo ""
    echo "1. probe-rs (Recommended - RTT support for high-speed logging):"
    echo "   cargo install probe-rs --features cli"
    echo ""
    echo "2. minicom (Popular terminal emulator):"
    echo "   sudo apt install minicom  # Ubuntu/Debian"
    echo "   brew install minicom      # macOS"
    echo ""
    echo "3. screen (Built-in on most systems):"
    echo "   sudo apt install screen   # Ubuntu/Debian (usually pre-installed)"
    echo "   # Usually pre-installed on macOS"
    echo ""
    echo "4. picocom (Lightweight serial terminal):"
    echo "   sudo apt install picocom  # Ubuntu/Debian"
    echo "   brew install picocom      # macOS"
    echo ""
    echo "For the best experience with detailed CRDT logging, use probe-rs with RTT!"
    echo ""
    echo "Alternative: Check serial output in your IDE or use a GUI tool like:"
    echo "  - Arduino IDE Serial Monitor"
    echo "  - PuTTY (Windows/Linux)"
    echo "  - CoolTerm (Cross-platform)"
    exit 1
fi

echo ""
echo "📊 Serial monitoring session ended"
