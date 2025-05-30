#![feature(cell_update)]
#![no_std]
#![no_main]
extern crate alloc;

mod lcd1602;
mod gsearch;
mod mempad;
mod bitops;
mod datmgt;
mod hash;

use crate::lcd1602::{HD44780Util, I2CLcd1602, MarqueStyle, ParallelLcd1602};
use alloc::{format, vec};
use alloc::vec::Vec;
use arduino_hal::Eeprom;
use arduino_hal::i2c::Direction;
use arduino_hal::port::mode::Output;
use arduino_hal::port::Pin;
use arduino_hal::prelude::_unwrap_infallible_UnwrapInfallible;
use embedded_alloc::LlffHeap as Heap;
use embedded_hal::digital::OutputPin;
use embedded_hal::i2c::I2c;
use panic_halt as _;
use ufmt::uwriteln;
use crate::datmgt::EntryManager;
use crate::gsearch::two_opt;

// use panic_halt as _;
#[global_allocator]
static HEAP: Heap = Heap::empty();

//include!(concat!(env!("OUT_DIR"), "/codegen.rs"));


const ROOM_DICT: [&str; 10] = ["Dropoff", "G010", "Veranda", "I315", "B888", "C148", "C024", "Atrium", "Y249", "F012"];

// ** Adapted from HTTP status codes courtesy of https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status **
// 100-199 = informational
// 200-299 = successful
// 300-399 = redirection
// 400-499 = client err
// 500-599 = server err

// Distilled into...
// 0 = N/A
// 1-9 = informational
// 10-19 = successful
// 20-39 = sender err
// 40-59 = recipient err


enum DeliveryStatus {
    OK = 10,                 // successful delivery
    Failed = 20,             // fully failed delivery (within TTD)
    Absent = 40,            // delivered but recipient missing
    Postponed = 1,          // delivery postponed (canceled and renewed as new delivery)
    Refused = 41,            // delivery would be successful but recipient declined; deliverable OK
    Timeout = 21,            // delivery exceeded TTD >10m
    Rejected = 22,            // delivery would be successful but deliverable subpar.
    Missing = 0             // data missing
}

#[arduino_hal::entry]
fn main() -> ! {
    // Initialise allocator (ripped from https://crates.io/crates/embedded-alloc/0.6.0)
    use core::mem::MaybeUninit;
    const HEAP_SIZE: usize = 1024;
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }

    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);
    let mut serial = arduino_hal::default_serial!(dp, pins, 57600);
    let mut i2c = arduino_hal::I2c::new(
        dp.TWI,
        pins.d20.into_pull_up_input(),
        pins.d21.into_pull_up_input(),
        50000,
    );

    i2c.i2cdetect(&mut serial, Direction::Write).expect("TODO: panic message");
    let target = 0x27;
    uwriteln!(serial, "{:?}", i2c.ping_device(target, Direction::Write));

    let mut lcd = I2CLcd1602::new(i2c, target, serial);
    lcd.init();

    /*
     * For examples (and inspiration), head to
     *
     *     https://github.com/Rahix/avr-hal/tree/main/examples
     *
     * NOTE: Not all examples were ported to all boards!  There is a good chance though, that code
     * for a different board can be adapted for yours.  The Arduino Uno currently has the most
     * examples available.
     */

    // ====================================================================================

    let DEMO_TYPE = 3; // <--- it's dangerous to go alone, take this!!!

    // ====================================================================================
    // 
    // let rs: Pin<Output> = pins.d10.into_output().downgrade();
    // let rw: Pin<Output> = pins.d11.into_output().downgrade();
    // let en: Pin<Output> = pins.d12.into_output().downgrade();
    // 
    // let db0: Pin<Output> = pins.d2.into_output().downgrade();
    // let db1: Pin<Output> = pins.d3.into_output().downgrade();
    // let db2: Pin<Output> = pins.d4.into_output().downgrade();
    // let db3: Pin<Output> = pins.d5.into_output().downgrade();
    // let db4: Pin<Output> = pins.d6.into_output().downgrade();
    // let db5: Pin<Output> = pins.d7.into_output().downgrade();
    // let db6: Pin<Output> = pins.d8.into_output().downgrade();
    // let db7: Pin<Output> = pins.d9.into_output().downgrade();
    // 
    // let mut lcd = Lcd1602::new(rs, rw, en, [db0, db1, db2, db3, db4, db5, db6, db7], serial);
    // let mut emgr: EntryManager = EntryManager::new(Eeprom::new(dp.EEPROM));
    // emgr.load_sample(&lcd.mapper);
    // 
    // 
    // lcd.init();
    // lcd.cmd(&0b00_0011_1000);
    // lcd.cmd(&0b00_0000_1100);
    // lcd.cmd(&0b00_0000_0110);
    // 
    // match DEMO_TYPE {
    //     0 => {
    //         let mut tour = vec![0, 1, 2, 6, 9];
    //         two_opt(&mut tour, 10);
    // 
    //         let stour = &format!("{:?}", tour);
    //         lcd.demo("** PATH FOUND **", stour, 2, true);
    // 
    //         arduino_hal::delay_ms(400);
    //         //
    //         // lcd.clr();
    //         // lcd.affix(0, "initializing. . .");
    //         // lcd.affix(1, "▓▓");
    //         // arduino_hal::delay_ms(300);
    //         // lcd.affix(1, "▓▓ ▓▓");
    //         // arduino_hal::delay_ms(300);
    //         // lcd.affix(1, "▓▓ ▓▓ ▓▓");
    //         // arduino_hal::delay_ms(300);
    //         // lcd.affix(1, "▓▓ ▓▓ ▓▓ ▓▓");
    //         // arduino_hal::delay_ms(300);
    //         // lcd.affix(1, "▓▓ ▓▓ ▓▓ ▓▓ ▓▓");
    //         // arduino_hal::delay_ms(300);
    //         lcd.clr();
    //         lcd.affix(0, "Init OK!");
    //         lcd.affix(1, "Reading predat 1");
    //         arduino_hal::delay_ms(2000);
    // 
    //         let (dictname, desc, cgrsym, dist) = emgr.read_pre(0, 1);
    // 
    //         lcd.clr();
    //         lcd.disp_sym(cgrsym);
    //         lcd.disp_str(&format!(" {}m to {}", dist, dictname));
    //         lcd.dds(0x40);
    //         lcd.disp_symv(Vec::from(desc));
    //         lcd.marque(3, true);
    //         //
    //         arduino_hal::delay_ms(4000);
    //         // lcd.demo("Transmuting predat 1", "Writing postdat 1", 0, false);
    //         // arduino_hal::delay_ms(1000);
    //         //
    //         //
    //         lcd.demo("Thank you.", "Insert demo B.", 0, false)
    //     }
    //     1 => {
    //         lcd.affix(0, "Test EEPROM @ 0x0");
    //         lcd.marque(1, false);
    //         lcd.dds(0x40);
    //         for i in 0..256 {
    //             lcd.disp_str(&format!("{}", emgr.eepread(i)));
    //         }
    //         lcd.clr();
    //         lcd.affix(0, "predat OK");
    //         arduino_hal::delay_ms(2000);
    // 
    //         lcd.affix(0, "Test EEPROM @ 0xC00");
    //         lcd.marque(1, false);
    //         lcd.dds(0x40);
    //         for i in 0xC00..(0xC00 + 8) {
    //             lcd.disp_str(&format!("{}", emgr.eepread(i)));
    //             arduino_hal::delay_ms(100);
    //         }
    //         lcd.clr();
    //         lcd.affix(0, "postdat OK");
    //         arduino_hal::delay_ms(2000);
    //         lcd.clr();
    //         lcd.demo("Thank you.", "Insert demo C.", 0, false)
    //     }
    //     2 => {
    //         lcd.timer("PREPARE TO TEST", 30u16);
    //         lcd.clr();
    //         lcd.timer("check current! →", 120u16);
    // 
    //         lcd.clr();
    //         lcd.disp_str("DEMO COMPLETE!       DEMO COMPLETE!");
    //         lcd.marquee(400);
    //     }
    //     3 => {
    //         lcd.timer("** OK **", 5999u16);
    //     }
    //     _ => {}
    // }
    // 
    // 
    // // lcd.disp_str("Ample chamomile and honey, omit lavender. B12 → C40. ▓▓▓ KEEP WARM, ALLERGEN ▓\
    // // ▓▓");
    // 
    // 
    // //lcd.disp_symv(vec![0b0011_0000, 0b0011_0001, 0b0011_1010, 0b0011_1000, 0b0011_0100]);
    // 
    // //lcd.disp_symv(vec![0b0111_0111u8, 0b0110_1111u8, 0b0111_1010u8, 0b1110_1111u8, 0b1111_1111u8, 0b1111_1010u8]);
    // //
    // // let mut bomb = 300u16;
    // // let mut blink = true;
    // // while bomb > 0 {
    // //     lcd.clr();
    // //     lcd.disp_str(&*format!("{:02}", bomb / 60));
    // //
    // //     let blc = if blink { ':' } else { ' ' };
    // //     blink = !blink;
    // //     lcd.disp_char(blc);
    // //
    // //     lcd.disp_str(&*format!("{:02}", bomb % 60));
    // //
    // //     bomb -= 1;
    // //
    // //     arduino_hal::delay_ms(800);
    // // }
    // //
    // // lcd.clr();
    // // lcd.disp_str("HAPPY NEW YEAR");
    // // lcd.marquee(400);
    // // lcd.marquee(600);
    loop {
       // ufmt::uwriteln!(&mut serial, "OK...\r").unwrap_infallible();
        arduino_hal::delay_ms(5000);
    }

    //lcd.disp_symv(vec![0b0100_1000, 0b0100_0001, 0b0101_0000, 0b0101_0000])
}





