// Editor's note: I feel like this oversteps into the Java OOP design paradigm... it might've been
//                better to make separate modules (e.g. ext, algo) n' such. Fix this later perhaps.
//                (or never. Up to you ya lovely programmer ^^)

use crate::bitops::{comp24, decomp24};
use crate::gsearch::ext_dm;
use crate::DeliveryStatus;
use arduino_hal::eeprom::OutOfBoundsError;
use arduino_hal::Eeprom;
use fchashmap::FcHashMap;
// trait Bytable {
//     fn bytize(&self) -> &[u8];
//     fn debytize(byt: &[u8]) -> Self;
// }

const ROOM_DICT: [&str; 10] = ["Dropoff", "G010", "Veranda", "I315", "B888", "C148", "C024", "Atrium", "Y249", "F012"];
    //     [
    //         Node { index: 0, id: *"Dropoff" },
    //         Node { index: 1, id: *"G010" },
    //         Node { index: 2, id: *"Veranda" },
    //         Node { index: 3, id: *"I315" },
    //         Node { index: 4, id: *"B888" },
    //         Node { index: 5, id: *"C148" },
    //         Node { index: 6, id: *"C024" },
    //         Node { index: 7, id: *"Atrium" },
    //         Node { index: 8, id: *"Y249" },
    //         Node { index: 9, id: *"F012" }
    //     ]
    // };

trait MemPointer {
    fn reset(&mut self) -> u16;
    fn set(&mut self, new_addr: u16) -> Result<(), OutOfBoundsError>;
    fn update<F>(&mut self, f: F) -> Result<u16, OutOfBoundsError>
    where
        F: FnOnce(u16) -> u16;
}

pub struct Preentry {
    dict: u8,
    ttd: u16,
    flags: u8,
    desc: [u8; 252] // chars are Unicode 4B and don't map easily to HD44780 CGROM, hence assume pre-mapped.
}

pub struct Postentry {
    dict: u8,
    prio: u8,
    eid: u8,
    oid: u8, // Use lookup table
    dst: u8,
    since: u32 // Assume u24
}

impl Preentry {
    fn bytize(&self) -> [u8; 256] {
        let mut arr = [0u8; 256];
        arr[..4].copy_from_slice(&[self.dict, ((self.ttd & 0xFF00) >> 8) as u8, (self.ttd & 0x00FF) as u8, self.flags]);
        arr[4..].copy_from_slice(&self.desc);
        arr

    }

    fn debytize(byt: [u8; 256]) -> Self {
        Self { dict: byt[0], ttd: (0u16 + byt[1] as u16) << 8 + byt[2] as u16, flags: byt[3], desc: byt[4..].try_into().unwrap() }
    }
}

impl Postentry {
    fn bytize(&self) -> [u8; 8] {
        let mut arr = [self.dict, self.prio, self.eid, self.oid, self.dst, 0, 0, 0];
        arr[5..].copy_from_slice(&decomp24(self.since));
        arr
    }

    fn debytize(byt: [u8; 8]) -> Self {
        Self { dict: byt[0], prio: byt[1], eid: byt[2], oid: byt[3], dst: byt[4], since: comp24(byt[5..].try_into().unwrap()) }
    }
}



struct AddressPointer {
    addr: u16,
    lbound: u16,
    ubound: u16
}

impl MemPointer for AddressPointer {
    fn reset(&mut self) -> u16 {
        let old = self.addr;
        self.addr = self.lbound;
        old
    }

    fn set(&mut self, new_addr: u16) -> Result<(), OutOfBoundsError> {
        if new_addr < self.lbound || new_addr > self.ubound {
            return Err(OutOfBoundsError);
        }

        self.addr = new_addr;
        Ok(())
    }

    fn update<F>(&mut self, f: F) -> Result<u16, OutOfBoundsError>
    where
        F: FnOnce(u16) -> u16
    {
        let res = self.set(f(self.addr));

        res.map(|_| self.addr)
    }
}

impl AddressPointer {
    fn default(lbound: u16, ubound: u16) -> Self {
        Self { lbound, ubound, addr: lbound }
    }
}

struct RelAddressPointer {
    offset: u16,
    addr: u16,
    len: u16
}

impl MemPointer for RelAddressPointer {
    fn reset(&mut self) -> u16 {
        let old = self.addr;
        self.addr = 0x0;
        old
    }

    fn set(&mut self, rel_addr: u16) -> Result<(), OutOfBoundsError> {
        if rel_addr > self.offset || rel_addr < self.offset + self.len {
            return Err(OutOfBoundsError);
        }

        self.addr = rel_addr;
        Ok(())
    }

    fn update<F>(&mut self, f: F) -> Result<u16, OutOfBoundsError>
    where
        F: FnOnce(u16) -> u16
    {
        let res = self.set(f(self.addr));

        res.map(|_| self.addr)
    }
}

impl RelAddressPointer {
    fn set_static(&mut self, abs_addr: u16) -> Result<(), OutOfBoundsError> {
        self.set(abs_addr - self.offset)
    }
}

pub struct EntryManager {
    pre_pointer: AddressPointer, // Open write addr; addresses first 3 sectors (0x0-0xBFF or 0-3071)
    post_pointer: AddressPointer, // Open write addr; addresses 90% of last sector (0xC00-0xF9B or 3072-3995)
    eeprw: Eeprom,
    ecounter: u8
    //pub(crate) serial: Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>> // evil floating point bit level hacking—— sorry, actually just me not understanding basic Rust lifetimes/borrowing lol -△-
}

impl EntryManager {
    pub fn new(eeprw: Eeprom) -> Self {
        Self {
            pre_pointer: AddressPointer::default(0x0, 0xBFF),
            post_pointer: AddressPointer::default(0xC00, 0xF9B),
            eeprw,
            ecounter: 0
           // serial
        }
    }

    // FIXME
    pub fn load_sample(&mut self, mapper: &FcHashMap<char, u8, 256>) {
        let p0 = const_dat(0, 0, 0, "Chamomile please! Keep warm.", mapper);
        self.write_pre(&p0);

        // let b0 = transmute_dat(&p0, 0, 0, DeliveryStatus::Absent);
        // self.write_post(&b0);

        // let p1 = const_dat(1, 0, 0, "Keep warm; steady.", mapper);
        // self.write_pre(&p1);
        //
        // let p2 = const_dat(2, 0, 0, "₤20 continental.", mapper);
        // self.write_pre(&p2);
        //
        // let p3 = const_dat(3, 0, 0, "No almonds, add utensils.", mapper);
        // self.write_pre(&p3);
        //
        // let p4 = const_dat(4, 0, 0, "Patient discharged, void.", mapper);
        // self.write_pre(&p4);
        //
        // let p5 = const_dat(5, 0, 0, "Veranda package to-go.", mapper);
        // self.write_pre(&p5);

        // let p6 = const_dat(6, 0, 0, "ニヲサーネロ。", mapper);
        // self.write_pre(&p6);
        //
        // let p7 = const_dat(7, 0, 0, "N/A", mapper);
        // self.write_pre(&p7);
        //
        // let p8 = const_dat(8, 0, 0, "Shellfish allergy, fragile.", mapper);
        // self.write_pre(&p8);

        // let p9 = const_dat(9, 0, 0, "Hand deliver triple-wrapped.", mapper);
        // self.write_pre(&p9);
    }

    // TODO FIX V
    pub fn read_pre(&mut self, index: u8, v: u8) -> (&str, [u8; 28], u8, u8) { // dictname, description, CGROM symbol, distance
        let addr = self.pre_pointer.lbound + (index * 8) as u16;
        let mut buf = [0u8; 256];
        self.eeprw.read(addr, &mut buf).expect("help :(");

        let dictname = ROOM_DICT[v as usize];
        let mut desc = [0u8; 28];
        desc.copy_from_slice(&buf[4..32]);


        (dictname, desc, ext_dm(index as usize, v as usize, false), ext_dm(index as usize, v as usize, true))
    }

    fn write_post(&mut self, post: &Postentry) { // 1KiB = 1024B = <u16::MAX
        // Allotted EEPROM space is upper 900 bytes of last sector or 0xC00-0xF9B (3072-3995) = 924B
        //eepwrite(&mut self.post_pointer, &post.bytize(), &mut self.eeprw);
        self.eeprw.write(0xC00, &post.bytize());
    }

    fn write_pre(&mut self, pre: &Preentry) {
        // Allotted EEPROM space is first 3 sectors or 0x0-0xBFF.
        eepwrite(&mut self.pre_pointer, &pre.bytize(), &mut self.eeprw);
        self.ecounter += 1;
    }

    // TODO remove this and add way to properly read EEPROM
    // pub fn eepread(&self, addr: u16, buf: &mut [u8]) {
    //     self.eeprw.read(addr, buf);
    // }
    //
    pub fn eepread(&self, addr: u16) -> u8 {
        self.eeprw.read_byte(addr)
    }
}

fn eepwrite(ptr: &mut AddressPointer, buf: &[u8], eeprw: &mut Eeprom) {
    let offset = ptr.addr;
    let blen = buf.len() as u16;

    if offset < ptr.lbound || offset > (ptr.ubound - blen) {
       // ufmt::uwriteln!(serial, "ABORT: EEPROM write @ {:#02x}-{:#02x} out of bounds.", offset, offset + blen - 1);
    } else {
        if eeprw.read_byte(offset) != 0 { // NOTE: smallest chance of off-by-1 error or data may be intended to be 0 there. Fix if needed.
            //ufmt::uwriteln!(serial, "WARN: EEPROM write @ {:#02x}-{:#02x} potentially overwriting data.", offset, offset + blen - 1);
        }

        let status = eeprw.write(offset, buf);
        ptr.update(|a| a + offset).unwrap();

        // match status {
        //     Ok(_) => ufmt::uwriteln!(serial, "OK: EEPROM write @ {:#02x}-{:#02x} successful.", offset, offset + blen - 1).unwrap_infallible(),
        //     Err(_) => ufmt::uwriteln!(serial, "ERR: EEPROM write @ {:#02x}-{:#02x} failed.", offset, offset + blen - 1).unwrap_infallible(),
        // }
    }
}

fn const_dat(dict: u8, ttd: u16, flags: u8, desc: &str, mapper: &FcHashMap<char, u8, 256>) -> Preentry {
    Preentry { dict, ttd, flags, desc: {
        let mut end = [0u8; 252];

        for (i,c) in desc.chars().take(252).enumerate() {
            end[i] = *mapper.get(&c).unwrap_or(&0b1111_1111);
        }

        end
    } }
}

fn transmute_dat(pre: &Preentry, eid: u8, oid: u8, dst: DeliveryStatus) -> Postentry {
    Postentry { dict: pre.dict, prio: (pre.flags & 0x7) + ((pre.ttd as f32 / u16::MAX as f32) * 10.0) as u8, eid, oid, dst: dst as u8, since: 0u32 }
}