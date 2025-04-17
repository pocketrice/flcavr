// Editor's note: I feel like this oversteps into the Java OOP design paradigm... it might've been
//                better to make separate modules (e.g. ext, algo) n' such. Fix this later perhaps.
//                (or never. Up to you ya lovely programmer ^^)

use crate::bitops::{comp24, decomp24};
use crate::DeliveryStatus;
use alloc::rc::Rc;
use alloc::vec::Vec;
use arduino_hal::eeprom::OutOfBoundsError;
use arduino_hal::hal::port::{PE0, PE1};
use arduino_hal::port::mode::{Input, Output};
use arduino_hal::port::Pin;
use arduino_hal::prelude::_unwrap_infallible_UnwrapInfallible;
use arduino_hal::{Eeprom, Usart};
use avr_device::atmega2560::USART0;
use core::cell::{RefCell, RefMut};
use fchashmap::FcHashMap;

// trait Bytable {
//     fn bytize(&self) -> &[u8];
//     fn debytize(byt: &[u8]) -> Self;
// }

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
    serial: Rc<RefCell<Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>>> // evil floating point bit level hacking—— sorry, actually just me not understanding basic Rust lifetimes/borrowing lol -△-
}

impl EntryManager {
    pub fn new(eeprw: Eeprom, serial: Rc<RefCell<Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>>>) -> Self {
        Self {
            pre_pointer: AddressPointer::default(0x0, 0xBFF),
            post_pointer: AddressPointer::default(0xC00, 0xF9B),
            eeprw,
            serial
        }
    }

    pub fn load_sample(&mut self) {

    }

    fn write_post(&mut self, post: Postentry) { // 1KiB = 1024B = <u16::MAX
        // Allotted EEPROM space is upper 900 bytes of last sector or 0xC00-0xF9B (3072-3995) = 924B
        eepwrite(&mut self.post_pointer, &post.bytize(), &mut self.eeprw, Rc::get_mut(&mut self.serial).unwrap().borrow_mut());
    }

    fn write_pre(&mut self, pre: Preentry) {
        // Allotted EEPROM space is first 3 sectors or 0x0-0xBFF.
        eepwrite(&mut self.pre_pointer, &pre.bytize(), &mut self.eeprw, Rc::get_mut(&mut self.serial).unwrap().borrow_mut());
    }
}

fn eepwrite(ptr: &mut AddressPointer, buf: &[u8], eeprw: &mut Eeprom, mut serial: RefMut<Usart<USART0, Pin<Input, PE0>, Pin<Output, PE1>>>) {
    let offset = ptr.addr;
    let blen = buf.len() as u16;

    if offset < ptr.lbound || offset > (ptr.ubound - blen) {
        ufmt::uwriteln!(serial, "ABORT: EEPROM write @ {:#02x}-{:#02x} out of bounds.", offset, offset + blen - 1);
    } else {
        if eeprw.read_byte(offset) != 0 { // NOTE: smallest chance of off-by-1 error or data may be intended to be 0 there. Fix if needed.
            ufmt::uwriteln!(serial, "WARN: EEPROM write @ {:#02x}-{:#02x} potentially overwriting data.", offset, offset + blen - 1);
        }

        let status = eeprw.write(offset, buf);
        ptr.update(|a| a + offset).unwrap();

        match status {
            Ok(_) => ufmt::uwriteln!(serial, "OK: EEPROM write @ {:#02x}-{:#02x} successful.", offset, offset + blen - 1).unwrap_infallible(),
            Err(_) => ufmt::uwriteln!(serial, "ERR: EEPROM write @ {:#02x}-{:#02x} failed.", offset, offset + blen - 1).unwrap_infallible(),
        }
    }
}

fn const_dat(dict: u8, ttd: u16, flags: u8, desc: &str, mapper: &FcHashMap<char, u8, 256>) -> Preentry {
    Preentry { dict, ttd, flags, desc: desc.chars().map(|c| *mapper.get(&c).unwrap_or(&0b1111_1111)).collect::<Vec<_>>().try_into().unwrap() }
}

fn transmute_dat(pre: Preentry, eid: u8, oid: u8, dst: DeliveryStatus) -> Postentry {
    Postentry { dict: pre.dict, prio: (pre.flags & 0x7) + ((pre.ttd as f32 / u16::MAX as f32) * 10.0) as u8, eid, oid, dst: dst as u8, since: 0u32 }
}