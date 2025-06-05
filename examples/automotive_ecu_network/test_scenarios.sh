#!/bin/bash

# Automotive ECU Network Test Scenarios Script
# This script provides easy testing of different automotive scenarios using Renode

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RENODE_SCRIPT="$SCRIPT_DIR/renode/simulation.resc"
BUILD_TARGET="thumbv7em-none-eabihf"
RENODE_LOG_FILE="/tmp/automotive_ecu_test.log"

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check prerequisites
check_prerequisites() {
    print_info "Checking prerequisites..."
    
    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo (Rust) is not installed. Please install Rust toolchain."
        exit 1
    fi
    
    # Check if target is installed
    if ! rustup target list --installed | grep -q "$BUILD_TARGET"; then
        print_warning "Target $BUILD_TARGET not installed. Installing..."
        rustup target add "$BUILD_TARGET"
    fi
    
    # Check if Renode is installed
    if ! command -v renode &> /dev/null; then
        print_error "Renode is not installed. Please install Renode simulation framework."
        print_info "Visit: https://renode.io for installation instructions"
        exit 1
    fi
    
    print_success "All prerequisites satisfied"
}

# Function to build the project
build_project() {
    print_info "Building automotive ECU network example..."
    
    cd "$SCRIPT_DIR"
    
    if cargo build --release --target "$BUILD_TARGET"; then
        print_success "Build completed successfully"
    else
        print_error "Build failed"
        exit 1
    fi
}

# Function to run a specific scenario
run_scenario() {
    local scenario="$1"
    local duration="${2:-10}"
    
    print_info "Running scenario: $scenario (duration: ${duration}s)"
    
    # Map scenario names to files
    local scenario_file=""
    case "$scenario" in
        "normal"|"start_normal_operation")
            scenario_file="$SCRIPT_DIR/scenarios/normal_operation.resc"
            ;;
        "emergency"|"emergency_brake_scenario")
            scenario_file="$SCRIPT_DIR/scenarios/emergency_brake.resc"
            ;;
        "fusion"|"sensor_fusion_scenario")
            scenario_file="$SCRIPT_DIR/scenarios/sensor_fusion.resc"
            ;;
        "failure"|"ecu_failure_scenario")
            scenario_file="$SCRIPT_DIR/scenarios/ecu_failure.resc"
            ;;
        *)
            print_error "Unknown scenario: $scenario"
            return 1
            ;;
    esac
    
    if [[ ! -f "$scenario_file" ]]; then
        print_error "Scenario file not found: $scenario_file"
        return 1
    fi
    
    # Create temporary Renode script for the specific scenario
    local temp_script="/tmp/automotive_scenario_${scenario}.resc"
    
    cat > "$temp_script" << EOF
# Load base simulation first
include @$RENODE_SCRIPT

# Now load the specific scenario
include @$scenario_file

# Wait for scenario initialization
echo "Scenario initialized. Running for ${duration} seconds..."

# Pause all machines to ensure we can control timing
mach set "engine_ecu"; pause
mach set "brake_ecu"; pause  
mach set "steering_ecu"; pause
mach set "gateway_ecu"; pause

# Resume all machines and run for the specified duration
mach set "engine_ecu"; start
mach set "brake_ecu"; start
mach set "steering_ecu"; start
mach set "gateway_ecu"; start

# Let simulation run for the specified duration
echo "Running simulation for ${duration} seconds..."
sleep $duration

# Pause all machines to capture final state
mach set "engine_ecu"; pause
mach set "brake_ecu"; pause
mach set "steering_ecu"; pause
mach set "gateway_ecu"; pause

# Show final status
echo ""
echo "=== Scenario '$scenario' completed after ${duration}s ==="
echo ""

# Display CRDT state manually since macro might not be available
echo "=== CRDT State Dump ==="
echo ""

# Engine ECU CRDT State (read from output regions)
mach set "engine_ecu"
echo "Engine ECU:"
echo "  Temperature: "
sysbus ReadDoubleWord 0x50001000
echo "  Error Count: "
sysbus ReadDoubleWord 0x50001300
echo "  Config Time: "
sysbus ReadDoubleWord 0x50001204
echo "  CAN Buffer: "
sysbus ReadDoubleWord 0x50002000
echo ""

# Brake ECU CRDT State (read from output regions)
mach set "brake_ecu"
echo "Brake ECU:"
echo "  Temperature: "
sysbus ReadDoubleWord 0x50001000
echo "  Error Count: "
sysbus ReadDoubleWord 0x50001300
echo "  Emergency State: "
sysbus ReadDoubleWord 0x50001100
echo "  Emergency Flag: "
sysbus ReadDoubleWord 0x50001104
echo ""

# Steering ECU CRDT State (read from output regions)
mach set "steering_ecu" 
echo "Steering ECU:"
echo "  Temperature: "
sysbus ReadDoubleWord 0x50001000
echo "  Error Count: "
sysbus ReadDoubleWord 0x50001300
echo "  CAN Buffer: "
sysbus ReadDoubleWord 0x40006C00
echo ""

# Gateway ECU CRDT State (read from output regions)
mach set "gateway_ecu"
echo "Gateway ECU:"
echo "  Temperature: "
sysbus ReadDoubleWord 0x50001000
echo "  Health Score: "
sysbus ReadDoubleWord 0x50001300
echo "  Routing Count: "
sysbus ReadDoubleWord 0x50001204
echo "  CAN Buffer: "
sysbus ReadDoubleWord 0x40006C00
echo ""

echo "=== End CRDT State Dump ==="

echo ""
echo "Scenario execution completed successfully"

# Exit Renode
quit
EOF

    # Run Renode with the scenario
    if timeout $((duration + 30)) renode --console -e "include @$temp_script" | tee "$RENODE_LOG_FILE"; then
        print_success "Scenario '$scenario' completed successfully"
        
        # Show relevant log output and parse CRDT state
        if [[ -f "$RENODE_LOG_FILE" ]]; then
            print_info "Key simulation results:"
            echo "----------------------------------------"
            
            # Check if CRDT state dump exists and parse it
            if rg -q "=== CRDT State Dump ===" "$RENODE_LOG_FILE"; then
                print_info "Parsing CRDT state data..."
                echo ""
                
                # Parse CRDT output with Python script
                if python3 "$SCRIPT_DIR/parse_crdt_output.py" "$RENODE_LOG_FILE"; then
                    echo ""
                    print_success "CRDT state analysis completed"
                else
                    print_warning "CRDT state parsing failed, showing raw output:"
                    echo ""
                    # Fallback to raw output
                    rg -A 50 "=== CRDT State Dump ===" "$RENODE_LOG_FILE" | head -60 || true
                fi
            else
                print_warning "No CRDT state dump found in output"
                # Show general simulation output
                tail -20 "$RENODE_LOG_FILE" || true
            fi
            
            echo "----------------------------------------"
        fi
    else
        print_error "Scenario '$scenario' failed or timed out"
        print_info "Check log file: $RENODE_LOG_FILE"
        return 1
    fi
    
    # Clean up
    rm -f "$temp_script"
}

# Function to run interactive mode
run_interactive() {
    print_info "Starting interactive Renode session..."
    print_info "Available scenarios:"
    print_info "  - start_normal_operation"
    print_info "  - emergency_brake_scenario"
    print_info "  - sensor_fusion_scenario"
    print_info "  - network_partition_scenario"
    print_info "  - ecu_failure_scenario"
    print_info "  - performance_test"
    print_info "  - safety_validation_test"
    print_info ""
    print_info "Type 'quit' to exit Renode"
    
    renode --console -e "include @$RENODE_SCRIPT"
}

# Function to run all scenarios
run_all_scenarios() {
    local duration="${1:-5}"
    
    print_info "Running all automotive scenarios (${duration}s each)..."
    
    local scenarios=(
        "start_normal_operation"
        "emergency_brake_scenario"
        "sensor_fusion_scenario"
        "network_partition_scenario"
        "ecu_failure_scenario"
        "performance_test"
        "safety_validation_test"
    )
    
    for scenario in "${scenarios[@]}"; do
        print_info "Running scenario: $scenario"
        run_scenario "$scenario" "$duration"
        sleep 1
    done
    
    print_success "All scenarios completed successfully"
}

# Function to show usage
show_usage() {
    echo "Automotive ECU Network Test Scenarios"
    echo ""
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo ""
    echo "Commands:"
    echo "  build                           Build the project"
    echo "  normal [duration]              Run normal operation scenario"
    echo "  emergency [duration]           Run emergency brake scenario"
    echo "  fusion [duration]              Run sensor fusion scenario"
    echo "  partition [duration]           Run network partition scenario"
    echo "  failure [duration]             Run ECU failure scenario"
    echo "  performance [duration]         Run performance test"
    echo "  safety [duration]              Run safety validation test"
    echo "  all [duration]                 Run all scenarios"
    echo "  interactive                    Start interactive Renode session"
    echo "  clean                          Clean build artifacts"
    echo "  help                           Show this help message"
    echo ""
    echo "Options:"
    echo "  duration                       Simulation duration in seconds (default: 10)"
    echo ""
    echo "Examples:"
    echo "  $0 build                       # Build the project"
    echo "  $0 emergency 15                # Run emergency scenario for 15 seconds"
    echo "  $0 all 5                       # Run all scenarios for 5 seconds each"
    echo "  $0 interactive                 # Start interactive session"
    echo ""
    echo "Prerequisites:"
    echo "  - Rust toolchain with thumbv7em-none-eabihf target"
    echo "  - Renode simulation framework"
    echo ""
}

# Function to clean build artifacts
clean_build() {
    print_info "Cleaning build artifacts..."
    cd "$SCRIPT_DIR"
    cargo clean
    rm -f "$RENODE_LOG_FILE"
    print_success "Clean completed"
}

# Function to validate scenario name
validate_scenario() {
    local scenario="$1"
    local valid_scenarios=(
        "start_normal_operation"
        "emergency_brake_scenario"
        "sensor_fusion_scenario"
        "network_partition_scenario"
        "ecu_failure_scenario"
        "performance_test"
        "safety_validation_test"
    )
    
    for valid in "${valid_scenarios[@]}"; do
        if [[ "$scenario" == "$valid" ]]; then
            return 0
        fi
    done
    
    return 1
}

# Main script logic
main() {
    local command="${1:-help}"
    local duration="${2:-10}"
    
    case "$command" in
        "build")
            check_prerequisites
            build_project
            ;;
        "normal")
            check_prerequisites
            build_project
            run_scenario "start_normal_operation" "$duration"
            ;;
        "emergency")
            check_prerequisites
            build_project
            run_scenario "emergency_brake_scenario" "$duration"
            ;;
        "fusion")
            check_prerequisites
            build_project
            run_scenario "sensor_fusion_scenario" "$duration"
            ;;
        "partition")
            check_prerequisites
            build_project
            run_scenario "network_partition_scenario" "$duration"
            ;;
        "failure")
            check_prerequisites
            build_project
            run_scenario "ecu_failure_scenario" "$duration"
            ;;
        "performance")
            check_prerequisites
            build_project
            run_scenario "performance_test" "$duration"
            ;;
        "safety")
            check_prerequisites
            build_project
            run_scenario "safety_validation_test" "$duration"
            ;;
        "all")
            check_prerequisites
            build_project
            run_all_scenarios "$duration"
            ;;
        "interactive")
            check_prerequisites
            build_project
            run_interactive
            ;;
        "clean")
            clean_build
            ;;
        "help"|"-h"|"--help")
            show_usage
            ;;
        *)
            print_error "Unknown command: $command"
            echo ""
            show_usage
            exit 1
            ;;
    esac
}

# Trap to clean up on exit
trap 'rm -f /tmp/automotive_scenario_*.resc' EXIT

# Run main function with all arguments
main "$@"
