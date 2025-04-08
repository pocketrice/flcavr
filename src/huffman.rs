#![no_std]

use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use std::ops::Index;
use ux::u24;
use crate::binary_tree::BinaryTree;

fn is_alpha(str: &str) -> bool {
    str.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

fn str2freq(str: &str) -> BTreeMap<char, u8> {
    let mut freqmap: BTreeMap<char, u8> = BTreeMap::default();

    str.chars().for_each(|c| {
        freqmap.insert(c, freqmap.get(&c).unwrap_or_else(|| &0) + 1);
    });

    freqmap
}

// Adapted from https://web.stanford.edu/class/archive/cs/cs106b/cs106b.1234/resources/huffman.html
fn huff_encode(str: &str) -> (String, String) {
    // [1] Frequency mapping
    let freqmap = str2freq(str);

    // [2] Merge forest
    // ...get two smallest weight trees
    let mut freqvec: Vec<(char, u8)> = freqmap.iter().map(|p| (*p.0, *p.1)).collect();
    let mut forest: Vec<BinaryTree<(char, u8)>> = Vec::new();

    freqvec.sort_by_key(|(_, k)| *k);
    freqvec.reverse();

    for freqi in freqvec {
        let mut ft = BinaryTree::new();
        ft.insert(freqi);
        forest.push(ft);
    }

    merge_forest(&mut forest);

    // [3] Convert hufftree to binary string mapper
    let huffmap = hufftree2binmap(&forest.pop().unwrap());

    // [4] Encode string
    let mut enc_str = String::new();
    for c in str.chars() {
        enc_str.push_str(huffmap[&c].as_str());
    }

    // [X] Flatten hufftree? → letter = ASCII 8 bits + encoding = 1-n bits
    //                         ...how to store padded in raw u24?
    //                         ...bit-pack with u24; high byte is letter ASCII, low byte is @LSB len, mid byte is @MSB rep.
    // let hft = huffmap.iter()
    //     .map(|(k,v)| hmap_pack(k,v))
    //     .collect();

    // [5] Optimise!!!
    // Storing the hufftree (even as u24) takes considerable storage, so...
    // * Canonical Huffman encoding (00, 01, 100)
    // * Delta-encoded lengths [2,3,3,4] → [2,+1,+0,+1] ✦ [0x2, 0x1, 0x0, 0x1]
    // * Run-length encoding [2,+1,+0,+1] → [2, +1 x2, +1] ✦ [0x2, 0x12, 0x1]

    // Interpret as "1;2;0;0$aaaaaaa".
    let mut enc_mapper = (String::new(), String::new());
    let mut enc_consec = 0;


    let mut shft: Vec<(&char, &str)> = huffmap.iter().map(|(k, v)| (k, v.as_str())).collect();
    shft.sort_by_key(|(_, k)| k.len());

    let empty_chain: &str = "0;";

    let iend = shft.len() - 1;
    for i in 0..shft.len() {
        let (k,v) = shft.get(i).unwrap();
        match &i {
            0 => {
                enc_mapper.0.push_str(empty_chain.repeat(*v.len() - 1));
            }

            iend => {
                enc_mapper.
            }

            _ => {


                enc_mapper.1.push(**k);

                if i == 0 || v.len() == shft.get(i-1).unwrap().1.len() {
                    enc_consec += 1;
                } else {
                    enc_mapper.0.push_str(enc_consec.to_string().as_str());
                    if i == shft.len() - 1 {
                        enc_mapper.0.push('$');
                    } else {
                        enc_mapper.0.push(';');
                    }
                    enc_consec = 0;
                }
            }

        }



    }

    enc_mapper.0.push_str(&*enc_mapper.1);


    (enc_str, enc_mapper.0)
}

fn shannon_entropy(str: &str) -> f64 {
    let freq = str2freq(str);
    -freq.values().map(|&p| (p as f64) * (p as f64).log2()).sum::<f64>()
}

fn huff_decode(str: &str, mappak: &str) -> String {
    // Copy string
    let mut str_cpy = str.clone().chars().collect::<Vec<_>>();
    let mut pak_cpy= mappak.clone().chars().collect::<Vec<_>>();
    let mut dec_str = String::new();

    // Unpack mapper
    let mut mapper: BTreeMap<&str, &char> = BTreeMap::default();
    let (values, keys): (Vec<u32>, Vec<char>) = {
        let delim = pak_cpy.binary_search(&'$').unwrap();
        (pak_cpy.drain(0..delim).map(|v| v.to_digit(10).unwrap()).collect(), pak_cpy.drain(1..dec_str.len()).collect())
    };

    let mut code_len = 1usize;
    let mut code = 0u32;

    let mut mappings: Vec<(String, char)> = Vec::new();
    for k in keys {
        let canon_code = if values.get(code_len).unwrap() == &0 {
            code_len += 1;
            huff_canonize(&code_len, &(code_len - 1), &mut code)
        } else {
            huff_canonize(&code_len, &code_len, &mut code)
        };

        let mut chain = *values.get(code_len).unwrap();
        chain -= 1;

        mappings.push((canon_code, k));
    }

    mappings.iter().for_each(|&(ref k, ref v)| { mapper.insert(&*k, &v); } );



    // Map string.
    let mut buf = String::new();

    while !str_cpy.is_empty() {
        buf.push(str_cpy.pop().unwrap());

        if let mapping = mapper.get(&buf as &str) {
            dec_str.push(**mapping.unwrap());
        }
    }

    dec_str
}

fn huff_canonize(curr_len: &usize, prev_len: &usize, code: &mut u32) -> String {
    *code <<= *curr_len - *prev_len;
    let curr_code = *code;
    *code += 1;
    (0..*prev_len).map(|n| char::from_u32((curr_code >> n) & 1)).fold(String::new(), |mut acc, n| {acc.push(n.unwrap()); acc})
}

fn huff_pack(k: &char, v: &str) -> u24 {
    ((u24::new(k.to_digit(2).unwrap()) << (v.len() / 2)
        + v.len() ) << 8
        + usize::from_str_radix(v, 2).expect("Huffman encoding not binary")) << (8 - v.len())
}
fn merge_forest(forest: &mut Vec<BinaryTree<(char, u8)>>) {
    // As per https://stackoverflow.com/questions/65948553/why-is-recursion-not-suggested-in-rust, iteration > recursion for memory safety and TCO over TCE.
    // so not a sorry excuse to be lazy yay!!! :>

    // Merge two trees with smallest weights, base node has combined weight until single tree remaining...
    // Assume sorted descending priority (H→L)
    // TODO: BTreeSet for sort @ insertion vs. manual sort? O(nlogn) vs. O(n)?
    while forest.len() > 1 {
        // Reuse that memory! Rather than a new Btree, clear one and use old roots...
        // ...whoops
        let (mut uno, mut dos) = (forest.remove(forest.len() - 1), forest.remove(forest.len() - 1));
        let merged_weight = uno.val().unwrap().1 + dos.val().unwrap().1;
        let merged_tree = BinaryTree::from_all(('\0', merged_weight), uno, dos);

        let pos = forest.partition_point(|t| t.val().unwrap().1 > merged_weight);
        forest.insert(pos, merged_tree);

        // forest.sort_by(|a, b| a.val().cmp(&b.val()));
    }
}

fn hufftree2binmap(hufftree: &BinaryTree<(char, u8)>) -> BTreeMap<char, String> {
    let mut binmap: BTreeMap<char, String> = BTreeMap::default();
    BinaryTree::<(char, u8)>::_huffleaf(hufftree, &mut binmap);
    binmap
}


#[cfg(test)]
mod tests {
    use std::ops::BitAnd;
    use super::*;

    #[test]
    fn s2f_std() {
        let fmap = str2freq("c");

        assert_eq!(fmap.get(&'c').unwrap(), &1);
    }

    #[test]
    fn s2f_std2() {
        let fmap = str2freq("0xDEADBEEF");

        assert_eq!(fmap.get(&'D').unwrap(), &2);
        assert_eq!(fmap.get(&'E').unwrap(), &3);
    }

    // Test with the famous "happy hip hop" test case
    // This will differ from https://huffman-coding-online.vercel.app/; it matches the tree found in the Stanford reference. Same cost...?
    #[test]
    fn mf_std() {
        let mut forest: Vec<BinaryTree<(char, u8)>> = Vec::new();
        let a = BinaryTree::from_val(('p', 4));
        let b = BinaryTree::from_val(('h', 3));
        let c = BinaryTree::from_val((' ', 2));
        let d = BinaryTree::from_val(('a', 1));
        let e = BinaryTree::from_val(('i', 1));
        let f = BinaryTree::from_val(('o', 1));
        let g = BinaryTree::from_val(('y', 1));

        forest.push(a);
        forest.push(b);
        forest.push(c);
        forest.push(d);
        forest.push(e);
        forest.push(f);
        forest.push(g);

        merge_forest(&mut forest);

        let sapling = forest.pop().unwrap();

        assert!(forest.is_empty());
        assert_eq!(sapling.height(), 5);
        assert_eq!(sapling.lum(), 7);
    }

    #[test]
    fn ht2bm_std() {
        let mut forest: Vec<BinaryTree<(char, u8)>> = Vec::new();
        let a = BinaryTree::from_val(('p', 4));
        let b = BinaryTree::from_val(('h', 3));
        let c = BinaryTree::from_val((' ', 2));
        let d = BinaryTree::from_val(('a', 1));
        let e = BinaryTree::from_val(('i', 1));
        let f = BinaryTree::from_val(('o', 1));
        let g = BinaryTree::from_val(('y', 1));

        forest.push(a);
        forest.push(b);
        forest.push(c);
        forest.push(d);
        forest.push(e);
        forest.push(f);
        forest.push(g);

        merge_forest(&mut forest);
        let hmap = hufftree2binmap(&forest.pop().unwrap());

        // Note: again deviates from vercel tool. Only diff b/w Stanford docs is i ↔ a, o ↔ y; this is trivial as same bit cost (there are several potential optimal candidates).
        // UPDATE: bitcost (34 bits) is equivalent!! :>
        assert_eq!(hmap.get(&'h').unwrap(), "01");
        assert_eq!(hmap.get(&'i').unwrap(), "000");
        assert_eq!(hmap.get(&'p').unwrap(), "10");
        assert_eq!(hmap.get(&'o').unwrap(), "1111");
        assert_eq!(hmap.get(&'a').unwrap(), "001");
        assert_eq!(hmap.get(&'y').unwrap(), "1110");
        assert_eq!(hmap.get(&' ').unwrap(), "110")
    }

    #[test]
    fn huffenc_str() {
        let pre: &str = "happy hip hop";
        let (post, _) = huff_encode(pre);

        assert_eq!(post.len(), 34);
    }

    #[test]
    fn huffenc_mapper() {
        let pre: &str = "happy hip hop";
        let (_, post) = huff_encode(pre);

        assert_eq!(post, "2;3;2;hpia oy")
    }



    #[test]
    fn hmpack_nopad() {
        let k = &'a';
        let v: &str = "0011";
        let pak = huff_pack(k, v);

        assert_eq!(pak.bitand(u24::new(0x100)) >> 0x0F, u24::new(u32::from('a')));
        assert_eq!(pak.bitand(u24::new(0x010)) >> 0x07, u24::new(4));
        assert_eq!(pak.bitand(u24::new(0x001)), u24::new(0b11));
    }

    // <META> Use the iso(lated) prefix for unseparated snippets.
    #[test]
    fn iso_blank() {
        let a = 'b';

        assert_eq!(a, 'b');
    }
}