use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Reverse;

// Due to inability in instantiating a static map (phf has >1 access issues, lazy_static and hashbrown do not compile since RISC-based AVR lacks the instructions to run the spin crate)
// ...and AVR-HAL has no functionality for writing separate data to flash (cannot read during runtime anyway)
// ...instead load another FcHashMap into SRAM.

// Note this is not to find shortest Hamiltonian cycle, but rather just shortest path to visit all others (returning to base is trivial).

// fibonacci_heap gives faster amortised θ runtime, but std-only.

// Not quite a true adjacency matrix, but rather listing (tested) distances between every node and each other. Perk is can disable distances if needed.
// Encode the raw priority in the diagonal; symmetrical matrix for convenience (can encode other data such as scoring in lower triangle if strict SRAM!)
// UPDATE 4/15/25 ~ the lower triangle will be used for storing relative direction from each node to the other (lest we use another 200B+ matrix...); these are raw CGROM addresses.
//          note! ~ due to only having one triangle assume it is TOP --> SIDE (mod++ to flip).
// CGRAM [1] = 0b0000_0000 = UP
// CGR2 = DOWN
// CGR3 = UP_LEFT
// CGR4 = DOWN_RIGHT
// CGR5 = UP_RIGHT
// CGR6 = DOWN_LEFT
// 0b0111_1110 = RIGHT
// 0b0111_1111 = LEFT

// TODO investigate if u8 is feasible (distances + priority?)

const CGR_UP: u8 = 0b0000_0000;
const CGR_DOWN: u8 = 0b0000_0001;
const CGR_LEFT: u8 = 0b0111_1110;
const CGR_RIGHT: u8 = 0b0111_1111;
const CGR_UPLEFT: u8 = 0b0000_0010;
const CGR_DOWNRIGHT: u8 = 0b0000_0011;
const CGR_UPRIGHT: u8 = 0b0000_0100;
const CGR_DOWNLEFT: u8 = 0b0000_0101;

const INFINITY: u8 = u8::MAX;
const ATRIUM_TAX: u8 = 10;
const ELEV_TAX: [u8; 9] = [5, 6, 8, 12, 14, 16, 17, 19, 20]; // Lookup table for x=0-8 (compare ⌈10log²(2x)+5⌉ to ⌈5x⌉)
const DISTMAP: [[u8; 10]; 10] = {
    [
        [            25           , 20, 40, 25 + ELEV_TAX[1], 30 + ELEV_TAX[1], 2 + ELEV_TAX[2], 20 + ELEV_TAX[2], 40 + ELEV_TAX[2] + ATRIUM_TAX, 50 + ELEV_TAX[3], 25 + ELEV_TAX[8]],                                   // Dropoff
        [CGR_RIGHT,                     5        , 20, 15, 10 + ELEV_TAX[1], 18 + ELEV_TAX[2], ELEV_TAX[2], 20 + ELEV_TAX[2] + ATRIUM_TAX, 40 + ELEV_TAX[3], 8 + ELEV_TAX[8]],                                           // G010
        [CGR_RIGHT, CGR_RIGHT,                       12                       , 5 + ELEV_TAX[1], 15 + ELEV_TAX[1], 65 + ELEV_TAX[2], 40 + ELEV_TAX[2], 5 + ELEV_TAX[2] + ATRIUM_TAX, 5 + ELEV_TAX[3], 20 + ELEV_TAX[8]], // Veranda
        [CGR_DOWNRIGHT, CGR_DOWN, CGR_DOWNLEFT,                  10                      , 60 + ELEV_TAX[1], 30 + ELEV_TAX[2], 45 + ELEV_TAX[2], 30 + ELEV_TAX[2], 30 + ELEV_TAX[3], 60 + ELEV_TAX[8]],                  // I315
        [CGR_UPRIGHT, CGR_UPRIGHT, CGR_UPLEFT, CGR_UP,                    18                 , 30 + ELEV_TAX[1], 50 + ELEV_TAX[1], 20 + ELEV_TAX[1], 20 + ELEV_TAX[2], 40 + ELEV_TAX[7]],                                // B888
        [CGR_UP, CGR_UPLEFT, CGR_UPLEFT, CGR_UPLEFT, CGR_UPLEFT,                    16                    , 15, 30 + ATRIUM_TAX, 60 + ELEV_TAX[1], 30 + ELEV_TAX[6]],                                                    // C148
        [CGR_UPRIGHT, CGR_UP, CGR_UPLEFT, CGR_UP, CGR_UPLEFT, CGR_RIGHT,                     16                        , 10, 35 + ELEV_TAX[1], 50 + ELEV_TAX[6]],                                                        // C024
        [CGR_UPRIGHT, CGR_UPRIGHT, CGR_UP, CGR_UPRIGHT, CGR_UP, CGR_RIGHT, CGR_RIGHT,                  8     , 5 + ELEV_TAX[1], 25 + ELEV_TAX[5] + ATRIUM_TAX],                                                          // Atrium
        [CGR_UPRIGHT, CGR_UPRIGHT, CGR_UP, CGR_UPRIGHT, CGR_UPRIGHT, CGR_UPRIGHT, CGR_UPRIGHT, CGR_UPRIGHT,     18             , 5],                                                                                     // Y249
        [CGR_UPRIGHT, CGR_UP, CGR_UPLEFT, CGR_UP, CGR_UP, CGR_UPRIGHT, CGR_UP, CGR_UPLEFT, CGR_UPLEFT,                        8          ]                                                                               // F012
    ]
};

const ROOM_DICT: [str; 10] = [*"Dropoff", *"G010", *"Veranda", *"I315", *"B888", *"C148", *"C024", *"Atrium", *"Y249", *"F012"];

// const ROOM_DICT: [Node; 10] =  {
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
//
// pub struct Node {
//     index: usize,
//     id: str
// }

// ** Adapted from https://en.wikipedia.org/wiki/Dijkstras_algorithm#Pseudocode **
// Assume source vertex @ first. Also adds hard-coded access to DISTMAP and assumptions tailored to this use-case, so please generify for any lib use.
// In essence a fundamental assumption is that the graph is K_n (complete graph with n nodes).
// TODO: look into SIMD vectors (e.g. u8xN)

pub fn k_dijkstra(graph: Vec<u8>) -> (Vec<u8>, Vec<u8>) { // <-- use u8 index over entire Node. Important to retain at least graph abstraction over assuming contiguous list of X elements (e.g. stringing hpaths together)
    let glen = graph.len();
    let mut q = priority_queue::PriorityQueue::with_capacity_and_default_hasher(glen);
    let mut dist = vec![INFINITY; glen];
    let mut prev = vec![0; glen];

    dist.insert(0, 0);
    q.insert(graph.first(), 0);

    for i in 1..glen {
        q.insert(graph.get(i).unwrap(), Reverse(INFINITY));
    }

    while !q.is_empty() {
        let (u, _) = q.peek().unwrap();

        for v in 0..glen {
            if v != u {
                let alt = dist[v] + ext_dm(u, v, true);

                if alt < dist[v] {
                    prev.insert(v, u);
                    dist.insert(v, alt);
                    q.change_priority_by(v, |p| Reverse(p.0 - alt));
                }
            }
        }
    }

    (dist, prev)
}

// H → L = edge length
// L → H = CGRAM direction
// n → n = raw priority
fn ext_dm(u: u8, v: u8, opt_high: bool) -> u8 {
    if u == v || (u < v && opt_high) {
        DISTMAP[u as usize][v as usize]
    } else {
        DISTMAP[v as usize][u as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ext_edge_nof() {
        assert_eq!(ext_dm(1,0,true), 20);
    }

    #[test]
    fn test_ext_edge_f() {
        assert_eq!(ext_dm(2,5,true), 65 + ELEV_TAX[2]);
    }

    #[test]
    fn test_ext_prio() {
        assert_eq!(ext_dm(3,3,false), 10);
    }

    #[test]
    fn test_ext_sym_nof() {
        assert_eq!(ext_dm(0,8,false), CGR_UPRIGHT);
    }

    #[test]
    fn test_ext_sym_f() {
        assert_eq!(ext_dm(8,0,false), CGR_UPRIGHT);
    }
}