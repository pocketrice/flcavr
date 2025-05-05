use alloc::format;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;
use arduino_hal::hal::port::{Dynamic, PE0, PE1};
use arduino_hal::port::mode::{Input, Output};
use arduino_hal::port::Pin;
use arduino_hal::prelude::{_embedded_hal_blocking_i2c_Read, _embedded_hal_blocking_i2c_Write, _embedded_hal_blocking_i2c_WriteRead, _unwrap_infallible_UnwrapInfallible};
use arduino_hal::{I2c, Usart};
use avr_device::atmega2560::USART0;
use core::ops::RangeBounds;
use embedded_hal::digital::{OutputPin, PinState};
use fchashmap::FcHashMap;
use ufmt::uwriteln;
use crate::bitops::{bits16, bits8};
// Adapted from https://www.waveshare.com/datasheet/LCD_en_PDF/LCD1602.pdf, https://cdn.sparkfun.com/assets/9/5/f/7/b/HD44780.pdf

const I2C_RS: u8 = 0;
const I2C_RW: u8 = 1;
const I2C_E: u8 = 2;
const I2C_BL: u8 = 3;
const I2C_D0: u8 = 4;
const I2C_D1: u8 = 5;
const I2C_D2: u8 = 6;
const I2C_D3: u8 = 7;

const CGRAM_UP: [[u8; 5]; 8] = {
    [
        [0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0],
        [0, 1, 1, 1, 0],
        [1, 0, 1, 0, 1],
        [0, 0, 1, 0, 1],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0]
    ]
};

const CGRAM_DOWN: [[u8; 5]; 8] = {
    [
        [0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 1, 0, 0],
        [1, 0, 1, 0, 1],
        [0, 1, 1, 1, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0]
    ]
};

const CGRAM_UP_LEFT: [[u8; 5]; 8] = {
    [
        [0, 0, 0, 0, 0],
        [0, 1, 1, 1, 0],
        [1, 1, 0, 0, 0],
        [1, 0, 1, 0, 0],
        [1, 0, 0, 1, 0],
        [0, 0, 0, 0, 1],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0]
    ]
};


const CGRAM_UP_RIGHT: [[u8; 5]; 8] = {
    [
        [0, 0, 0, 0, 0],
        [0, 1, 1, 1, 0],
        [0, 0, 0, 1, 1],
        [0, 0, 1, 0, 1],
        [0, 1, 0, 0, 1],
        [1, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0]
    ]
};

const CGRAM_DOWN_LEFT: [[u8; 5]; 8] = {
    [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 1],
        [1, 0, 0, 1, 0],
        [1, 0, 1, 0, 0],
        [1, 1, 0, 0, 0],
        [0, 1, 1, 1, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0]
    ]
};

const CGRAM_DOWN_RIGHT: [[u8; 5]; 8] = {
    [
        [0, 0, 0, 0, 0],
        [1, 0, 0, 0, 0],
        [0, 1, 0, 0, 1],
        [0, 0, 1, 0, 1],
        [0, 0, 0, 1, 1],
        [0, 1, 1, 1, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0]
    ]
};

pub enum MarqueStyle {
    SoloHighL,
    SoloHighR,
    SoloLowL,
    SoloLowR,
    SyncLeft,
    SyncRight,
    GearLeft,
    GearRight
}

pub trait HD44780Kernel {
    fn bus(&mut self);
    fn enp(&mut self);
    fn enp_then_bus(&mut self);
    fn cmd(&mut self, reg: &u16);
    fn cmb(&mut self, reg: &u16);
    fn rdb(&mut self) -> bool;
    fn clr(&mut self);
    fn ret(&mut self);
    fn ems(&mut self, id: bool, s: bool);
    fn dsw(&mut self, d: bool, c: bool, b: bool);
    fn cds(&mut self, sc: bool, rl: bool);
    fn fns(&mut self, dl: bool, n: bool, f: bool);
    fn cgs(&mut self, addr: u8);
    fn dds(&mut self, addr: u8);
    fn dtw(&mut self, data: u8);
    fn dtr(&mut self) -> u8;
}

pub trait HD44780Util {
    fn cgload(&mut self, data: [[u8;8];7]);
    fn init(&mut self);
    fn affix(&mut self, row: u8, str: &str);
    fn marque(&mut self, iters: u8, graceful: bool);
    fn demo(&mut self, row0: &str, row1: &str, iters: u8, graceful: bool);
}

pub trait Lcd1602 {
    fn disp_char(&mut self, c: char);
    fn disp_sym(&mut self, sym: u8);
    fn disp_str(&mut self, str: &str);
}

trait I2CBlOps { // ← blops stands for "byte-level ops"
    fn iiw(&mut self, data: &u8);
    fn iir(&mut self) -> u8;
    fn iirw(&mut self, data: &u8) -> u8;
}

trait Pinnable { // ← pin-level operations; requires struct to have buffer attribute
    fn pin(&mut self, i: u8, state: bool);
    fn fin(&mut self) -> u8;
    fn gin(&mut self, i: u8, state: bool);
    fn bin(&mut self, bitmask: u8, state: u8);
}

pub struct ParallelLcd1602 {
    rs: Pin<Output>,
    rw: Pin<Output>,
    en: Pin<Output>,
    db: [Pin<Output>; 8],// ← NOTE... little endian (0-7)
    serial: Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>,
    mapper: FcHashMap<char, u8, 256>,
    anchor: u8, // TODO: account for EMS S = 0.
    overcast: u8
}

pub struct I2CLcd1602 {
    serial: Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>,
    i2c: I2c,
    mapper: FcHashMap<char, u8, 256>,
    target: u8,
    buf: u8,
    anchor: u8,
    overcast: u8
}

impl HD44780Kernel for ParallelLcd1602 {
    fn bus(&mut self) { // ← imagine a bus... wait at the bus stop... framerules... :p
        while self.rdb() {}
    }

    fn enp(&mut self) { // ← enable (E) pulse
        self.en.set_high();
        arduino_hal::delay_us(1);
        self.en.set_low();
        arduino_hal::delay_us(1);
        ufmt::uwriteln!(&mut self.serial, "ENP OK");
    }

    fn enp_then_bus(&mut self) {
        self.enp();
        self.bus();
    }

    fn cmd(&mut self, reg: &u16) { // ← the "skip pleasantries and go for it" option
        self.cmb(reg);
        self.bus();
    }

    fn cmb(&mut self, reg: &u16) { // cmd with no busing
        ufmt::uwriteln!(&mut self.serial, "CMD: {} {} / {:?}", (reg >> 9) & 0b1u16, (reg >> 8) & 0b1u16, bits8((reg & 0xFF) as u8));

        self.register((reg & 0b00_1111_1111) as u8);
        self.rw.set_state(PinState::from((reg & 0b01_0000_0000) != 0)).expect("Could not set register pin state");
        self.rs.set_state(PinState::from((reg & 0b10_0000_0000) != 0)).expect("Could not set register pin state");
        self.enp();
        self.check();
    }

    fn rdb(&mut self) -> bool { // ← Read B(usy) flag
        self.rs.set_low();
        self.rw.set_high();
        self.enp();

        self.en.is_set_high()
    }

    fn clr(&mut self) { // ← screen clear
        self.cmd(&0b00_0000_0001);
    }

    fn ret(&mut self) { // ← cursor return
        let bits = &(0b00_0000_0010 | (self.dbx(0..=0) as u16));
        self.cmd(bits);
    }

    fn ems(&mut self, id: bool, s: bool) { // ← entry mode set
        self.cmd(&(0b00_0000_0100 | ((id as u16) << 1) | (s as u16)));
    }

    fn dsw(&mut self, d: bool, c: bool, b: bool) { // ← display switch
        self.cmd(&(0b00_0000_1000 | ((d as u16) << 2) | ((c as u16) << 1) | (b as u16)));
    }

    fn cds(&mut self, sc: bool, rl: bool) { // ← cursor/display shift
        let bits = &(0b00_0001_0000 | ((sc as u16) << 3) | ((rl as u8) << 2 | self.dbx(0..=1)) as u16);
        self.cmd(bits);
    }

    fn fns(&mut self, dl: bool, n: bool, f: bool) { // ← function set
        let bits = &(0b00_0010_0000 | ((dl as u16) << 4) | ((n as u16) << 3) | ((f as u8) << 2 | self.dbx(0..=1)) as u16);
        self.cmd(bits);
    }

    fn cgs(&mut self, addr: u8) { // ← CGRAM set address
        self.cmd(&(0b00_0100_0000 | (addr as u16) & 0b00_0011_1111));
    }

    fn dds(&mut self, addr: u8) { // ← DDRAM set address
        self.cmb(&(0b00_1000_0000 | (addr as u16) & 0b00_0111_1111));
        arduino_hal::delay_us(70);
    }

    fn dtw(&mut self, data: u8) { // ← Data write (cgs/dds 1st!)
        self.cmb(&(0b10_0000_0000 | (data as u16))); // Froze on busing, so manual delay override.
        arduino_hal::delay_us(70);
    }

    fn dtr(&mut self) -> u8 { // ← Data read (cgs/dds 1st!)
        self.rs.set_high();
        self.rw.set_high();
        self.enp();

        self.dbx(0..8)
    }
}

impl HD44780Kernel for I2CLcd1602 { // ~~← [[ RS RW E D4 D5 D6 D7 _ ]]~~ [[ D7/3 D6/2 D5/1 D4/0 BL EN RW RS ]]
    fn bus(&mut self) {
        while self.rdb() {}
    }

    fn enp(&mut self) {
        self.gin(I2C_E, true);
        arduino_hal::delay_us(1);
        self.gin(I2C_E, false);
        arduino_hal::delay_us(1);
        ufmt::uwriteln!(&mut self.serial, "ENP OK");
    }

    fn enp_then_bus(&mut self) {
        self.enp();
        self.bus();
    }

    fn cmd(&mut self, reg: &u16) {
        self.cmb(reg);
        self.bus();
    }

    fn cmb(&mut self, reg: &u16) {
        let append = ((reg & 0b11_0000_0000) >> 4) as u8;  // ← RS, RW
        self.iiw(&(append + ((reg & 0b1111_0000) >> 4) as u8));
        uwriteln!(self.serial, "U: {:?}", bits8(append + ((reg & 0b1111_0000) >> 4) as u8));
        self.bus();
        self.iiw(&(append + (reg & 0b0000_1111) as u8));
        uwriteln!(self.serial, "L: {:?}\n\n", bits8(append + (reg & 0b0000_1111) as u8));
    }

    fn rdb(&mut self) -> bool {
        self.bin(0b1100_0000, 0b0100_0000);
        self.fin();
        self.enp();
        self.iir() & 0b0010_0000 != 0 // BF set high
    }

    fn clr(&mut self) {
        self.cmb(&0b00_0000_0001);
    }

    fn ret(&mut self) {
        self.cmbm(&0b11_1111_1110, &0b00_0000_0010);
    }

    fn ems(&mut self, id: bool, s: bool) {
        self.cmb(&(0b00_0000_0100 | ((id as u16) << 1) | (s as u16)));
    }

    fn dsw(&mut self, d: bool, c: bool, b: bool) {
        self.cmb(&(0b00_0000_1000 | ((d as u16) << 2) | ((c as u16) << 1) | (b as u16)));
    }

    fn cds(&mut self, sc: bool, rl: bool) {
        self.cmbm(&0b11_1111_1100, &(0b00_0001_0000 | ((sc as u16) << 3) | ((rl as u16) << 2)));
    }

    fn fns(&mut self, dl: bool, n: bool, f: bool) {
        self.cmbm(&0b11_1111_1100, &(0b00_0010_0000 | ((dl as u16) << 4) | ((n as u16) << 3) | ((f as u16) << 2)));
    }

    fn cgs(&mut self, addr: u8) {
        self.cmb(&(0b00_0100_0000 | (addr as u16) & 0b00_0011_1111));
    }

    fn dds(&mut self, addr: u8) {
        self.cmb(&(0b00_1000_0000 | (addr as u16) & 0b00_0111_1111));
        arduino_hal::delay_us(70);
    }

    fn dtw(&mut self, data: u8) {
        self.cmd(&(0b10_0000_0000 | (data as u16))); // Froze on busing, so manual delay override.
        arduino_hal::delay_us(70);
    }

    fn dtr(&mut self) -> u8 {
        self.cmbm(&0b11_0000_0000, &0b11_0000_0000);
        self.enp();
        self.iir()  // TODO how to read both nibbles?????f
    }
}

impl HD44780Util for ParallelLcd1602 {
    // ========================== UTILITY ===============================
    // Partially based on HD44780U datasheet p40-41.
    fn cgload(&mut self, data: [[u8; 8]; 7]) { // ← load 5x8 CGRAM symbols (0-5 LSB). Read from flash memory.
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

    fn init(&mut self) {
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

    fn affix(&mut self, row: u8, str: &str) { // heckin' keep calling things affix lul (lovely word)
        let dd_addr = match row {
            0 => 0x00,
            1 => 0x40,
            _ => 0x00
        };

        self.dds(dd_addr);
        self.disp_str(str);
    }

    fn marque(&mut self, iters: u8, graceful: bool) {
        // 40 chars per line
        for _ in 0..iters {
            for i in 0..42 {
                match i {
                    0 | 42 => {
                        if graceful {
                            arduino_hal::delay_ms(1000);
                        }
                    }

                    _ => {
                        self.cds(true, true);
                        arduino_hal::delay_ms(200);
                    }
                }
            }
        }
    }

    fn demo(&mut self, row0: &str, row1: &str, iters: u8, graceful: bool) {
        self.clr();
        self.affix(0, row0);
        self.affix(1, row1);
        self.marque(iters, graceful);
    }
}

impl Lcd1602 for ParallelLcd1602 {
    fn disp_char(&mut self, c: char) {
        let map = self.mapper.get(&c);

        if map.is_some() {
            self.disp_sym(*map.unwrap_or_else(|| &0b1111_1111));
        } else {
            ufmt::uwriteln!(&mut self.serial, "NOMAP => {}", c);
        }
    }

    fn disp_sym(&mut self, sym: u8) {
        // 16 x 2 = 32B DDRAM
        // TODO: DDRAM is already selected?
        self.dtw(sym);
    }

    fn disp_str(&mut self, str: &str) {
        for c in str.chars() {
            self.disp_char(c);
        }
    }
}

impl ParallelLcd1602 {
    pub fn new(rs: Pin<Output>, rw: Pin<Output>, en: Pin<Output>, db: [Pin<Output>; 8], serial: Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>) -> ParallelLcd1602 {
        Self { rs, rw, en, db, serial, mapper: {
            let mut fhm = FcHashMap::new();
            fhm.insert('↑', 0b1111_1111).unwrap(); // TODO UPDATE WITH CGRAM SYMBOLS WHEN IMPLEMENTED
            fhm.insert('↓', 0b1111_1111).unwrap();
            fhm.insert('↖', 0b1111_1111).unwrap();
            fhm.insert('↗', 0b1111_1111).unwrap();
            fhm.insert('↘', 0b1111_1111).unwrap();
            fhm.insert('↙', 0b1111_1111).unwrap();

            fhm.insert('▓', 0b1111_1111).unwrap();
            fhm.insert('"', 0b0010_0010).unwrap();
            fhm.insert('!', 0b0010_0001).unwrap(); // ← Based on JIS X 0201 with JIS X 0208 mappings for ktk
            fhm.insert('#', 0b0010_0011).unwrap();
            fhm.insert('$', 0b0010_0100).unwrap();
            fhm.insert('%', 0b0010_0101).unwrap();
            fhm.insert('&', 0b0010_0110).unwrap();
            fhm.insert('\'', 0b0010_0111).unwrap();
            fhm.insert('(', 0b0010_1000).unwrap();
            fhm.insert(')', 0b0010_1001).unwrap();
            fhm.insert('*', 0b0010_1010).unwrap();
            fhm.insert('+', 0b0010_1011).unwrap();
            fhm.insert(',', 0b0010_1100).unwrap();
            fhm.insert('-', 0b0010_1101).unwrap();
            fhm.insert('.', 0b0010_1110).unwrap();
            fhm.insert('/', 0b0010_1101).unwrap();

            fhm.insert('0', 0b0011_0000).unwrap();
            fhm.insert('1', 0b0011_0001).unwrap();
            fhm.insert('2', 0b0011_0010).unwrap();
            fhm.insert('3', 0b0011_0011).unwrap();
            fhm.insert('4', 0b0011_0100).unwrap();
            fhm.insert('5', 0b0011_0101).unwrap();
            fhm.insert('6', 0b0011_0110).unwrap();
            fhm.insert('7', 0b0011_0111).unwrap();
            fhm.insert('8', 0b0011_1000).unwrap();
            fhm.insert('9', 0b0011_1001).unwrap();
            fhm.insert(':', 0b0011_1010).unwrap();
            fhm.insert(';', 0b0011_1011).unwrap();
            fhm.insert('<', 0b0011_1100).unwrap();
            fhm.insert('=', 0b0011_1101).unwrap();
            fhm.insert('>', 0b0011_1110).unwrap();
            fhm.insert('?', 0b0011_1101).unwrap();

            fhm.insert('@', 0b0100_0000).unwrap();
            fhm.insert('A', 0b0100_0001).unwrap();
            fhm.insert('B', 0b0100_0010).unwrap();
            fhm.insert('C', 0b0100_0011).unwrap();
            fhm.insert('D', 0b0100_0100).unwrap();
            fhm.insert('E', 0b0100_0101).unwrap();
            fhm.insert('F', 0b0100_0110).unwrap();
            fhm.insert('G', 0b0100_0111).unwrap();
            fhm.insert('H', 0b0100_1000).unwrap();
            fhm.insert('I', 0b0100_1001).unwrap();
            fhm.insert('J', 0b0100_1010).unwrap();
            fhm.insert('K', 0b0100_1011).unwrap();
            fhm.insert('L', 0b0100_1100).unwrap();
            fhm.insert('M', 0b0100_1101).unwrap();
            fhm.insert('N', 0b0100_1110).unwrap();
            fhm.insert('O', 0b0100_1111).unwrap();

            fhm.insert('P', 0b0101_0000).unwrap();
            fhm.insert('Q', 0b0101_0001).unwrap();
            fhm.insert('R', 0b0101_0010).unwrap();
            fhm.insert('S', 0b0101_0011).unwrap();
            fhm.insert('T', 0b0101_0100).unwrap();
            fhm.insert('U', 0b0101_0101).unwrap();
            fhm.insert('V', 0b0101_0110).unwrap();
            fhm.insert('W', 0b0101_0111).unwrap();
            fhm.insert('X', 0b0101_1000).unwrap();
            fhm.insert('Y', 0b0101_1001).unwrap();
            fhm.insert('Z', 0b0101_1010).unwrap();
            fhm.insert('[', 0b0101_1011).unwrap();
            fhm.insert('¥', 0b0101_1100).unwrap();
            fhm.insert(']', 0b0101_1101).unwrap();
            fhm.insert('^', 0b0101_1110).unwrap();
            fhm.insert('_', 0b0101_1111).unwrap();

            fhm.insert('`', 0b0110_0000).unwrap();
            fhm.insert('a', 0b0110_0001).unwrap();
            fhm.insert('b', 0b0110_0010).unwrap();
            fhm.insert('c', 0b0110_0011).unwrap();
            fhm.insert('d', 0b0110_0100).unwrap();
            fhm.insert('e', 0b0110_0101).unwrap();
            fhm.insert('f', 0b0110_0110).unwrap();
            fhm.insert('g', 0b0110_0111).unwrap();
            fhm.insert('h', 0b0110_1000).unwrap();
            fhm.insert('i', 0b0110_1001).unwrap();
            fhm.insert('j', 0b0110_1010).unwrap();
            fhm.insert('k', 0b0110_1011).unwrap();
            fhm.insert('l', 0b0110_1100).unwrap();
            fhm.insert('m', 0b0110_1101).unwrap();
            fhm.insert('n', 0b0110_1110).unwrap();
            fhm.insert('o', 0b0110_1111).unwrap();

            fhm.insert('p', 0b0111_0000).unwrap();
            fhm.insert('q', 0b0111_0001).unwrap();
            fhm.insert('r', 0b0111_0010).unwrap();
            fhm.insert('s', 0b0111_0011).unwrap();
            fhm.insert('t', 0b0111_0100).unwrap();
            fhm.insert('u', 0b0111_0101).unwrap();
            fhm.insert('v', 0b0111_0110).unwrap();
            fhm.insert('w', 0b0111_0111).unwrap();
            fhm.insert('x', 0b0111_1000).unwrap();
            fhm.insert('y', 0b0111_1001).unwrap();
            fhm.insert('z', 0b0111_1010).unwrap();
            fhm.insert('{', 0b0111_1011).unwrap();
            fhm.insert('|', 0b0111_1100).unwrap();
            fhm.insert('}', 0b0111_1101).unwrap();
            fhm.insert('→', 0b0111_1110).unwrap();
            fhm.insert('←', 0b0111_1111).unwrap();

            // Skip 0b1000XXXX.

            fhm.insert(' ', 0b1010_0000).unwrap();
            fhm.insert('。', 0b1010_0001).unwrap();
            fhm.insert('「', 0b1010_0010).unwrap();
            fhm.insert('」', 0b1010_0011).unwrap();
            fhm.insert('ヽ', 0b1010_0100).unwrap();
            fhm.insert('・', 0b1010_0101).unwrap();
            fhm.insert('ヲ', 0b1010_0110).unwrap();
            fhm.insert('ァ', 0b1010_0111).unwrap();
            fhm.insert('ィ', 0b1010_1000).unwrap();
            fhm.insert('ゥ', 0b1010_1001).unwrap();
            fhm.insert('ェ', 0b1010_1010).unwrap();
            fhm.insert('ォ', 0b1010_1011).unwrap();
            fhm.insert('ャ', 0b1010_1100).unwrap();
            fhm.insert('ュ', 0b1010_1101).unwrap();
            fhm.insert('ョ', 0b1010_1110).unwrap();
            fhm.insert('ッ', 0b1010_1111).unwrap();

            fhm.insert('ー', 0b1011_0000).unwrap();
            fhm.insert('ア', 0b1011_0001).unwrap();
            fhm.insert('イ', 0b1011_0010).unwrap();
            fhm.insert('ウ', 0b1011_0011).unwrap();
            fhm.insert('エ', 0b1011_0100).unwrap();
            fhm.insert('オ', 0b1011_0101).unwrap();
            fhm.insert('カ', 0b1011_0110).unwrap();
            fhm.insert('キ', 0b1011_0111).unwrap();
            fhm.insert('ク', 0b1011_1000).unwrap();
            fhm.insert('ケ', 0b1011_1001).unwrap();
            fhm.insert('コ', 0b1011_1010).unwrap();
            fhm.insert('サ', 0b1011_1011).unwrap();
            fhm.insert('シ', 0b1011_1100).unwrap();
            fhm.insert('ス', 0b1011_1101).unwrap();
            fhm.insert('セ', 0b1011_1110).unwrap();
            fhm.insert('ソ', 0b1011_1111).unwrap();

            fhm.insert('タ', 0b1100_0000).unwrap();
            fhm.insert('チ', 0b1100_0001).unwrap();
            fhm.insert('ツ', 0b1100_0010).unwrap();
            fhm.insert('テ', 0b1100_0011).unwrap();
            fhm.insert('ト', 0b1100_0100).unwrap();
            fhm.insert('ナ', 0b1100_0101).unwrap();
            fhm.insert('ニ', 0b1100_0110).unwrap();
            fhm.insert('ヌ', 0b1100_0111).unwrap();
            fhm.insert('ネ', 0b1100_1000).unwrap();
            fhm.insert('ノ', 0b1100_1001).unwrap();
            fhm.insert('ハ', 0b1100_1010).unwrap();
            fhm.insert('ヒ', 0b1100_1011).unwrap();
            fhm.insert('フ', 0b1100_1100).unwrap();
            fhm.insert('ヘ', 0b1100_1101).unwrap();
            fhm.insert('ホ', 0b1100_1110).unwrap();
            fhm.insert('マ', 0b1100_1111).unwrap();

            fhm.insert('ミ', 0b1101_0000).unwrap();
            fhm.insert('ム', 0b1101_0001).unwrap();
            fhm.insert('メ', 0b1101_0010).unwrap();
            fhm.insert('モ', 0b1101_0011).unwrap();
            fhm.insert('ヤ', 0b1101_0100).unwrap();
            fhm.insert('ユ', 0b1101_0101).unwrap();
            fhm.insert('ヨ', 0b1101_0110).unwrap();
            fhm.insert('ラ', 0b1101_0111).unwrap();
            fhm.insert('リ', 0b1101_1000).unwrap();
            fhm.insert('ル', 0b1101_1001).unwrap();
            fhm.insert('レ', 0b1101_1010).unwrap();
            fhm.insert('ロ', 0b1101_1011).unwrap();
            fhm.insert('ワ', 0b1101_1100).unwrap();
            fhm.insert('ン', 0b1101_1101).unwrap();
            fhm.insert('゛', 0b1101_1110).unwrap();
            fhm.insert('゜', 0b1101_1111).unwrap();

            // 10-bit wide symbols...

            fhm.insert('α', 0b1110_0000).unwrap();
            fhm.insert('ä', 0b1110_0001).unwrap();
            fhm.insert('β', 0b1110_0010).unwrap();
            fhm.insert('ε', 0b1110_0011).unwrap();
            fhm.insert('μ', 0b1110_0100).unwrap();
            fhm.insert('σ', 0b1110_0101).unwrap();
            fhm.insert('ρ', 0b1110_0110).unwrap();
            fhm.insert('ⓖ', 0b1110_0111).unwrap(); // kerned g
            fhm.insert('√', 0b1110_1000).unwrap();
            fhm.insert('ⁱ', 0b1110_1001).unwrap(); // ← superscript -1
            fhm.insert('ⓙ', 0b1110_1010).unwrap(); // kerned j
            fhm.insert('*', 0b1110_1011).unwrap();
            fhm.insert('¢', 0b1110_1100).unwrap();
            fhm.insert('₤', 0b1110_1101).unwrap(); // gnd upsidedown? lira?
            fhm.insert('ñ', 0b1110_1110).unwrap();
            fhm.insert('ö', 0b1110_1111).unwrap();

            fhm.insert('ⓟ', 0b1111_0000).unwrap();  // kerned p
            fhm.insert('ⓠ', 0b1111_0001).unwrap();  // kerned q
            fhm.insert('θ', 0b1111_0010).unwrap();
            fhm.insert('∞', 0b1111_0011).unwrap();
            fhm.insert('Ω', 0b1111_0100).unwrap();
            fhm.insert('ü', 0b1111_0101).unwrap();
            fhm.insert('Σ', 0b1111_0110).unwrap();
            fhm.insert('π', 0b1111_0111).unwrap();
            fhm.insert('ⓧ', 0b1111_1000).unwrap(); // x-bar
            fhm.insert('ⓨ', 0b1111_1001).unwrap(); // kerned y
            fhm.insert('千', 0b1111_1010).unwrap();
            fhm.insert('万', 0b1111_1011).unwrap();
            fhm.insert('両', 0b1111_1100).unwrap();
            fhm.insert('÷', 0b1111_1101).unwrap();
            fhm.insert('▓', 0b1111_1111).unwrap();
            fhm
        }, anchor: 0, overcast: 0
        }
    }

    fn register(&mut self, mut byte: u8) { // ← write to DB register
        ufmt::uwriteln!(&mut self.serial, "REGISTERING {:?}", bits8(byte));
        for i in 0..8 {
            let dbi = &mut self.db[i];
            dbi.set_state(PinState::from(byte & 0x1 == 1)).expect("Could not set register pin state");
            byte >>= 1;
            // ufmt::uwriteln!(&mut self.serial, "REGUPD {:?}", bits8(byte));
        }
    }

    fn check(&mut self) {
        let binding = self.db.iter().map(|p| u8::from(p.is_set_high())).rev().collect::<Vec<_>>();
        let ps: &[u8] = binding.as_slice();
        ufmt::uwriteln!(&mut self.serial, "CHK: {} {} / {:?}\n", u8::from(self.rs.is_set_high()), u8::from(self.rw.is_set_high()), ps);
    }

    fn dbx<R: RangeBounds<usize> + core::slice::SliceIndex<[Pin<Output, Dynamic>], Output = [Pin<Output, Dynamic>]>>(&mut self, i: R) -> u8 { // ← utility for bitmasking ith register value. Range to save accesses if several needed.
        let dbs: &[Pin<Output>] = self.db.get(i).expect("Could not index DB pins");
        let mut x = 0u8;

        for db in dbs {
            x |= u8::from(db.is_set_high());
            x <<= 1;
        }

        x
    }

    fn disp_symv(&mut self, symv: Vec<u8>) {
        assert!(symv.iter().all(|b| b <= &0b11111111)); // Only accept 8-bit symbols

        for sym in symv {
            self.disp_sym(sym);
            // TODO: need to change lines?
        }
    }

    fn line(i: u8) {
        assert!(i == 0 || i == 1);


    }

    fn bso(&mut self, fw: bool) {   // ← cyclic "bit shift" for current line.
        // Read value @ 0, set DDRAM pointer +1, read/store and write @ 1.
        for mut i in 0..8u8 { // note that Range<> does not implement storing as reverse; use the either crate
            if !fw {
                i = 8 - i;
            }

            self.dds(8 % i);
            let mut sym = 0b0;

            if i != 0 {
                self.dtw(sym);
            }

            sym = {
                self.dtr();
                self.dbx(0..8)
            };
        }
    }

    fn bsd(&mut self, dist: u8) {
        // DDRAM is 0x0-0x20 (0-32)
        // Read DDRAM at 0x0, Set offset DDRAM pointer, read + store then write.
    }
    fn marquee(&mut self, ms: u32) {
        loop {
            self.cds(true, true);
            arduino_hal::delay_ms(ms);
        }
    }

    fn timer(&mut self, msg: &str, duration: u16) {
        let mut bomb = duration;
        let mut blink = true;
        while bomb > 0 {
            self.disp_str(&*format!("{:02}", bomb / 60));

            let blc = if blink { ':' } else { ' ' };
            blink = !blink;
            self.disp_char(blc);

            self.disp_str(&*format!("{:02}", bomb % 60));

            bomb -= 1;

            self.affix(1, msg);
            self.dds(0);

            arduino_hal::delay_ms(670);
        }
    }

    //                                                                                                        * ! **
    fn amarque(&mut self, iters: u8, style: MarqueStyle, fixstr: &str) {  // imaginary words, in my code??!! o.o
        for _ in 0..iters {
            for i in 0..42 {
                match style {
                    MarqueStyle::SoloHighR => {
                        self.cds(true, true);
                        self.affix(1, fixstr);
                        arduino_hal::delay_ms(130);
                    }

                    _ => {}
                }
            }
        }
    }

    // pub fn ls(&mut self, row: u8, rightward: bool) {  // Line shift
    //     let offset = row * 0x40; // Row is either 0 or 1
    //     let mut temp = 0u8;
    //
    //     if rightward {
    //         let mut prev = {
    //             self.dds(offset + 0x27);
    //             self.dtr()
    //         };
    //
    //         for i in offset..=(offset + 0x27) {
    //             temp = {
    //                 self.dds(i);
    //                 self.dtr()
    //             };
    //
    //             self.dtw(prev);
    //             prev = temp;
    //         }
    //     } else {
    //         let mut prev = {
    //             self.dds(offset + 1);
    //             self.dtr()
    //         };
    //
    //         for i in offset..=(offset + 0x27) {
    //             let oi = (offset + 0x27) - i;
    //
    //             temp = {
    //                 self.dds(oi);
    //                 self.dtr()
    //             };
    //
    //             self.dtw(prev);
    //             prev = temp;
    //         }
    //     }
    // }


    // pub fn map_str(str: &str) -> [u8; ]
}

impl I2CLcd1602 {
    pub fn new(i2c: I2c, target: u8, serial: Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>) -> I2CLcd1602 {
        Self { i2c, target, buf: 0u8, serial, mapper: {
            let mut fhm = FcHashMap::new();
            fhm.insert('↑', 0b1111_1111).unwrap(); // TODO UPDATE WITH CGRAM SYMBOLS WHEN IMPLEMENTED
            fhm.insert('↓', 0b1111_1111).unwrap();
            fhm.insert('↖', 0b1111_1111).unwrap();
            fhm.insert('↗', 0b1111_1111).unwrap();
            fhm.insert('↘', 0b1111_1111).unwrap();
            fhm.insert('↙', 0b1111_1111).unwrap();

            fhm.insert('▓', 0b1111_1111).unwrap();
            fhm.insert('"', 0b0010_0010).unwrap();
            fhm.insert('!', 0b0010_0001).unwrap(); // ← Based on JIS X 0201 with JIS X 0208 mappings for ktk
            fhm.insert('#', 0b0010_0011).unwrap();
            fhm.insert('$', 0b0010_0100).unwrap();
            fhm.insert('%', 0b0010_0101).unwrap();
            fhm.insert('&', 0b0010_0110).unwrap();
            fhm.insert('\'', 0b0010_0111).unwrap();
            fhm.insert('(', 0b0010_1000).unwrap();
            fhm.insert(')', 0b0010_1001).unwrap();
            fhm.insert('*', 0b0010_1010).unwrap();
            fhm.insert('+', 0b0010_1011).unwrap();
            fhm.insert(',', 0b0010_1100).unwrap();
            fhm.insert('-', 0b0010_1101).unwrap();
            fhm.insert('.', 0b0010_1110).unwrap();
            fhm.insert('/', 0b0010_1101).unwrap();

            fhm.insert('0', 0b0011_0000).unwrap();
            fhm.insert('1', 0b0011_0001).unwrap();
            fhm.insert('2', 0b0011_0010).unwrap();
            fhm.insert('3', 0b0011_0011).unwrap();
            fhm.insert('4', 0b0011_0100).unwrap();
            fhm.insert('5', 0b0011_0101).unwrap();
            fhm.insert('6', 0b0011_0110).unwrap();
            fhm.insert('7', 0b0011_0111).unwrap();
            fhm.insert('8', 0b0011_1000).unwrap();
            fhm.insert('9', 0b0011_1001).unwrap();
            fhm.insert(':', 0b0011_1010).unwrap();
            fhm.insert(';', 0b0011_1011).unwrap();
            fhm.insert('<', 0b0011_1100).unwrap();
            fhm.insert('=', 0b0011_1101).unwrap();
            fhm.insert('>', 0b0011_1110).unwrap();
            fhm.insert('?', 0b0011_1101).unwrap();

            fhm.insert('@', 0b0100_0000).unwrap();
            fhm.insert('A', 0b0100_0001).unwrap();
            fhm.insert('B', 0b0100_0010).unwrap();
            fhm.insert('C', 0b0100_0011).unwrap();
            fhm.insert('D', 0b0100_0100).unwrap();
            fhm.insert('E', 0b0100_0101).unwrap();
            fhm.insert('F', 0b0100_0110).unwrap();
            fhm.insert('G', 0b0100_0111).unwrap();
            fhm.insert('H', 0b0100_1000).unwrap();
            fhm.insert('I', 0b0100_1001).unwrap();
            fhm.insert('J', 0b0100_1010).unwrap();
            fhm.insert('K', 0b0100_1011).unwrap();
            fhm.insert('L', 0b0100_1100).unwrap();
            fhm.insert('M', 0b0100_1101).unwrap();
            fhm.insert('N', 0b0100_1110).unwrap();
            fhm.insert('O', 0b0100_1111).unwrap();

            fhm.insert('P', 0b0101_0000).unwrap();
            fhm.insert('Q', 0b0101_0001).unwrap();
            fhm.insert('R', 0b0101_0010).unwrap();
            fhm.insert('S', 0b0101_0011).unwrap();
            fhm.insert('T', 0b0101_0100).unwrap();
            fhm.insert('U', 0b0101_0101).unwrap();
            fhm.insert('V', 0b0101_0110).unwrap();
            fhm.insert('W', 0b0101_0111).unwrap();
            fhm.insert('X', 0b0101_1000).unwrap();
            fhm.insert('Y', 0b0101_1001).unwrap();
            fhm.insert('Z', 0b0101_1010).unwrap();
            fhm.insert('[', 0b0101_1011).unwrap();
            fhm.insert('¥', 0b0101_1100).unwrap();
            fhm.insert(']', 0b0101_1101).unwrap();
            fhm.insert('^', 0b0101_1110).unwrap();
            fhm.insert('_', 0b0101_1111).unwrap();

            fhm.insert('`', 0b0110_0000).unwrap();
            fhm.insert('a', 0b0110_0001).unwrap();
            fhm.insert('b', 0b0110_0010).unwrap();
            fhm.insert('c', 0b0110_0011).unwrap();
            fhm.insert('d', 0b0110_0100).unwrap();
            fhm.insert('e', 0b0110_0101).unwrap();
            fhm.insert('f', 0b0110_0110).unwrap();
            fhm.insert('g', 0b0110_0111).unwrap();
            fhm.insert('h', 0b0110_1000).unwrap();
            fhm.insert('i', 0b0110_1001).unwrap();
            fhm.insert('j', 0b0110_1010).unwrap();
            fhm.insert('k', 0b0110_1011).unwrap();
            fhm.insert('l', 0b0110_1100).unwrap();
            fhm.insert('m', 0b0110_1101).unwrap();
            fhm.insert('n', 0b0110_1110).unwrap();
            fhm.insert('o', 0b0110_1111).unwrap();

            fhm.insert('p', 0b0111_0000).unwrap();
            fhm.insert('q', 0b0111_0001).unwrap();
            fhm.insert('r', 0b0111_0010).unwrap();
            fhm.insert('s', 0b0111_0011).unwrap();
            fhm.insert('t', 0b0111_0100).unwrap();
            fhm.insert('u', 0b0111_0101).unwrap();
            fhm.insert('v', 0b0111_0110).unwrap();
            fhm.insert('w', 0b0111_0111).unwrap();
            fhm.insert('x', 0b0111_1000).unwrap();
            fhm.insert('y', 0b0111_1001).unwrap();
            fhm.insert('z', 0b0111_1010).unwrap();
            fhm.insert('{', 0b0111_1011).unwrap();
            fhm.insert('|', 0b0111_1100).unwrap();
            fhm.insert('}', 0b0111_1101).unwrap();
            fhm.insert('→', 0b0111_1110).unwrap();
            fhm.insert('←', 0b0111_1111).unwrap();

            // Skip 0b1000XXXX.

            fhm.insert(' ', 0b1010_0000).unwrap();
            fhm.insert('。', 0b1010_0001).unwrap();
            fhm.insert('「', 0b1010_0010).unwrap();
            fhm.insert('」', 0b1010_0011).unwrap();
            fhm.insert('ヽ', 0b1010_0100).unwrap();
            fhm.insert('・', 0b1010_0101).unwrap();
            fhm.insert('ヲ', 0b1010_0110).unwrap();
            fhm.insert('ァ', 0b1010_0111).unwrap();
            fhm.insert('ィ', 0b1010_1000).unwrap();
            fhm.insert('ゥ', 0b1010_1001).unwrap();
            fhm.insert('ェ', 0b1010_1010).unwrap();
            fhm.insert('ォ', 0b1010_1011).unwrap();
            fhm.insert('ャ', 0b1010_1100).unwrap();
            fhm.insert('ュ', 0b1010_1101).unwrap();
            fhm.insert('ョ', 0b1010_1110).unwrap();
            fhm.insert('ッ', 0b1010_1111).unwrap();

            fhm.insert('ー', 0b1011_0000).unwrap();
            fhm.insert('ア', 0b1011_0001).unwrap();
            fhm.insert('イ', 0b1011_0010).unwrap();
            fhm.insert('ウ', 0b1011_0011).unwrap();
            fhm.insert('エ', 0b1011_0100).unwrap();
            fhm.insert('オ', 0b1011_0101).unwrap();
            fhm.insert('カ', 0b1011_0110).unwrap();
            fhm.insert('キ', 0b1011_0111).unwrap();
            fhm.insert('ク', 0b1011_1000).unwrap();
            fhm.insert('ケ', 0b1011_1001).unwrap();
            fhm.insert('コ', 0b1011_1010).unwrap();
            fhm.insert('サ', 0b1011_1011).unwrap();
            fhm.insert('シ', 0b1011_1100).unwrap();
            fhm.insert('ス', 0b1011_1101).unwrap();
            fhm.insert('セ', 0b1011_1110).unwrap();
            fhm.insert('ソ', 0b1011_1111).unwrap();

            fhm.insert('タ', 0b1100_0000).unwrap();
            fhm.insert('チ', 0b1100_0001).unwrap();
            fhm.insert('ツ', 0b1100_0010).unwrap();
            fhm.insert('テ', 0b1100_0011).unwrap();
            fhm.insert('ト', 0b1100_0100).unwrap();
            fhm.insert('ナ', 0b1100_0101).unwrap();
            fhm.insert('ニ', 0b1100_0110).unwrap();
            fhm.insert('ヌ', 0b1100_0111).unwrap();
            fhm.insert('ネ', 0b1100_1000).unwrap();
            fhm.insert('ノ', 0b1100_1001).unwrap();
            fhm.insert('ハ', 0b1100_1010).unwrap();
            fhm.insert('ヒ', 0b1100_1011).unwrap();
            fhm.insert('フ', 0b1100_1100).unwrap();
            fhm.insert('ヘ', 0b1100_1101).unwrap();
            fhm.insert('ホ', 0b1100_1110).unwrap();
            fhm.insert('マ', 0b1100_1111).unwrap();

            fhm.insert('ミ', 0b1101_0000).unwrap();
            fhm.insert('ム', 0b1101_0001).unwrap();
            fhm.insert('メ', 0b1101_0010).unwrap();
            fhm.insert('モ', 0b1101_0011).unwrap();
            fhm.insert('ヤ', 0b1101_0100).unwrap();
            fhm.insert('ユ', 0b1101_0101).unwrap();
            fhm.insert('ヨ', 0b1101_0110).unwrap();
            fhm.insert('ラ', 0b1101_0111).unwrap();
            fhm.insert('リ', 0b1101_1000).unwrap();
            fhm.insert('ル', 0b1101_1001).unwrap();
            fhm.insert('レ', 0b1101_1010).unwrap();
            fhm.insert('ロ', 0b1101_1011).unwrap();
            fhm.insert('ワ', 0b1101_1100).unwrap();
            fhm.insert('ン', 0b1101_1101).unwrap();
            fhm.insert('゛', 0b1101_1110).unwrap();
            fhm.insert('゜', 0b1101_1111).unwrap();

            // 10-bit wide symbols...

            fhm.insert('α', 0b1110_0000).unwrap();
            fhm.insert('ä', 0b1110_0001).unwrap();
            fhm.insert('β', 0b1110_0010).unwrap();
            fhm.insert('ε', 0b1110_0011).unwrap();
            fhm.insert('μ', 0b1110_0100).unwrap();
            fhm.insert('σ', 0b1110_0101).unwrap();
            fhm.insert('ρ', 0b1110_0110).unwrap();
            fhm.insert('ⓖ', 0b1110_0111).unwrap(); // kerned g
            fhm.insert('√', 0b1110_1000).unwrap();
            fhm.insert('ⁱ', 0b1110_1001).unwrap(); // ← superscript -1
            fhm.insert('ⓙ', 0b1110_1010).unwrap(); // kerned j
            fhm.insert('*', 0b1110_1011).unwrap();
            fhm.insert('¢', 0b1110_1100).unwrap();
            fhm.insert('₤', 0b1110_1101).unwrap(); // gnd upsidedown? lira?
            fhm.insert('ñ', 0b1110_1110).unwrap();
            fhm.insert('ö', 0b1110_1111).unwrap();

            fhm.insert('ⓟ', 0b1111_0000).unwrap();  // kerned p
            fhm.insert('ⓠ', 0b1111_0001).unwrap();  // kerned q
            fhm.insert('θ', 0b1111_0010).unwrap();
            fhm.insert('∞', 0b1111_0011).unwrap();
            fhm.insert('Ω', 0b1111_0100).unwrap();
            fhm.insert('ü', 0b1111_0101).unwrap();
            fhm.insert('Σ', 0b1111_0110).unwrap();
            fhm.insert('π', 0b1111_0111).unwrap();
            fhm.insert('ⓧ', 0b1111_1000).unwrap(); // x-bar
            fhm.insert('ⓨ', 0b1111_1001).unwrap(); // kerned y
            fhm.insert('千', 0b1111_1010).unwrap();
            fhm.insert('万', 0b1111_1011).unwrap();
            fhm.insert('両', 0b1111_1100).unwrap();
            fhm.insert('÷', 0b1111_1101).unwrap();
            fhm.insert('▓', 0b1111_1111).unwrap();
            fhm
        }, anchor: 0, overcast: 0
        }
    }

    fn cmbm(&mut self, bitmask: &u16, reg: &u16) { // Bitmask command... keeping in lieu as may have to add ParallelLCD1602's intrinsic bitmasking
        self.cmb(&(reg & bitmask));
    }
}

impl HD44780Util for I2CLcd1602 {
    fn cgload(&mut self, data: [[u8; 8]; 7]) {
        todo!()
    }

    // https://web.alfredstate.edu/faculty/weimandn/lcd/lcd_initialization/lcd_initialization_index.html
    fn init(&mut self) {
        arduino_hal::delay_ms(150);
        self.iiw(&0b00_0011);
        arduino_hal::delay_ms(10);
        self.iiw(&0b00_0011);
        arduino_hal::delay_us(150);
        self.iiw(&0b00_0011);
        arduino_hal::delay_us(150);
        self.iiw(&0b00_0010);
        arduino_hal::delay_us(100);

        self.iiw(&0b00_0010);
        arduino_hal::delay_us(50);
        self.iiw(&0b00_1000);
        arduino_hal::delay_us(100);

        self.iiw(&0b00_0000);
        arduino_hal::delay_us(50);
        self.iiw(&0b00_1000);
        arduino_hal::delay_us(100);

        self.iiw(&0b00_0000);
        arduino_hal::delay_us(50);
        self.iiw(&0b00_0001);
        arduino_hal::delay_ms(10);

        self.iiw(&0b00_0000);
        arduino_hal::delay_us(50);
        self.iiw(&0b00_0111);
        arduino_hal::delay_us(100);

        self.iiw(&0b00_0000);
        arduino_hal::delay_us(50);
        self.iiw(&0b00_1111);
        arduino_hal::delay_us(100);
        // self.cmd(&0b00_0010_1000); // DL=4D, N=2R, F=5x7
        // self.cmd(&0b00_0000_1000); // Display off
        // self.cmd(&0b00_0000_0001); // Display clear
        // self.cmd(&0b00_0000_0111); // I/D=inc, S=shift
        // self.cmd(&0b00_0000_1111); // C=on, B=blink

        ufmt::uwriteln!(&mut self.serial, "\n\nInitialised.\n\n");
    }

    fn affix(&mut self, row: u8, str: &str) {
        todo!()
    }

    fn marque(&mut self, iters: u8, graceful: bool) {
        todo!()
    }

    fn demo(&mut self, row0: &str, row1: &str, iters: u8, graceful: bool) {
        todo!()
    }
}

impl I2CBlOps for I2CLcd1602 {
    fn iiw(&mut self, data: &u8) {
       // ufmt::uwriteln!(&mut self.serial, ">> {:?}", bits8(*data));
        self.i2c.write(self.target, &[*data]);
        arduino_hal::delay_ms(8);
    }

    fn iir(&mut self) -> u8 {
        let mut buf = [0u8; 1];
        self.i2c.read(self.target, &mut buf);
        buf[0]
    }

    fn iirw(&mut self, data: &u8) -> u8 { // ← seems more efficient to use write_read compared to forcing manual 2x?
        let mut buf = [0u8; 1];
        self.i2c.write_read(self.target, &[*data], &mut buf);
        buf[0]
    }
}

impl Pinnable for I2CLcd1602 {
    fn pin(&mut self, ind: u8, state: bool) { // push pin, "pinned"... pin.
        self.buf |= u8::from(state) << ind;
    }

    fn bin(&mut self, bitmask: u8, state: u8) { // bitmask push pin... bin.
        self.buf &= state | !bitmask; // Affix all 0s to be set... must modify all bits in state that shouldn't be 0 to 1... all 0 bits in bitmask must be 1.
        self.buf |= state & bitmask; // Affix all 1s to be set... must modify all bits in state that shouldn't be 1 to 0... all 0 bits in bitmask should be 0.
    }

    fn fin(&mut self) -> u8 { // flush pin... fín
        let buf = self.buf;
        self.buf = self.iirw(&buf); // ← important! make sure to poll pin values when pinning.
        buf                              //              this is NOT for polling updated values for RDB or DTR.
    }

    fn gin(&mut self, ind: u8, state: bool) { // go pin; consolidates if just changing single pin... gin
        self.pin(ind, state);
        self.fin();
    }
}









