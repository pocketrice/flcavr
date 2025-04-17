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

use crate::datmgt::EntryManager;
use crate::lcd1602::Lcd1602;
use alloc::format;
use alloc::rc::Rc;
use core::cell::RefCell;
use arduino_hal::port::mode::Output;
use arduino_hal::port::Pin;
use arduino_hal::prelude::_unwrap_infallible_UnwrapInfallible;
use arduino_hal::Eeprom;
use embedded_alloc::LlffHeap as Heap;
use embedded_hal::digital::OutputPin;

use panic_halt as _;
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
    let mut serial = Rc::new(RefCell::new(arduino_hal::default_serial!(dp, pins, 57600)));

    /*
     * For examples (and inspiration), head to
     *
     *     https://github.com/Rahix/avr-hal/tree/main/examples
     *
     * NOTE: Not all examples were ported to all boards!  There is a good chance though, that code
     * for a different board can be adapted for yours.  The Arduino Uno currently has the most
     * examples available.
     */

    let rs: Pin<Output> = pins.d10.into_output().downgrade();
    let rw: Pin<Output> = pins.d11.into_output().downgrade();
    let en: Pin<Output> = pins.d12.into_output().downgrade();

    let db0: Pin<Output> = pins.d2.into_output().downgrade();
    let db1: Pin<Output> = pins.d3.into_output().downgrade();
    let db2: Pin<Output> = pins.d4.into_output().downgrade();
    let db3: Pin<Output> = pins.d5.into_output().downgrade();
    let db4: Pin<Output> = pins.d6.into_output().downgrade();
    let db5: Pin<Output> = pins.d7.into_output().downgrade();
    let db6: Pin<Output> = pins.d8.into_output().downgrade();
    let db7: Pin<Output> = pins.d9.into_output().downgrade();

    let mut lcd = Lcd1602::new(rs, rw, en, [db0, db1, db2, db3, db4, db5, db6, db7], Rc::clone(&serial));
    //let mut emgr: EntryManager = EntryManager::new(Eeprom::new(dp.EEPROM), Rc::clone(&serial));
   // emgr.load_sample();

    lcd.init();
    lcd.cmd(&0b00_0011_1000);
    lcd.cmd(&0b00_0000_1100);
    lcd.cmd(&0b00_0000_0110);
    // lcd.disp_str("Ample chamomile and honey, omit lavender. B12 → C40. ▓▓▓ KEEP WARM, ALLERGEN ▓\
    // ▓▓");


    //lcd.disp_symv(vec![0b0011_0000, 0b0011_0001, 0b0011_1010, 0b0011_1000, 0b0011_0100]);

    //lcd.disp_symv(vec![0b0111_0111u8, 0b0110_1111u8, 0b0111_1010u8, 0b1110_1111u8, 0b1111_1111u8, 0b1111_1010u8]);

    let mut bomb = 300u16;
    let mut blink = true;
    while bomb > 0 {
        lcd.clr();
        lcd.disp_str(&*format!("{:02}", bomb / 60));

        let blc = if blink { ':' } else { ' ' };
        blink = !blink;
        lcd.disp_char(blc);

        lcd.disp_str(&*format!("{:02}", bomb % 60));

        bomb -= 1;

        arduino_hal::delay_ms(800);
    }

    lcd.clr();
    lcd.disp_str("HAPPY NEW YEAR");
    lcd.marquee(400);

   // lcd.marquee(600);
    loop {
        ufmt::uwriteln!(*Rc::get_mut(&mut serial).unwrap().borrow_mut(), "OK...\r").unwrap_infallible();
        arduino_hal::delay_ms(5000);
    }

    //lcd.disp_symv(vec![0b0100_1000, 0b0100_0001, 0b0101_0000, 0b0101_0000])
}





