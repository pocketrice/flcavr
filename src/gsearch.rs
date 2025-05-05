use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Reverse;
use priority_queue::PriorityQueue;
use crate::hash::NaiveXORHasherBuilder;
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

const EMPTY_NODE: Node = Node{ dm_index: 0, graph_index: 0 };

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
#[derive(Eq, Hash, PartialEq)]
#[derive(Debug)]
#[derive(Clone)]
pub struct Node {
    dm_index: usize,
    graph_index: usize,
}

impl Node {
    pub fn new(dm_index: usize, graph_index: usize) -> Node {
        Node { dm_index, graph_index }
    }
}

// ** Adapted from https://en.wikipedia.org/wiki/Dijkstras_algorithm#Pseudocode **
// Assume source vertex @ first. Also adds hard-coded access to DISTMAP and assumptions tailored to this use-case, so please generify for any lib use.
// In essence a fundamental assumption is that the graph is K_n (complete graph with n nodes).
// TODO: look into SIMD vectors (e.g. u8xN)

// ## Enough inefficiency here to last a lifetime... implicitly elided <'_>!!! smh my head o.o

// UPDATE 4/16/25: turns out Dijkstra is NOT the right choice for solving the Traveling Salesman's Problem. Ah.
pub fn k_dijkstra(inds: Vec<u8>) -> (Vec<u8>, Vec<Node>) { // <-- use u8 index over entire Node. Important to retain at least graph abstraction over assuming contiguous list of X elements (e.g. stringing hpaths together)
    let glen = inds.len();
    let graph = {
        let mut g = Vec::with_capacity(glen);
        for i in 0..glen {
            g.push(Node::new(*inds.get(i).unwrap() as usize, i));
        }
        g
    };

    let mut dist = vec![INFINITY; glen];
    let mut prev = vec![&EMPTY_NODE; glen];
    let mut pq = PriorityQueue::<&Node, Reverse<u8>, NaiveXORHasherBuilder>::with_capacity_and_default_hasher(glen);

    dist[0] = 0;
    pq.push(graph.first().unwrap(), Reverse(0));

    // for i in 1..glen {
    //     q.push(*graph.get(i).unwrap(), Reverse(INFINITY));
    // }

    while let Some((u, _)) = pq.pop() {
        for v in &graph {
            if v.graph_index != u.graph_index {
                let alt = dist[u.graph_index] + ext_dm(u.dm_index, v.dm_index, true);

                if alt < dist[v.graph_index] {
                    prev[v.graph_index] = u;
                    dist[v.graph_index] = alt;

                    pq.push(v, Reverse(alt));
                    //let v8 = &(v as u8); // jank :c
                    //q.change_priority(v8, Reverse(q.get_priority(v8).unwrap().0 + alt));
                }
            }
        }
    }

    (dist, prev.iter().cloned().cloned().collect())
}

// O(n^2*2^n) > O(n!)

// Held-Karp (exact but slooow), Lin-Kernighan (slower than 2-O but OK for symm)
// Use Two-Opt based on https://or.stackexchange.com/questions/6764/is-there-a-ranking-of-heuristics-for-the-travelling-salesman-problem -> https://link.springer.com/article/10.1007/s00453-002-0986-1
// You will get negative cost; this seems to be OK? Due to not using Euclidian distances but instead non-balanced metre weights.
pub fn two_opt(tour: &mut [usize], max_iters: usize) {
    let n = tour.len();
    let mut improved = true;
    let mut iters = 0;
    let mut cost: i32 = calc_tour_cost(tour) as i32;

    while improved && iters < max_iters {
        improved = false;
        for i in 0..n-1 {
            for j in (i+2)..n {
                let cost_delta: i32 = (ext_dm(i, j, true) as i32)
                    + (ext_dm(i + 1, (j + 1) % n, true) as i32)
                    - (ext_dm(i, i + 1, true) as i32)
                    - (ext_dm(j, (j + 1) % n, true) as i32);

                // If cost reduced, 2-opt swap.
                if cost_delta < 0 {
                    swap_edges(tour, i, j);
                    cost += cost_delta;
                    improved = true;
                }
            }
        }

        iters += 1;
    }
}

pub fn calc_tour_cost(tour: &[usize]) -> u32 {
    let mut cost = 0u32;
    for u in 0..tour.len() {
        let v = (u + 1) % tour.len();
        cost += ext_dm(u,v,true) as u32;
    }
    cost
}

// i → i+1, j → j+1 <-> i → j, i+1 → j+1
pub fn swap_edges(tour: &mut [usize], mut i: usize, mut j: usize) {
    i += 1;

    while i < j {
        tour.swap(i, j);

        i += 1;
        j -= 1;
    }
}

// H → L = edge length
// L → H = CGRAM direction; if uv swapped then also swap dir
// n → n = raw priority
pub fn ext_dm(u: usize, v: usize, opt_high: bool) -> u8 {
    if u == v || (u > v) ^ opt_high {
        if !opt_high { // TODO unbranch this (just for flipping dir symbol on opt_low)
            let sym = DISTMAP[u][v];

            match sym {
                CGR_UP => CGR_DOWN,
                CGR_DOWN => CGR_UP,
                CGR_RIGHT => CGR_LEFT,
                CGR_LEFT => CGR_RIGHT,
                CGR_UPRIGHT => CGR_DOWNLEFT,
                CGR_DOWNLEFT => CGR_UPRIGHT,
                CGR_UPLEFT => CGR_DOWNRIGHT,
                CGR_DOWNRIGHT => CGR_UPLEFT,
                _ => 0b1111_1111
            }

        } else {
            DISTMAP[u][v]
        }

    } else {
        DISTMAP[v][u]
    }
}