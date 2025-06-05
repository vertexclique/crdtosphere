#![no_std]
#![no_main]

//! STM32 NUCLEO-F767ZI CRDTosphere Demonstration
//! 
//! This demo showcases CRDTosphere library capabilities using the three user LEDs
//! to indicate different CRDT operations:
//! 
//! - LED1 (Green, PB0): Insert operations (add elements, increment counters)
//! - LED2 (Blue, PB7): Delete operations (remove elements, tombstones)
//! - LED3 (Red, PB14): Merge operations (synchronize between nodes)
//! 
//! The user button (PC13) can be pressed to trigger manual operations.

// use panic_halt as _;  // We'll define our own panic handler
use cortex_m_rt::entry;
use stm32f7xx_hal::{
    pac,
    prelude::*,
    rcc::{HSEClock, HSEClockMode},
};
use embedded_hal::digital::v2::InputPin;
use defmt::{info, error};
use defmt_rtt as _;
use cortex_m::peripheral::syst::SystClkSource;

// Provide defmt timestamp (required by defmt)
defmt::timestamp!("{=u64}", {
    // Simple counter-based timestamp for demo purposes
    static mut TIMESTAMP: u64 = 0;
    unsafe {
        TIMESTAMP += 1;
        TIMESTAMP
    }
});

// Critical section implementation (required for embedded)
use critical_section::RawRestoreState;

struct CriticalSection;
critical_section::set_impl!(CriticalSection);

unsafe impl critical_section::Impl for CriticalSection {
    unsafe fn acquire() -> RawRestoreState {
        let primask = cortex_m::register::primask::read();
        cortex_m::interrupt::disable();
        primask.is_active() as u8
    }

    unsafe fn release(was_active: RawRestoreState) {
        if was_active != 0 {
            cortex_m::interrupt::enable();
        }
    }
}

mod led_controller;
mod crdt_demo;

use led_controller::{LedController, LedOperation, BlinkPattern};
use crdt_demo::{CrdtDemo, validate_crdt_properties};

// Global millisecond counter for accurate timing
static mut MILLIS: u32 = 0;

/// Get current milliseconds since startup
fn get_millis() -> u32 {
    unsafe { MILLIS }
}

/// SysTick interrupt handler - increments millisecond counter
#[cortex_m_rt::exception]
fn SysTick() {
    unsafe {
        MILLIS = MILLIS.wrapping_add(1);
    }
}

#[entry]
fn main() -> ! {
    info!("🚀 STM32 NUCLEO-F767ZI CRDTosphere Demo Starting");
    
    // Initialize the hardware
    let dp = pac::Peripherals::take().unwrap();
    let mut cp = cortex_m::Peripherals::take().unwrap();
    
    // Configure clocks - use default configuration for simplicity
    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.freeze();
    
    // Configure SysTick for 1ms interrupts
    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(clocks.sysclk().to_Hz() / 1000 - 1); // 1ms intervals
    syst.clear_current();
    syst.enable_counter();
    syst.enable_interrupt();
    
    info!("Clock configuration: Using default clocks with SysTick at 1ms");
    
    // Configure GPIO ports
    let gpiob = dp.GPIOB.split();
    let gpioc = dp.GPIOC.split();
    
    // Configure LEDs (active high)
    let led1_green = gpiob.pb0.into_push_pull_output();  // LD1 - Green
    let led2_blue = gpiob.pb7.into_push_pull_output();   // LD2 - Blue  
    let led3_red = gpiob.pb14.into_push_pull_output();   // LD3 - Red
    
    // Configure user button (active low with pull-up)
    let user_button = gpioc.pc13.into_pull_up_input();
    
    // Initialize LED controller
    let mut led_controller = LedController::new(led1_green, led2_blue, led3_red);
    
    info!("Hardware initialization complete");
    
    // Show startup sequence
    led_controller.startup_sequence();
    
    // Initialize CRDT demo
    let mut crdt_demo = match CrdtDemo::new() {
        Ok(demo) => {
            info!("✅ CRDT demo initialized successfully");
            demo
        }
        Err(_e) => {
            error!("❌ Failed to initialize CRDT demo");
            led_controller.error_pattern();
            loop {
                cortex_m::asm::wfi(); // Wait for interrupt (low power)
            }
        }
    };
    
    // Validate CRDT mathematical properties
    if let Err(e) = validate_crdt_properties() {
        error!("❌ CRDT property validation failed");
        // Print error details based on error type
        match e {
            crdtosphere::error::CRDTError::InvalidMerge => error!("Error type: InvalidMerge"),
            crdtosphere::error::CRDTError::OutOfMemory => error!("Error type: OutOfMemory"),
            crdtosphere::error::CRDTError::NodeCountExceeded => error!("Error type: NodeCountExceeded"),
            crdtosphere::error::CRDTError::InvalidOperation => error!("Error type: InvalidOperation"),
            crdtosphere::error::CRDTError::InvalidState => error!("Error type: InvalidState"),
            crdtosphere::error::CRDTError::ClockSkew => error!("Error type: ClockSkew"),
            crdtosphere::error::CRDTError::CausalityViolation => error!("Error type: CausalityViolation"),
            _ => error!("Error type: Other"),
        }
        led_controller.error_pattern();
        loop {
            cortex_m::asm::wfi();
        }
    }
    
    info!("✅ CRDT properties validated");
    
    // Show initial state
    crdt_demo.show_current_state();
    
    // Button debouncing state
    let mut button_pressed = false;
    let mut button_count = 0u32;
    
    info!("🎯 Demo ready! Press user button to interact or wait for auto demo");
    info!("LED Indicators:");
    info!("  🟢 Green (LED1): Insert operations");
    info!("  🔵 Blue (LED2): Delete operations");
    info!("  🔴 Red (LED3): Merge operations");
    
    // Main demo loop with proper timing
    let mut demo_cycle = 0u32;
    let mut last_demo_time = get_millis();
    
    loop {
        // Check user button with debouncing
        let button_state = user_button.is_low();
        
        if button_state && !button_pressed {
            // Button just pressed
            button_pressed = true;
            button_count += 1;
            
            info!("👆 User button pressed (count: {})", button_count);
            
            // Handle button press with CRDT
            if let Err(_e) = crdt_demo.handle_button_press(&mut led_controller) {
                error!("❌ Button press handling failed");
                led_controller.error_pattern();
            }
            
            // Show current state after button press
            crdt_demo.show_current_state();
            
        } else if !button_state && button_pressed {
            // Button released
            button_pressed = false;
        }
        
        // Auto demo every 10 seconds using accurate timing
        let current_time = get_millis();
        // Do it every 5 seconds. Sigh...
        if current_time.wrapping_sub(last_demo_time) >= 5_000 { // 5 seconds
            last_demo_time = current_time;
            demo_cycle += 1;
            
            info!("🤖 Starting automated demo cycle {}", demo_cycle);
            
            match demo_cycle % 4 {
                0 => {
                    // Full demo sequence
                    if let Err(_e) = crdt_demo.run_demo_sequence(&mut led_controller) {
                        error!("❌ Demo sequence failed");
                        led_controller.error_pattern();
                    }
                }
                1 => {
                    // Add some devices
                    info!("🔧 Adding devices to registry");
                    for device_id in [50, 51, 52] {
                        if let Err(_e) = crdt_demo.add_device(device_id, &mut led_controller) {
                            error!("❌ Failed to add device {}", device_id);
                        }
                        delay_ms(200);
                    }
                }
                2 => {
                    // Remove some devices
                    info!("🗑️ Removing devices from registry");
                    for device_id in [50, 51] {
                        if let Err(_e) = crdt_demo.remove_device(device_id, &mut led_controller) {
                            error!("❌ Failed to remove device {}", device_id);
                        }
                        delay_ms(200);
                    }
                }
                3 => {
                    // Simulate network activity
                    info!("🌐 Simulating network merge");
                    if let Err(_e) = crdt_demo.simulate_node_merge(&mut led_controller) {
                        error!("❌ Network merge failed");
                        led_controller.error_pattern();
                    }
                }
                _ => unreachable!(),
            }
            
            // Show statistics
            let stats = crdt_demo.get_led_stats();
            info!("📊 Operation Statistics:");
            info!("  Insert: {}, Delete: {}, Merge: {}, Total: {}", 
                  stats.insert_count, stats.delete_count, stats.merge_count, stats.total_operations());
        }
        
        // Small delay to prevent busy waiting
        delay_ms(1);
    }
}

/// Simple delay function
fn delay_ms(ms: u32) {
    // Much more reasonable delay - approximately 1000 cycles per ms
    // This gives a good balance between accuracy and responsiveness
    let cycles_per_ms = 1000;
    for _ in 0..(ms * cycles_per_ms) {
        cortex_m::asm::nop();
    }
}

/// Panic handler - blink all LEDs rapidly
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    // Try to get GPIO access for emergency LED signaling
    if let Some(dp) = pac::Peripherals::take() {
        let gpiob = dp.GPIOB.split();
        let mut led1 = gpiob.pb0.into_push_pull_output();
        let mut led2 = gpiob.pb7.into_push_pull_output();
        let mut led3 = gpiob.pb14.into_push_pull_output();
        
        // Rapid blinking to indicate panic
        loop {
            let _ = led1.set_high();
            let _ = led2.set_high();
            let _ = led3.set_high();
            delay_ms(100);
            let _ = led1.set_low();
            let _ = led2.set_low();
            let _ = led3.set_low();
            delay_ms(100);
        }
    } else {
        // Fallback - just halt
        loop {
            cortex_m::asm::wfi();
        }
    }
}

/// Hard fault handler
#[cortex_m_rt::exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    panic!("Hard fault occurred");
}

/// Default exception handler
#[cortex_m_rt::exception]
unsafe fn DefaultHandler(_irqn: i16) {
    // Note: warn! might not work in exception context, but we'll keep it simple
}
