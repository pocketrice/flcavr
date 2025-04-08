pub fn vec_matches<T: core::cmp::PartialEq>(v: &Vec<T>, w: &Vec<T>) -> bool {
    v.iter().all(|x| w.contains(x))
}