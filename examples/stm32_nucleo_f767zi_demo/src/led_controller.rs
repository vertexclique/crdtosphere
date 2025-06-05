//! LED Controller for STM32 NUCLEO-F767ZI
//! 
//! Controls the three user LEDs to indicate CRDT operations:
//! - LED1 (Green, PB0): Insert operations
//! - LED2 (Blue, PB7): Delete operations  
//! - LED3 (Red, PB14): Merge operations

use stm32f7xx_hal::{
    gpio::{Output, PushPull, gpiob::{PB0, PB7, PB14}},
    prelude::*,
};
use embedded_hal::digital::v2::OutputPin;
use defmt::info;

pub struct LedController {
    led1_green: PB0<Output<PushPull>>,   // Insert operations
    led2_blue: PB7<Output<PushPull>>,    // Delete operations
    led3_red: PB14<Output<PushPull>>,    // Merge operations
}

#[derive(Debug, Clone, Copy)]
pub enum LedOperation {
    Insert,
    Delete,
    Merge,
}

#[derive(Debug, Clone, Copy)]
pub enum BlinkPattern {
    Single,
    Double,
    Triple,
    Solid(u32),
}

impl LedController {
    pub fn new(
        led1_green: PB0<Output<PushPull>>,
        led2_blue: PB7<Output<PushPull>>,
        led3_red: PB14<Output<PushPull>>,
    ) -> Self {
        let mut controller = Self {
            led1_green,
            led2_blue,
            led3_red,
        };
        
        // Initialize all LEDs to off
        controller.all_off();
        
        controller
    }
    
    /// Turn all LEDs off
    pub fn all_off(&mut self) {
        let _ = self.led1_green.set_low();
        let _ = self.led2_blue.set_low();
        let _ = self.led3_red.set_low();
    }
    
    /// Turn all LEDs on (for testing)
    pub fn all_on(&mut self) {
        let _ = self.led1_green.set_high();
        let _ = self.led2_blue.set_high();
        let _ = self.led3_red.set_high();
    }
    
    /// Indicate a CRDT operation with appropriate LED
    pub fn indicate_operation(&mut self, operation: LedOperation, pattern: BlinkPattern) {
        info!("LED: Indicating operation");
        
        match operation {
            LedOperation::Insert => self.blink_led1(pattern),
            LedOperation::Delete => self.blink_led2(pattern),
            LedOperation::Merge => self.blink_led3(pattern),
        }
    }
    
    /// Blink LED1 (Green) for insert operations
    fn blink_led1(&mut self, pattern: BlinkPattern) {
        match pattern {
            BlinkPattern::Single => {
                let _ = self.led1_green.set_high();
                self.delay_ms(100);
                let _ = self.led1_green.set_low();
            }
            BlinkPattern::Double => {
                for _ in 0..2 {
                    let _ = self.led1_green.set_high();
                    self.delay_ms(100);
                    let _ = self.led1_green.set_low();
                    self.delay_ms(100);
                }
            }
            BlinkPattern::Triple => {
                for _ in 0..3 {
                    let _ = self.led1_green.set_high();
                    self.delay_ms(100);
                    let _ = self.led1_green.set_low();
                    self.delay_ms(100);
                }
            }
            BlinkPattern::Solid(duration_ms) => {
                let _ = self.led1_green.set_high();
                self.delay_ms(duration_ms);
                let _ = self.led1_green.set_low();
            }
        }
    }
    
    /// Blink LED2 (Blue) for delete operations
    fn blink_led2(&mut self, pattern: BlinkPattern) {
        match pattern {
            BlinkPattern::Single => {
                let _ = self.led2_blue.set_high();
                self.delay_ms(100);
                let _ = self.led2_blue.set_low();
            }
            BlinkPattern::Double => {
                for _ in 0..2 {
                    let _ = self.led2_blue.set_high();
                    self.delay_ms(100);
                    let _ = self.led2_blue.set_low();
                    self.delay_ms(100);
                }
            }
            BlinkPattern::Triple => {
                for _ in 0..3 {
                    let _ = self.led2_blue.set_high();
                    self.delay_ms(100);
                    let _ = self.led2_blue.set_low();
                    self.delay_ms(100);
                }
            }
            BlinkPattern::Solid(duration_ms) => {
                let _ = self.led2_blue.set_high();
                self.delay_ms(duration_ms);
                let _ = self.led2_blue.set_low();
            }
        }
    }
    
    /// Blink LED3 (Red) for merge operations
    fn blink_led3(&mut self, pattern: BlinkPattern) {
        match pattern {
            BlinkPattern::Single => {
                let _ = self.led3_red.set_high();
                self.delay_ms(100);
                let _ = self.led3_red.set_low();
            }
            BlinkPattern::Double => {
                for _ in 0..2 {
                    let _ = self.led3_red.set_high();
                    self.delay_ms(100);
                    let _ = self.led3_red.set_low();
                    self.delay_ms(100);
                }
            }
            BlinkPattern::Triple => {
                for _ in 0..3 {
                    let _ = self.led3_red.set_high();
                    self.delay_ms(100);
                    let _ = self.led3_red.set_low();
                    self.delay_ms(100);
                }
            }
            BlinkPattern::Solid(duration_ms) => {
                let _ = self.led3_red.set_high();
                self.delay_ms(duration_ms);
                let _ = self.led3_red.set_low();
            }
        }
    }
    
    /// Show startup sequence
    pub fn startup_sequence(&mut self) {
        info!("LED: Starting startup sequence");
        
        // Light up each LED in sequence
        let _ = self.led1_green.set_high();
        self.delay_ms(200);
        let _ = self.led2_blue.set_high();
        self.delay_ms(200);
        let _ = self.led3_red.set_high();
        self.delay_ms(200);
        
        // Turn all off
        self.all_off();
        self.delay_ms(200);
        
        // Quick flash all
        self.all_on();
        self.delay_ms(100);
        self.all_off();
        
        info!("LED: Startup sequence complete");
    }
    
    /// Show error pattern (all LEDs blinking rapidly)
    pub fn error_pattern(&mut self) {
        info!("LED: Showing error pattern");
        
        for _ in 0..5 {
            self.all_on();
            self.delay_ms(100);
            self.all_off();
            self.delay_ms(100);
        }
    }
    
    /// Show success pattern (green LED solid for 1 second)
    pub fn success_pattern(&mut self) {
        info!("LED: Showing success pattern");
        
        let _ = self.led1_green.set_high();
        self.delay_ms(1000);
        let _ = self.led1_green.set_low();
    }
    
    /// Simple delay implementation using busy wait
    /// Note: In a real application, you'd use a proper timer
    fn delay_ms(&self, ms: u32) {
        // Much more reasonable delay - approximately 1000 cycles per ms
        // This gives a good balance between accuracy and responsiveness
        let cycles_per_ms = 1000;
        for _ in 0..(ms * cycles_per_ms) {
            cortex_m::asm::nop();
        }
    }
}

/// LED operation tracking for statistics
pub struct LedStats {
    pub insert_count: u32,
    pub delete_count: u32,
    pub merge_count: u32,
}

impl LedStats {
    pub fn new() -> Self {
        Self {
            insert_count: 0,
            delete_count: 0,
            merge_count: 0,
        }
    }
    
    pub fn record_operation(&mut self, operation: LedOperation) {
        match operation {
            LedOperation::Insert => self.insert_count += 1,
            LedOperation::Delete => self.delete_count += 1,
            LedOperation::Merge => self.merge_count += 1,
        }
    }
    
    pub fn total_operations(&self) -> u32 {
        self.insert_count + self.delete_count + self.merge_count
    }
    
    pub fn reset(&mut self) {
        self.insert_count = 0;
        self.delete_count = 0;
        self.merge_count = 0;
    }
}
