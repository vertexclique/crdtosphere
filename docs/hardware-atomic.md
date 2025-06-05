# Hardware Atomic Feature

The `hardware-atomic` feature enables atomic variants of CRDTs that support concurrent access from multiple threads or cores. This is particularly useful for multi-core embedded systems where multiple processing units need to coordinate through shared CRDTs.

## Overview

When the `hardware-atomic` feature is enabled, CRDTs use Rust's `core::sync::atomic` types internally, allowing safe concurrent access without requiring explicit locking mechanisms.

## Key Benefits

### Multi-Core Safety
- **Lock-free operations**: No mutexes or spinlocks required
- **Wait-free guarantees**: Operations complete in bounded time
- **ABA problem resistant**: Atomic operations prevent race conditions

### Real-Time Characteristics
- **Deterministic timing**: No blocking on lock contention
- **Lower latency**: Direct hardware atomic instructions
- **Interrupt safety**: Can be used from interrupt handlers

### Platform Support
- **ARM Cortex-M3+**: Hardware atomic support
- **AURIX TriCore**: Multi-core automotive processors
- **RISC-V**: With atomic extension (A)
- **x86/x64**: Full atomic support

## API Differences

### Standard Implementation
```rust
// Requires &mut self for modifications
let mut counter = GCounter::<DefaultConfig>::new(1);
counter.increment(5)?;
```

### Atomic Implementation
```rust
// Allows &self for modifications (concurrent access)
let counter = Arc::new(GCounter::<DefaultConfig>::new(1));
counter.increment(5)?; // Can be called from multiple threads
```

## Supported CRDTs

Currently, the following CRDTs support atomic operations:

### ✅ GCounter (Grow-only Counter)
- **Atomic operations**: `increment()`, `inc()`
- **Concurrent reads**: `value()`, `node_value()`
- **Thread-safe merging**: `merge()`

### 🚧 Planned Support
- **PNCounter**: Increment/decrement counter
- **LWWRegister**: Last-writer-wins register
- **Timestamps**: Atomic timestamp coordination

## Usage Examples

### Multi-Core Counter
```rust
use crdtosphere::prelude::*;
use std::sync::Arc;
use std::thread;

// Create shared atomic counter
let counter = Arc::new(GCounter::<DefaultConfig>::new(1));

// Spawn multiple threads
let handles: Vec<_> = (0..4).map(|_| {
    let counter_clone = Arc::clone(&counter);
    thread::spawn(move || {
        // Each thread can increment concurrently
        for _ in 0..1000 {
            counter_clone.increment(1).unwrap();
        }
    })
}).collect();

// Wait for completion
for handle in handles {
    handle.join().unwrap();
}

println!("Final value: {}", counter.value()); // 4000
```

### Interrupt Handler Usage
```rust
// In embedded context
static COUNTER: GCounter<EmbeddedConfig> = GCounter::new(1);

// From interrupt handler
#[interrupt]
fn timer_interrupt() {
    // Safe to call from interrupt context
    COUNTER.increment(1).unwrap();
}

// From main thread
fn main() {
    let value = COUNTER.value(); // Always safe to read
}
```

## Performance Characteristics

### Memory Usage
- **Same size**: Atomic variants use same memory as standard
- **Cache-friendly**: Atomic operations respect cache coherency
- **No overhead**: When not contended, performance is identical

### Operation Costs
| Operation | Standard | Atomic | Notes |
|-----------|----------|--------|-------|
| `increment()` | ~1 cycle | ~3-5 cycles | Hardware atomic add |
| `value()` | ~N cycles | ~N cycles | Sum of atomic loads |
| `merge()` | ~N cycles | ~N×10 cycles | Compare-exchange loops |

### Scalability
- **Linear scaling**: Performance scales with core count
- **No lock contention**: Wait-free operations
- **NUMA aware**: Respects memory locality

## Platform-Specific Notes

### ARM Cortex-M
```rust
// Cortex-M3+ supports 32-bit atomics
#[cfg(target_arch = "arm")]
use crdtosphere::prelude::*;

let counter = GCounter::<CortexMConfig>::new(1);
```

### AURIX TriCore
```rust
// Multi-core automotive processor
#[cfg(feature = "aurix")]
use crdtosphere::prelude::*;

let counter = GCounter::<AurixConfig>::new(1);
```

### RISC-V
```rust
// Requires atomic extension
#[cfg(all(target_arch = "riscv32", target_feature = "a"))]
use crdtosphere::prelude::*;

let counter = GCounter::<RiscVConfig>::new(1);
```

## Compilation

### Enable Feature
```toml
[dependencies]
crdtosphere = { version = "0.1", features = ["hardware-atomic"] }
```

### Platform Detection
The library automatically detects platform atomic support:
```rust
#[cfg(target_has_atomic = "32")]
// 32-bit atomics available

#[cfg(target_has_atomic = "64")]  
// 64-bit atomics available
```

### Fallback Behavior
If atomics are not available on the target platform, the library falls back to the standard implementation with appropriate compile-time warnings.

## Best Practices

### When to Use Atomic CRDTs
- ✅ Multi-core embedded systems
- ✅ Interrupt-driven coordination
- ✅ Real-time systems requiring deterministic timing
- ✅ Lock-free data structures

### When to Use Standard CRDTs
- ✅ Single-core microcontrollers
- ✅ Memory-constrained systems
- ✅ Simple coordination scenarios
- ✅ Maximum performance on single thread

### Design Guidelines
1. **Minimize contention**: Design to reduce concurrent access to same data
2. **Batch operations**: Group multiple increments when possible
3. **Read-heavy workloads**: Atomic reads are very efficient
4. **Avoid busy loops**: Use proper synchronization primitives

## Limitations

### Current Limitations
- **Limited CRDT support**: Only GCounter currently implemented
- **No persistence**: Atomic state is not automatically persisted
- **Platform dependent**: Requires hardware atomic support

### Future Enhancements
- **More CRDTs**: PNCounter, LWWRegister, etc.
- **Atomic persistence**: Integration with atomic storage
- **SIMD optimization**: Vector atomic operations
- **Custom memory ordering**: Fine-tuned performance

## Testing

### Concurrent Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    
    #[test]
    fn test_concurrent_increments() {
        let counter = Arc::new(GCounter::<DefaultConfig>::new(1));
        
        let handles: Vec<_> = (0..10).map(|_| {
            let counter = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..100 {
                    counter.increment(1).unwrap();
                }
            })
        }).collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        assert_eq!(counter.value(), 1000);
    }
}
```

### Stress Testing
```bash
# Run with multiple threads
cargo test --features hardware-atomic --release -- --test-threads=8

# Run example with timing
time cargo run --example atomic_counter --features hardware-atomic --release
```

## Troubleshooting

### Compilation Errors
```
error: target does not support atomic operations
```
**Solution**: Use a target with atomic support or disable the feature.

### Runtime Issues
```
// Unexpected values in concurrent scenarios
```
**Solution**: Ensure proper synchronization and avoid data races in application logic.

### Performance Issues
```
// Slower than expected performance
```
**Solution**: Profile for contention hotspots and consider batching operations.

## References

- [Rust Atomics and Locks](https://marabos.nl/atomics/)
- [ARM Cortex-M Programming Manual](https://developer.arm.com/documentation/)
- [RISC-V Atomic Extension](https://riscv.org/specifications/)
- [Intel Memory Ordering](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
