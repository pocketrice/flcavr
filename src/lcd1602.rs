use crate::ASCII_TO_HD44780;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use arduino_hal::hal::port::{Dynamic, PE0, PE1};
use arduino_hal::port::mode::{Input, Output};
use arduino_hal::port::Pin;
use arduino_hal::Usart;
use avr_device::atmega2560::USART0;
use core::cell::RefCell;
use core::ops::RangeBounds;
use arduino_hal::prelude::_unwrap_infallible_UnwrapInfallible;
use embedded_hal::digital::{OutputPin, PinState};

// Adapted from https://www.waveshare.com/datasheet/LCD_en_PDF/LCD1602.pdf, https://cdn.sparkfun.com/assets/9/5/f/7/b/HD44780.pdf

pub struct Lcd1602 {
    pub(crate) rs: Pin<Output>,
    pub(crate) rw: Pin<Output>,
    en: Pin<Output>,
    db: [Pin<Output>; 8],// ← NOTE... little endian (0-7)
    pub(crate) serial: Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>
}

impl Lcd1602 {
    pub fn new(rs: Pin<Output>, rw: Pin<Output>, en: Pin<Output>, db: [Pin<Output>; 8], serial: Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>) -> Lcd1602 {
        Self { rs, rw, en, db, serial }
    }

    pub fn register(&mut self, mut byte: u8) { // ← write to DB register
        ufmt::uwriteln!(&mut self.serial, "REGISTERING {:?}", bits8(byte));
        for i in 0..8 {
            let dbi = &mut self.db[i];
            dbi.set_state(PinState::from(byte & 0x1 == 1)).expect("Could not set register pin state");
            byte >>= 1;
          //  ufmt::uwriteln!(&mut self.serial, "REGUPD {:?}", bits8(byte));
        }
    }

    pub fn check(&mut self) {
        let binding = self.db.iter().map(|p| u8::from(p.is_set_high())).rev().collect::<Vec<_>>();
        let ps: &[u8] = binding.as_slice();
        ufmt::uwriteln!(&mut self.serial, "CHK... {} {} / {:?}\n", u8::from(self.rs.is_set_high()), u8::from(self.rw.is_set_high()), ps);
    }

    pub fn dbx<R: RangeBounds<usize> + core::slice::SliceIndex<[Pin<Output, Dynamic>], Output = [Pin<Output, Dynamic>]>>(&mut self, i: R) -> u8 { // ← utility for bitmasking ith register value. Range to save accesses if several needed.
        let dbs: &[Pin<Output>] = self.db.get(i).expect("Could not index DB pins");
        let mut x = 0u8;

        for db in dbs {
            x |= u8::from(db.is_set_high());
            x <<= 1;
        }

        x
    }

    pub fn bus(&mut self) { // ← imagine a bus... wait at the bus stop... framerules... :p
        while self.rdb() {}
    }

    pub fn enp(&mut self) { // ← enable (E) pulse
        self.en.set_high();
        arduino_hal::delay_us(1);
        self.en.set_low();
        arduino_hal::delay_us(1);
        ufmt::uwriteln!(&mut self.serial, "ENP");
    }

    pub fn enp_then_bus(&mut self) {
        self.enp();
        arduino_hal::delay_us(300);
        self.bus();
    }

    pub fn cmd(&mut self, reg: &u16) { // ← the "skip pleasantries and go for it" option
        self.cmb(reg);
        self.bus();
        self.check();
    }

    pub fn cmb(&mut self, reg: &u16) { // cmd with no busing
        ufmt::uwriteln!(&mut self.serial, "CMD {} {} / {:?}", (reg >> 9) & 0b1u16, (reg >> 8) & 0b1u16, bits8((reg & 0xFF) as u8));
        self.register((reg & 0b00_1111_1111) as u8);
        self.rw.set_state(PinState::from((reg & 0b01_0000_0000) != 0)).expect("Could not set register pin state");
        self.rs.set_state(PinState::from((reg & 0b10_0000_0000) != 0)).expect("Could not set register pin state");
        self.enp();
        self.check();
    }

    pub fn rdb(&mut self) -> bool { // ← Read B(usy) flag
        self.rs.set_low();
        self.rw.set_high();
        self.enp();

        self.db[7].is_set_high()
    }

    pub fn clr(&mut self) { // ← screen clear
        self.cmd(&0b00_0000_0001);
    }

    pub fn ret(&mut self) { // ← cursor return
        let bits = &(0b00_0000_0010 | (self.dbx(0..=0) as u16));
        self.cmd(bits);
    }

    pub fn ems(&mut self, id: bool, s: bool) { // ← entry mode set
        self.cmd(&(0b00_0000_0100 | ((id as u16) << 1) | (s as u16)));
    }

    pub fn dsw(&mut self, d: bool, c: bool, b: bool) { // ← display switch
        self.cmd(&(0b00_0000_1000 | ((d as u16) << 2) | ((c as u16) << 1) | (b as u16)));
    }

    pub fn cds(&mut self, sc: bool, rl: bool) { // ← cursor/display shift
        let bits = &(0b00_0001_0000 | ((sc as u16) << 3) | ((rl as u8) << 2 | self.dbx(0..=1)) as u16);
        self.cmd(bits);
    }

    pub fn fns(&mut self, dl: bool, n: bool, f: bool) { // ← function set
        let bits = &(0b00_0010_0000 | ((dl as u16) << 4) | ((n as u16) << 3) | ((f as u8) << 2 | self.dbx(0..=1)) as u16);
        self.cmd(bits);
    }

    pub fn cgs(&mut self, acg: u8) { // ← CGRAM set address
        self.cmd(&(0b00_0100_0000 | (acg as u16) & 0b00_0011_1111));
    }

    pub fn dds(&mut self, add: u8) { // ← DDRAM set address
        self.cmd(&(0b00_1000_0000 | (add as u16) & 0b00_0111_1111));
    }

    pub fn dtw(&mut self, data: u8) { // ← Data write (cgs/dds 1st!)
        self.cmd(&(0b10_0000_0000 | (data as u16)));
    }

    pub fn dtr(&mut self) -> u8 { // ← Data read (cgs/dds 1st!)
        self.rs.set_high();
        self.rw.set_high();
        self.enp();

        self.dbx(0..8)
    }

    // ========================== UTILITY ===============================
    // Partially based on HD44780U datasheet p40-41.
    pub fn cgload(&mut self, data: [[u8;8];7]) { // ← load 5x8 CGRAM symbols (0-5 LSB). Read from flash memory.
        // CGRAM addresses are 0b000000-0b001111, relevant CGRAM data is 5c x 8r = 40 bits.
        for symind in 0..data.len() {
            let sym = data[symind];
            for symline in 0..sym.len() {
                self.cgs((&symind << 3 | symline) as u8);
                self.dtw(sym[symline]) // NOTE: MS3B irrelevant, but also masking is redundant.
            }

            // Protect 8th line for cursor
            self.cgs((&symind << 3 | 0b000111usize) as u8);
            self.dtw(0x0);
        }
    }

    pub fn init(&mut self) {
        // See Figure 23 of Hitachi HD44780U datasheet; manual initialisation
        arduino_hal::delay_ms(150);
        self.cmb(&0b00_0011_0000);
        arduino_hal::delay_ms(10);
        self.cmb(&0b00_0011_0000);
        arduino_hal::delay_us(150);
        self.cmb(&0b00_0011_0000);
        arduino_hal::delay_us(150);
        self.cmd(&0b00_0011_1000); // DL=8D, N=2R, F=5x7
        self.cmd(&0b00_0000_1000); // Display off
        self.cmd(&0b00_0000_0001); // Display clear
        self.cmd(&0b00_0000_0111); // I/D=inc, S=shift

        ufmt::uwriteln!(&mut self.serial, "\n\nInitialised.\n\n");
    }


    pub fn disp_str(&mut self, str: &str) {
        self.disp_symv(str.chars().map(|c| ASCII_TO_HD44780.get(&c).cloned().unwrap()).collect())
    }

    pub fn disp_sym(&mut self, sym: u8) {
        // 16 x 2 = 32B DDRAM
        // TODO: DDRAM is already selected?
        self.dtw(sym);
    }

    pub fn disp_symv(&mut self, symv: Vec<u8>) {
        assert!(symv.iter().all(|b| b < &0b11100000)); // Only accept 8-bit symbols

        for sym in symv {
            self.disp_sym(sym);
            // TODO: need to change lines?
        }
    }
}

pub fn bits8(byte: u8) -> [u8; 8] { // big-endian (8-0)
    [byte >> 7 & 0x1, byte >> 6 & 0x1, byte >> 5 & 0x1, byte >> 4 & 0x1, byte >> 3 & 0x1, byte >> 2 & 0x1, byte >> 1 & 0x1, byte & 0x1]
}

pub fn bits16(num: u16) -> [u8; 16] {
    [(num >> 15 & 0x1) as u8, (num >> 14 & 0x1) as u8, (num >> 13 & 0x1) as u8, (num >> 12 & 0x1) as u8, (num >> 11 & 0x1) as u8, (num >> 10 & 0x1) as u8, (num >> 9 & 0x1) as u8, (num >> 8 & 0x1) as u8, (num >> 7 & 0x1) as u8, (num >> 6 & 0x1) as u8, (num >> 5 & 0x1) as u8, (num >> 4 & 0x1) as u8, (num >> 3 & 0x1) as u8, (num >> 2 & 0x1) as u8, (num >> 1 & 0x1) as u8, (num & 0x1) as u8]
}