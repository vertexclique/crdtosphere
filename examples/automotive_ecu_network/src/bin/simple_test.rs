//! Simple test binary for Renode simulation
//! This is a minimal binary to test the platform configuration

#![no_std]
#![no_main]

use panic_halt as _;
use cortex_m_rt::entry;
use stm32f4xx_hal as _; // Provides interrupt vectors

// Simple global variable to prevent optimization
static mut COUNTER: u32 = 0;

#[entry]
fn main() -> ! {
    // Simple loop that does something observable
    loop {
        unsafe {
            COUNTER = COUNTER.wrapping_add(1);
            // Write to memory to make it observable
            core::ptr::write_volatile(0x20000000 as *mut u32, COUNTER);
        }
        
        // Simple delay
        for _ in 0..1000 {
            cortex_m::asm::nop();
        }
    }
}

#[no_mangle]
pub extern "C" fn get_counter() -> u32 {
    unsafe { COUNTER }
}
