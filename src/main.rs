#![no_std]
#![no_main]
extern crate alloc;

mod binary_tree;
mod lcd1602;

use arduino_hal::prelude::*;
use arduino_hal::spi;
use heapless::{String, Vec, FnvIndexMap, BinaryHeap, IndexMap};
use panic_halt as _;
use crate::binary_tree::BinaryTree;

#[repr(C)] #[derive(Debug)]
struct PreEntry {
    room: u8,
    time: u16,
    flags: u8,
    desc: String<252>
}

#[repr(C)] #[derive(Debug)]
struct PostEntry {
    room: u8,
    prio: u8,
    eid: u8,
    oid: String<2>,
    status: u8,
    days: u16
}

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);

    // 256-byte preseg (0x00 = room # via dict, 0x01-0x02 = time in sec, 0x03 = ↴, 0x04-0xFF = 252-byte description in Huffman-encoded ASCII
    //                                                            (keep warm, keep cold, fragile/mix/spill, add utensils, allergen-prone, [3] raw priority)

    // 8-byte postseg (0x00 = room # via dict, 0x01 = priority, 0x02 = NID, 0x03-0x07 = ↴
    //                                                                                ([2] ASCII OID, [1] status, [2] days since 1/1/25)

    // You should write presegs to 3+1kb EEPROM (up to 12 entries), then write postsegs after every delivery to 6kb+2k SRAM (up to 768 entries → 6 buffer)
    let mut preseqs: Vec<PreEntry, 12> = Vec::new();
    let mut postseqs: Vec<PostEntry, 6> = Vec::new();

    let pretest = PreEntry {
        room: 0x36,
        time: 0xB33F,
        flags: 0b10001100,
        desc: String::from("Send with chamomile and honey, do not forget tea".parse().unwrap())
    };

    preseqs.push(pretest).expect("TODO: panic message");


    // Sort by distance/priority using Dijkstra

    // TODO


    // serial interface
    let mut serial = arduino_hal::default_serial!(dp, pins, 57600);

    // SPI interface
    let (mut spi, _) = arduino_hal::Spi::new(
        dp.SPI,
        pins.d52.into_output(),
        pins.d51.into_output(),
        pins.d50.into_pull_up_input(),
        pins.d53.into_output(),
        spi::Settings::default(),
    );

    loop {
        // send byte
        nb::block!(spi.send(0b00001111)).unwrap_infallible();

        // MISO -> MOSI, read data is same?
        let data = nb::block!(spi.read()).unwrap_infallible();

        ufmt::uwriteln!(&mut serial, "data: {}\r", data).unwrap_infallible();
        arduino_hal::delay_ms(1000);
    }
}

fn str2huffman(str: &str) -> &str {
    let freqmap = str2freq(str);

    let bt: BinaryTree<char> = BinaryTree::new();
}

fn str2freq(str: &str) -> FnvIndexMap<char, u8, 26> {
    let mut freqmap = FnvIndexMap::<char, u8, 26>::new();

    str.chars().for_each(|c| {
        freqmap.insert(c, freqmap.get(&c).unwrap_or_else(0) + 1).expect("Freqmap update failure");
    });

    freqmap
}



