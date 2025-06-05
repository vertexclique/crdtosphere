#!/usr/bin/env python3
"""
CRDT State Output Parser and Converter

This script parses the hexadecimal CRDT state dump from the automotive ECU
simulation and converts values to human-readable units.
"""

import sys
import re
import struct

def hex_to_float(hex_str):
    """Convert IEEE 754 hex string to float"""
    try:
        hex_int = int(hex_str, 16)
        return struct.unpack('!f', struct.pack('!I', hex_int))[0]
    except (ValueError, struct.error):
        return None

def hex_to_int(hex_str):
    """Convert hex string to integer"""
    try:
        return int(hex_str, 16)
    except ValueError:
        return None

def parse_crdt_state(input_text):
    """Parse CRDT state dump and convert values"""
    
    # Look for CRDT State Dump section
    crdt_match = re.search(r'=== CRDT State Dump ===(.*?)=== End CRDT State Dump ===', 
                          input_text, re.DOTALL)
    
    if not crdt_match:
        return None
    
    crdt_section = crdt_match.group(1)
    
    # Parse each ECU section
    ecus = {}
    
    # Engine ECU
    engine_match = re.search(r'Engine ECU:\s*Temperature:\s*(0x[0-9A-Fa-f]+)\s*Error Count:\s*(0x[0-9A-Fa-f]+)\s*Config Time:\s*(0x[0-9A-Fa-f]+)\s*CAN Buffer:\s*(0x[0-9A-Fa-f]+)', crdt_section)
    if engine_match:
        temp = hex_to_float(engine_match.group(1))
        error_count = hex_to_int(engine_match.group(2))
        config_time = hex_to_int(engine_match.group(3))
        can_buffer = hex_to_int(engine_match.group(4))
        
        ecus['Engine ECU'] = {
            'temperature': temp,
            'error_count': error_count,
            'config_time': config_time,
            'can_buffer': can_buffer,
            'raw': {
                'temperature': engine_match.group(1),
                'error_count': engine_match.group(2),
                'config_time': engine_match.group(3),
                'can_buffer': engine_match.group(4)
            }
        }
    
    # Brake ECU
    brake_match = re.search(r'Brake ECU:\s*Temperature:\s*(0x[0-9A-Fa-f]+)\s*Error Count:\s*(0x[0-9A-Fa-f]+)\s*Emergency State:\s*(0x[0-9A-Fa-f]+)\s*Emergency Flag:\s*(0x[0-9A-Fa-f]+)', crdt_section)
    if brake_match:
        temp = hex_to_float(brake_match.group(1))
        error_count = hex_to_int(brake_match.group(2))
        emergency_state = hex_to_int(brake_match.group(3))
        emergency_flag = hex_to_int(brake_match.group(4))
        
        ecus['Brake ECU'] = {
            'temperature': temp,
            'error_count': error_count,
            'emergency_state': emergency_state,
            'emergency_flag': emergency_flag,
            'raw': {
                'temperature': brake_match.group(1),
                'error_count': brake_match.group(2),
                'emergency_state': brake_match.group(3),
                'emergency_flag': brake_match.group(4)
            }
        }
    
    # Steering ECU
    steering_match = re.search(r'Steering ECU:\s*Temperature:\s*(0x[0-9A-Fa-f]+)\s*Error Count:\s*(0x[0-9A-Fa-f]+)\s*CAN Buffer:\s*(0x[0-9A-Fa-f]+)', crdt_section)
    if steering_match:
        temp = hex_to_float(steering_match.group(1))
        error_count = hex_to_int(steering_match.group(2))
        can_buffer = hex_to_int(steering_match.group(3))
        
        ecus['Steering ECU'] = {
            'temperature': temp,
            'error_count': error_count,
            'can_buffer': can_buffer,
            'raw': {
                'temperature': steering_match.group(1),
                'error_count': steering_match.group(2),
                'can_buffer': steering_match.group(3)
            }
        }
    
    # Gateway ECU
    gateway_match = re.search(r'Gateway ECU:\s*Temperature:\s*(0x[0-9A-Fa-f]+)\s*Health Score:\s*(0x[0-9A-Fa-f]+)\s*Routing Count:\s*(0x[0-9A-Fa-f]+)\s*CAN Buffer:\s*(0x[0-9A-Fa-f]+)', crdt_section)
    if gateway_match:
        temp = hex_to_float(gateway_match.group(1))
        health_score = hex_to_int(gateway_match.group(2))
        routing_count = hex_to_int(gateway_match.group(3))
        can_buffer = hex_to_int(gateway_match.group(4))
        
        ecus['Gateway ECU'] = {
            'temperature': temp,
            'health_score': health_score,
            'routing_count': routing_count,
            'can_buffer': can_buffer,
            'raw': {
                'temperature': gateway_match.group(1),
                'health_score': gateway_match.group(2),
                'routing_count': gateway_match.group(3),
                'can_buffer': gateway_match.group(4)
            }
        }
    
    return ecus

def format_output(ecus):
    """Format the parsed ECU data for display"""
    if not ecus:
        return "No CRDT state data found in input"
    
    output = []
    output.append("=" * 60)
    output.append("CRDT STATE ANALYSIS")
    output.append("=" * 60)
    
    for ecu_name, data in ecus.items():
        output.append(f"\n{ecu_name}:")
        output.append("-" * (len(ecu_name) + 1))
        
        # Temperature (always present)
        if data['temperature'] is not None:
            temp_c = data['temperature']
            temp_f = temp_c * 9/5 + 32
            output.append(f"  Temperature:    {temp_c:6.2f}°C ({temp_f:6.2f}°F) [{data['raw']['temperature']}]")
        
        # ECU-specific fields
        if 'error_count' in data:
            output.append(f"  Error Count:    {data['error_count']:,} [{data['raw']['error_count']}]")
        
        if 'emergency_state' in data:
            emergency_active = "ACTIVE" if data['emergency_state'] != 0 else "INACTIVE"
            output.append(f"  Emergency:      {emergency_active} [{data['raw']['emergency_state']}]")
            
        if 'emergency_flag' in data:
            flag_status = "SET" if data['emergency_flag'] != 0 else "CLEAR"
            output.append(f"  Emergency Flag: {flag_status} [{data['raw']['emergency_flag']}]")
        
        if 'health_score' in data:
            output.append(f"  Health Score:   {data['health_score']:,} [{data['raw']['health_score']}]")
            
        if 'routing_count' in data:
            output.append(f"  Routing Count:  {data['routing_count']:,} [{data['raw']['routing_count']}]")
        
        if 'config_time' in data:
            output.append(f"  Config Time:    {data['config_time']:,} [{data['raw']['config_time']}]")
        
        if 'can_buffer' in data:
            output.append(f"  CAN Buffer:     {data['can_buffer']:,} [{data['raw']['can_buffer']}]")
    
    # Summary analysis
    output.append("\n" + "=" * 60)
    output.append("SYSTEM ANALYSIS")
    output.append("=" * 60)
    
    # Temperature analysis
    temps = [data['temperature'] for data in ecus.values() if data['temperature'] is not None]
    if temps:
        avg_temp = sum(temps) / len(temps)
        max_temp = max(temps)
        min_temp = min(temps)
        
        output.append(f"Average Temperature: {avg_temp:.2f}°C")
        output.append(f"Temperature Range:   {min_temp:.2f}°C - {max_temp:.2f}°C")
        
        # Temperature warnings
        if max_temp > 100:
            output.append("⚠️  WARNING: High temperature detected!")
        if max_temp > 110:
            output.append("🚨 CRITICAL: Overheating condition!")
    
    # Emergency status
    emergency_active = False
    if 'Brake ECU' in ecus:
        brake_data = ecus['Brake ECU']
        if brake_data.get('emergency_state', 0) != 0 or brake_data.get('emergency_flag', 0) != 0:
            emergency_active = True
    
    if emergency_active:
        output.append("🚨 EMERGENCY: Emergency braking system active!")
    else:
        output.append("✅ NORMAL: No emergency conditions detected")
    
    return "\n".join(output)

def main():
    """Main function"""
    if len(sys.argv) > 1:
        # Read from file
        try:
            with open(sys.argv[1], 'r') as f:
                input_text = f.read()
        except FileNotFoundError:
            print(f"Error: File '{sys.argv[1]}' not found", file=sys.stderr)
            sys.exit(1)
    else:
        # Read from stdin
        input_text = sys.stdin.read()
    
    # Parse the CRDT state
    ecus = parse_crdt_state(input_text)
    
    # Format and print output
    result = format_output(ecus)
    print(result)
    
    # Exit with appropriate code
    if ecus:
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
