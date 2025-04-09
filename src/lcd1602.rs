use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use arduino_hal::hal::port::Dynamic;
use arduino_hal::port::mode::Output;
use arduino_hal::port::Pin;
use core::ops::RangeBounds;
use embedded_hal::digital::{OutputPin, PinState};
use phf::phf_map;

// Adapted from https://www.waveshare.com/datasheet/LCD_en_PDF/LCD1602.pdf, https://cdn.sparkfun.com/assets/9/5/f/7/b/HD44780.pdf
static ASCII_TO_HD44780: phf::map::Map<char, &'static u8> = phf_map!([
    (&'!', 0b0010_0001), // ← Based on JIS X 0201 with JIS X 0208 mappings for ktka
    (&'"', 0b0010_0010),
    (&'#', 0b0010_0011),
    (&'$', 0b0010_0100),
    (&'%', 0b0010_0101),
    (&'&', 0b0010_0110),
    (&'\'', 0b0010_0111),
    (&'(', 0b0010_1000),
    (&')', 0b0010_1001),
    (&'*', 0b0010_1010),
    (&'+', 0b0010_1011),
    (&',', 0b0010_1100),
    (&'-', 0b0010_1101),
    (&'.', 0b0010_1110),
    (&'/', 0b0010_1101),

    (&'0', 0b0011_0000),
    (&'1', 0b0011_0001),
    (&'2', 0b0011_0010),
    (&'3', 0b0011_0011),
    (&'4', 0b0011_0100),
    (&'5', 0b0011_0101),
    (&'6', 0b0011_0110),
    (&'7', 0b0011_0111),
    (&'8', 0b0011_1000),
    (&'9', 0b0011_1001),
    (&':', 0b0011_1010),
    (&';', 0b0011_1011),
    (&'<', 0b0011_1100),
    (&'=', 0b0011_1101),
    (&'>', 0b0011_1110),
    (&'?', 0b0011_1101),

    (&'@', 0b0100_0000),
    (&'A', 0b0100_0001),
    (&'B', 0b0100_0010),
    (&'C', 0b0100_0011),
    (&'D', 0b0100_0100),
    (&'E', 0b0100_0101),
    (&'F', 0b0100_0110),
    (&'G', 0b0100_0111),
    (&'H', 0b0100_1000),
    (&'I', 0b0100_1001),
    (&'J', 0b0100_1010),
    (&'K', 0b0100_1011),
    (&'L', 0b0100_1100),
    (&'M', 0b0100_1101),
    (&'N', 0b0100_1110),
    (&'O', 0b0100_1111),

    (&'P', 0b0101_0000),
    (&'Q', 0b0101_0001),
    (&'R', 0b0101_0010),
    (&'S', 0b0101_0011),
    (&'T', 0b0101_0100),
    (&'U', 0b0101_0101),
    (&'V', 0b0101_0110),
    (&'W', 0b0101_0111),
    (&'X', 0b0101_1000),
    (&'Y', 0b0101_1001),
    (&'Z', 0b0101_1010),
    (&'[', 0b0101_1011),
    (&'¥', 0b0101_1100),
    (&']', 0b0101_1101),
    (&'^', 0b0101_1110),
    (&'_', 0b0101_1111),

    (&'`', 0b0110_0000),
    (&'a', 0b0110_0001),
    (&'b', 0b0110_0010),
    (&'c', 0b0110_0011),
    (&'d', 0b0110_0100),
    (&'e', 0b0110_0101),
    (&'f', 0b0110_0110),
    (&'g', 0b0110_0111),
    (&'h', 0b0110_1000),
    (&'i', 0b0110_1001),
    (&'j', 0b0110_1010),
    (&'k', 0b0110_1011),
    (&'l', 0b0110_1100),
    (&'m', 0b0110_1101),
    (&'n', 0b0110_1110),
    (&'o', 0b0110_1111),

    (&'p', 0b0111_0000),
    (&'q', 0b0111_0001),
    (&'r', 0b0111_0010),
    (&'s', 0b0111_0011),
    (&'t', 0b0111_0100),
    (&'u', 0b0111_0101),
    (&'v', 0b0111_0110),
    (&'w', 0b0111_0111),
    (&'x', 0b0111_1000),
    (&'y', 0b0111_1001),
    (&'z', 0b0111_1010),
    (&'{', 0b0111_1011),
    (&'|', 0b0111_1100),
    (&'}', 0b0111_1101),
    (&'→', 0b0111_1110),
    (&'←', 0b0111_1111),

    // Skip 0b1000XXXX.

    (&' ', 0b1010_0000),
    (&'。', 0b1010_0001),
    (&'「', 0b1010_0010),
    (&'」', 0b1010_0011),
    (&'ヽ', 0b1010_0100),
    (&'・', 0b1010_0101),
    (&'ヲ', 0b1010_0110),
    (&'ァ', 0b1010_0111),
    (&'ィ', 0b1010_1000),
    (&'ゥ', 0b1010_1001),
    (&'ェ', 0b1010_1010),
    (&'ォ', 0b1010_1011),
    (&'ャ', 0b1010_1100),
    (&'ュ', 0b1010_1101),
    (&'ョ', 0b1010_1110),
    (&'ッ', 0b1010_1111),

    (&'ー', 0b1011_0000),
    (&'ア', 0b1011_0001),
    (&'イ', 0b1011_0010),
    (&'ウ', 0b1011_0011),
    (&'エ', 0b1011_0100),
    (&'オ', 0b1011_0101),
    (&'カ', 0b1011_0110),
    (&'キ', 0b1011_0111),
    (&'ク', 0b1011_1000),
    (&'ケ', 0b1011_1001),
    (&'コ', 0b1011_1010),
    (&'サ', 0b1011_1011),
    (&'シ', 0b1011_1100),
    (&'ス', 0b1011_1101),
    (&'セ', 0b1011_1110),
    (&'ソ', 0b1011_1111),

    (&'タ', 0b1100_0000),
    (&'チ', 0b1100_0001),
    (&'ツ', 0b1100_0010),
    (&'テ', 0b1100_0011),
    (&'ト', 0b1100_0100),
    (&'ナ', 0b1100_0101),
    (&'ニ', 0b1100_0110),
    (&'ヌ', 0b1100_0111),
    (&'ネ', 0b1100_1000),
    (&'ノ', 0b1100_1001),
    (&'ハ', 0b1100_1010),
    (&'ヒ', 0b1100_1011),
    (&'フ', 0b1100_1100),
    (&'ヘ', 0b1100_1101),
    (&'ホ', 0b1100_1110),
    (&'マ', 0b1100_1111),

    (&'ミ', 0b1101_0000),
    (&'ム', 0b1101_0001),
    (&'メ', 0b1101_0010),
    (&'モ', 0b1101_0011),
    (&'ヤ', 0b1101_0100),
    (&'ユ', 0b1101_0101),
    (&'ヨ', 0b1101_0110),
    (&'ラ', 0b1101_0111),
    (&'リ', 0b1101_1000),
    (&'ル', 0b1101_1001),
    (&'レ', 0b1101_1010),
    (&'ロ', 0b1101_1011),
    (&'ワ', 0b1101_1100),
    (&'ン', 0b1101_1101),
    (&'゛', 0b1101_1110),
    (&'゜', 0b1101_1111),

    // 10-bit wide symbols...

    (&'α', 0b1110_0000),
    (&'ä', 0b1110_0001),
    (&'β', 0b1110_0010),
    (&'ε', 0b1110_0011),
    (&'μ', 0b1110_0100),
    (&'σ', 0b1110_0101),
    (&'ρ', 0b1110_0110),
    // fancy g
    (&'√', 0b1110_1000),
    (&'ⁱ', 0b1110_1001), // ← superscript -1
    // fancy j
    (&'*', 0b1110_1011),
    (&'¢', 0b1110_1100),
    // gnd upsidedown?
    (&'ñ', 0b1110_1110),
    (&'ö', 0b1110_1111),

    // fancy p
    // fancy q
    (&'θ', 0b1111_0001),
    (&'∞', 0b1111_0010),
    (&'Ω', 0b1111_0011),
    (&'ü', 0b1111_0100),
    (&'Σ', 0b1111_0101),
    (&'π', 0b1111_0110),
    // x-bar
    //fancy y
    (&'千', 0b1111_1010),
    (&'万', 0b1111_1011),
     // 亿?
    (&'÷', 0b1111_1101),
    (&'▓', 0b1111_1111),
]);

pub struct Lcd1602 {
    rs: Pin<Output>,
    rw: Pin<Output>,
    en: Pin<Output>,
    db: [Pin<Dynamic>; 8] // ← NOTE... little endian (0-7)
}

impl Lcd1602 {
    pub fn new(rs: Pin<Output>, rw: Pin<Output>, en: Pin<Output>, db: [Pin<Dynamic>; 8]) -> Lcd1602 {
        Self { rs, rw, en, db }
    }

    pub fn register(&mut self, mut byte: u8) { // ← write to DB register
        for i in 0..8 {
            let dbi = &mut self.db[i];
            dbi.set_state(PinState::from(byte & 0x1 == 1));
            byte >>= 1;
        }
    }

    pub fn dbx<R: RangeBounds<usize>>(&mut self, i: R) -> u8 { // ← utility for bitmasking ith register value. Range to save accesses if several needed.
        let dbs: &[Pin<Dynamic>] = self.db[i];
        let mut x = 0u8;

        for db in dbs {
            x |= u8::from(db.is_high());
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
    }

    pub fn enp_then_bus(&mut self) {
        self.enp();
        self.bus();
    }

    pub fn cmd(&mut self, reg: &u16) { // ← the "skip pleasantries and go for it" option
        self.register((reg & 0b00_1111_1111) as u8);
        self.rw.set_state(PinState::from((reg & 0b01_0000_0000) != 0));
        self.rs.set_state(PinState::from((reg & 0b10_0000_0000) != 0));
        self.enp_then_bus();
    }

    pub fn rdb(&mut self) -> bool { // ← Read B(usy) flag
        self.rs.set_low();
        self.rw.set_high();
        self.enp();

        self.db[4].is_high().unwrap()
    }

    pub fn clr(&mut self) { // ← screen clear
        self.cmd(&0b00_0000_0001);
    }

    pub fn ret(&mut self) { // ← cursor return
        self.cmd(&(0b00_0000_0010 | (self.dbx(&0) as u16)))
    }

    pub fn ems(&mut self, id: bool, s: bool) { // ← entry mode set
        self.cmd(&(0b00_0000_0100 | ((id as u16) << 1) | (s as u16)));
    }

    pub fn dsw(&mut self, d: bool, c: bool, b: bool) { // ← display switch
        self.cmd(&(0b00_0000_1000 | ((d as u16) << 2) | ((c as u16) << 1) | (b as u16)));
    }

    pub fn cds(&mut self, sc: bool, rl: bool) { // ← cursor/display shift
        self.cmd(&(0b00_0001_0000 | ((sc as u16) << 3) | ((rl as u8) << 2 | self.dbx(0..=1)) as u16));
    }

    pub fn fns(&mut self, dl: bool, n: bool, f: bool) { // ← function set
        self.cmd(&(0b00_0010_0000 | ((dl as u16) << 4) | ((n as u16) << 3) | ((f as u8) << 2 | self.dbx(0..=1)) as u16));
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
                self.cgs(&symind << 3 | symline);
                self.dtw(sym[symline]) // NOTE: MS3B irrelevant, but also masking is redundant.
            }

            // Protect 8th line for cursor
            self.cgs(&symind << 3 | 0b000111usize);
            self.dtw(0x0);
        }
    }

    pub fn init(&mut self) {
        self.fns(true, false, false); // 00 001100**
        self.dsw(true, true, true);   // 00 00001110
    }


    pub fn disp_str(&mut self, str: &str) {
        self.symv(str.chars().map(|c| ASCII_TO_HD44780.get(c).unwrap()))
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
