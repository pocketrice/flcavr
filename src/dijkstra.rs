use alloc::vec::Vec;

// Due to inability in instantiating a static map (phf has >1 access issues, lazy_static and hashbrown do not compile since RISC-based AVR lacks the instructions to run the spin crate)
// ...and AVR-HAL has no functionality for writing separate data to flash (cannot read during runtime anyway)
// ...instead load another FcHashMap into SRAM.

// Note this is not to find shortest Hamiltonian cycle, but rather just shortest path to visit all others (returning to base is trivial).

// Not quite a true adjacency matrix, but rather listing (tested) distances between every node and each other. Perk is can disable distances if needed.
// Encode the raw priority in the diagonal; symmetrical matrix for convenience (can encode other data such as scoring in lower triangle if strict SRAM!)

const INFINITY: u16 = u16::MAX;
const ATRIUM_TAX: u16 = 10;
const ELEV_TAX: [u16; 9] = [5, 6, 8, 12, 14, 16, 17, 19, 20]; // Lookup table for x=0-8 (compare ⌈10log²(2x)+5⌉ to ⌈5x⌉)
const DISTMAP: [[u16; 10]; 10] = {
    [
        [   25   , 20, 40, 25 + ELEV_TAX[1], 30 + ELEV_TAX[1], 2 + ELEV_TAX[2], 20 + ELEV_TAX[2], 40 + ELEV_TAX[2] + ATRIUM_TAX, 50 + ELEV_TAX[3], 25 + ELEV_TAX[8]], // Dropoff
        [20,                  5        , 20, 15, 10 + ELEV_TAX[1], 18 + ELEV_TAX[2], ELEV_TAX[2], 20 + ELEV_TAX[2] + ATRIUM_TAX, 40 + ELEV_TAX[3], 8 + ELEV_TAX[8]], // G010
        [40,20,                                   12                       , 5 + ELEV_TAX[1], 15 + ELEV_TAX[1], 65 + ELEV_TAX[2], 40 + ELEV_TAX[2], 5 + ELEV_TAX[2] + ATRIUM_TAX, 5 + ELEV_TAX[3], 20 + ELEV_TAX[8]], // Veranda
        [25+ELEV_TAX[1],15,5+ELEV_TAX[1],                           10                      , 60 + ELEV_TAX[1], 30 + ELEV_TAX[2], 45 + ELEV_TAX[2], 30 + ELEV_TAX[2], 30 + ELEV_TAX[3], 60 + ELEV_TAX[8]], // I315
        [30+ELEV_TAX[1],10+ELEV_TAX[1],15+ELEV_TAX[1],60+ELEV_TAX[1],                   18                 , 30 + ELEV_TAX[1], 50 + ELEV_TAX[1], 20 + ELEV_TAX[1], 20 + ELEV_TAX[2], 40 + ELEV_TAX[7]], // B888
        [2+ELEV_TAX[2],18+ELEV_TAX[2],65+ELEV_TAX[2],30+ELEV_TAX[2],30+ELEV_TAX[1],                       16                    , 15, 30 + ATRIUM_TAX, 60 + ELEV_TAX[1], 30 + ELEV_TAX[6]], // C148
        [20+ELEV_TAX[2],ELEV_TAX[2],40+ELEV_TAX[2],45+ELEV_TAX[2],50+ELEV_TAX[1],15,                                          16                        , 10, 35 + ELEV_TAX[1], 50 + ELEV_TAX[6]], // C024
        [40+ELEV_TAX[2]+ATRIUM_TAX,40+ELEV_TAX[2]+ATRIUM_TAX,5+ELEV_TAX[2]+ATRIUM_TAX,30+ELEV_TAX[2],20+ELEV_TAX[1],30+ATRIUM_TAX,10,     8     , 5 + ELEV_TAX[1], 25 + ELEV_TAX[5] + ATRIUM_TAX], // Atrium
        [50+ELEV_TAX[3],40+ELEV_TAX[3],5+ELEV_TAX[3],30+ELEV_TAX[3],20+ELEV_TAX[2],60+ELEV_TAX[1],35+ELEV_TAX[1],5+ELEV_TAX[1],                   18             , 5], // Y249
        [25+ELEV_TAX[8],8+ELEV_TAX[8],20+ELEV_TAX[8],60+ELEV_TAX[8],40+ELEV_TAX[7],30+ELEV_TAX[6],50+ELEV_TAX[6],25+ELEV_TAX[5]+ATRIUM_TAX,5,                    8          ] // F012
    ]
};
//c onst ROOM_DICT: [str; 10] = [*"Dropoff", *"G010", *"Veranda", *"I315", *"B888", *"C148", *"C024", *"Atrium", *"Y249", *"F012"];
const ROOM_DICT: [Node; 10] =  {
    [
        Node { index: 0, id: *"Dropoff" },
        Node { index: 1, id: *"G010" },
        Node { index: 2, id: *"Veranda" },
        Node { index: 3, id: *"I315" },
        Node { index: 4, id: *"B888" },
        Node { index: 5, id: *"C148" },
        Node { index: 6, id: *"C024" },
        Node { index: 7, id: *"Atrium" },
        Node { index: 8, id: *"Y249" },
        Node { index: 9, id: *"F012" }
    ]
};

pub struct Node {
    index: usize,
    id: str
}