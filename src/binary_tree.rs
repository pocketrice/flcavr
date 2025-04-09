#![no_std]

use alloc::boxed::Box;
use core::cmp::Ordering;
use core::cmp::max;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[derive(Debug)]
struct Node<T: Ord> {
    value: T,
    left: Subtree<T>,
    right: Subtree<T>
}

#[derive(Debug)]
struct Subtree<T: Ord>(Option<Box<Node<T>>>);

impl<T: Ord> Node<T> {
    fn new(value: T) -> Self {
        Self { value, left: Subtree::new(), right: Subtree::new() }
    }

    fn assemble(value: T, left: Subtree<T>, right: Subtree<T>) -> Self {
        Self { value, left, right }
    }
}

impl<T: Ord> Subtree<T> {
    fn new() -> Self {
        Self(None)
    }

    fn insert(&mut self, value: T) {
        match &mut self.0 {
            None => self.0 = Some(Box::new(Node::new(value))),
            Some(n) => match value.cmp(&n.value) {
                Ordering::Less => n.left.insert(value),
                Ordering::Equal => {}
                Ordering::Greater => n.right.insert(value),
            },
        }
    }

    fn has(&self, value: &T) -> bool {
        match &self.0 {
            None => false,
            Some(n) => match value.cmp(&n.value) {
                Ordering::Less => n.left.has(value),
                Ordering::Equal => true,
                Ordering::Greater => n.right.has(value),
            }
        }
    }

    // In-order traversal.
    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&T),
    {
        if let Some(ref root) = self.0 {
            root.left.for_each(&mut f);
            f(&root.value);
            root.right.for_each(&mut f);
        }
    }

    fn len(&self) -> usize {
        match &self.0 {
            None => 0,
            Some(n) => n.left.len() + n.right.len() + 1,
        }
    }

    fn height(&self) -> usize {
        match &self.0 {
            None => 0,
            Some(n) => max(n.left.height(), n.right.height()) + 1
        }
    }

    fn lum(&self) -> usize {
        match &self.0 {
            None => 0,
            Some(n) => usize::from(!(n.left.0.is_some() || n.right.0.is_some())) + n.left.lum() + n.right.lum()
        }
    }

    fn leaves<'a>(&'a self, v: &mut Vec<&'a T>) {
        match &self.0 {
            None => (),
            Some(n) => {
                if !(n.left.0.is_some() || n.right.0.is_some()) {
                    v.push(&n.value);
                } else {
                    n.left.leaves(v);
                    n.right.leaves(v);
                }
            }
        }
    }

    // Helper method for finding binary representation of Huffman tree leaf paths.
    // Using recursion saves time over iteratively looking thru the tree for each mapping.
    fn _huffleaf(hufftree: &Subtree<(char, u8)>, current_code: String, binmap: &mut BTreeMap<char, String>) {
        match &hufftree.0 {
            None => (),
            Some(n) => {
                if !(n.left.0.is_some() || n.right.0.is_some()) {
                    binmap.insert(n.value.0, current_code.clone());
                } else {
                    let mut left_code = current_code.clone();
                    left_code.push('0');
                    Self::_huffleaf(&n.left, left_code, binmap);

                    let mut right_code = current_code.clone();
                    right_code.push('1');
                    Self::_huffleaf(&n.right, right_code, binmap);
                }
            }
        }
    }

    fn disassemble(&mut self) -> (T, Subtree<T>, Subtree<T>) {
        let node = self.0.take().unwrap();

        *self = Self::new();

        (node.value, node.left, node.right)
    }

    fn assemble(&mut self, value: T, left: Subtree<T>, right: Subtree<T>) {
        *self = Self(
            Some(Box::new(Node::assemble(value, left, right)))
        )
    }

    fn val(&self) -> Option<&T> {
        self.0.as_ref().map(|node| &node.value)
    }

    fn left(&self) -> Option<&Subtree<T>> {
        self.0.as_ref()
            .map(|node| &(node.left))
    }

    fn right(&self) -> Option<&Subtree<T>> {
        self.0.as_ref()
            .map(|node| &(node.right))
    }

    // TODO: implementing iterator on Subtree (by proxy BinaryTree) would make this a lot easier
    fn compute<U,F>(&self, func: &F, v: &mut Vec<U>)
        where F: Fn(&T) -> U
    {
        match &self.0 {
            None => (),
            Some(n) => {
                v.push(func(&n.value));
                n.left.compute(func, v);
                n.right.compute(func, v);
            }
        }
    }

    fn assemble_then_fix(&mut self, value: T, left: Subtree<T>, right: Subtree<T>) {
        // Fixing binary tree... TODO
    }
    //
    // fn to_string(&self) -> String {
    //     let mut s = String::new();
    //     s.push('(');
    //     s.push('[');
    //     s.push_str(self.val().unwrap());
    // }
}

#[derive(Debug)]
pub struct BinaryTree<T: Ord> {
    pub root: Subtree<T>
}

impl<T: Ord> BinaryTree<T> {
    pub fn new() -> Self {
        Self { root: Subtree::new() }
    }

    pub fn from_val(value: T) -> Self {
        let mut this = Self::new();
        this.insert(value);
        this
    }

    pub fn from_all(value: T, left: BinaryTree<T>, right: BinaryTree<T>) -> Self {
        let mut this = Self::new();
        this.assemble(value, left.root, right.root);
        this
    }

    pub fn insert(&mut self, value: T) {
        self.root.insert(value);
    }

    pub fn has(&self, value: &T) -> bool {
        self.root.has(value)
    }

    pub fn len(&self) -> usize {
        self.root.len()
    }

    pub fn height(&self) -> usize {
        self.root.height()
    }

    // Number of leaves (lum)
    pub fn lum(&self) -> usize {
        self.root.lum()
    }

    pub fn leaves<'a>(&'a self, v: &mut Vec<&'a T>) {
        self.root.leaves(v)
    }

    pub fn _huffleaf(hufftree: &BinaryTree<(char, u8)>, binmap: &mut BTreeMap<char, String>) {
        Subtree::<T>::_huffleaf(&hufftree.root, String::new(), binmap);
    }

    pub fn val(&self) -> Option<&T> {
        self.root.val()
    }

    pub fn left(&self) -> Option<&Subtree<T>> {
        self.root.left()
    }

    pub fn right(&self) -> Option<&Subtree<T>> {
        self.root.right()
    }

    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&T),
    {
        self.root.for_each(&mut f);
    }

    fn assemble(&mut self, value: T, left: Subtree<T>, right: Subtree<T>) {
        self.root.assemble(value, left, right)
    }

    fn disassemble(&mut self) -> (T, Subtree<T>, Subtree<T>) {
        self.root.disassemble()
    }

    fn assemble_then_fix(&mut self, value: T, left: Subtree<T>, right: Subtree<T>) {
        self.root.assemble_then_fix(value, left, right)
    }
}

#[cfg(test)]
mod tests {
    use crate::utility::vec_matches;
    use super::*;

    #[test]
    fn bt_const() {
        let tree: BinaryTree<char> = BinaryTree::new();

        assert!(tree.root.val().is_none());
        assert!(tree.root.left().is_none());
        assert!(tree.root.right().is_none());
    }

    #[test]
    fn bt_ins_1_num() {
        let mut tree = BinaryTree::new();
        tree.insert(1);
        assert_eq!(tree.root.val().unwrap(), &1);
    }

    #[test]
    fn bt_ins_1_alpha() {
        let mut tree = BinaryTree::new();
        tree.insert('a');
        assert_eq!(tree.root.val().unwrap(), &'a');
    }

    #[test]
    fn bt_ins_3_balanced_num() {
        let mut tree = BinaryTree::new();
        tree.insert(2);
        tree.insert(1);
        tree.insert(3);
        assert_eq!(tree.root.val().unwrap(), &2);
        assert_eq!(tree.root.left().unwrap().val().unwrap(), &1);
        assert_eq!(tree.root.right().unwrap().val().unwrap(), &3);
    }

    #[test]
    fn bt_has() {
        let mut tree = BinaryTree::new();
        tree.insert('a');
        tree.insert('h');
        tree.insert('l');
        tree.insert('0');
        tree.insert('A');

        assert!(tree.has(&'a'));
        assert!(tree.has(&'l'));
        assert!(!tree.has(&'z'));
    }

    #[test]
    fn bt_len() {
        let mut tree = BinaryTree::new();
        tree.insert('p');
        tree.insert('q');
        tree.insert('r');
        tree.insert('b');
        tree.insert('3');

        assert_eq!(tree.len(), 5);
    }

    #[test]
    fn bt_height_0() {
        let tree = BinaryTree::<char>::new();

        assert_eq!(tree.height(), 0);
    }

    #[test]
    fn bt_height_1() {
        let mut tree = BinaryTree::new();
        tree.insert('a');

        assert_eq!(tree.height(), 1);
    }

    #[test]
    fn bt_height_n_degen() {
        let mut tree = BinaryTree::new();
        tree.insert('p');
        tree.insert('q');
        tree.insert('r');

        assert_eq!(tree.height(), 3);
    }

    #[test]
    fn bt_height_n() {
        let mut tree = BinaryTree::new();
        tree.insert('p');
        tree.insert('q');
        tree.insert('r');
        tree.insert('a');
        tree.insert('b');
        tree.insert('s');

        assert_eq!(tree.height(), 4);
    }

    #[test]
    fn bt_lum_0() {
        let tree = BinaryTree::<char>::new();

        assert_eq!(tree.lum(), 0);
    }

    #[test]
    fn bt_lum_1() {
        let mut tree = BinaryTree::new();
        tree.insert('a');

        assert_eq!(tree.lum(), 1);
    }

    #[test]
    fn bt_lum_n_sm() {
        let mut tree = BinaryTree::new();
        tree.insert('c');
        tree.insert('d');
        tree.insert('b');

        assert_eq!(tree.lum(), 2);
    }

    #[test]
    fn bt_lum_n_lg() {
        let mut tree = BinaryTree::new();
        tree.insert('c');
        tree.insert('e');
        tree.insert('b');
        tree.insert('f');
        tree.insert('d');

        assert_eq!(tree.lum(), 3);
    }

    #[test]
    fn bt_leaves_0() {
        let tree = BinaryTree::<char>::new();

        let mut v = Vec::<&char>::new();
        tree.leaves(&mut v);

        assert!(v.is_empty());
    }

    #[test]
    fn bt_leaves_1() {
        let mut tree = BinaryTree::new();
        tree.insert('a');

        let mut v = Vec::<&char>::new();
        tree.leaves(&mut v);

        let mut w = Vec::<&char>::new();
        w.push(&'a');

        assert!(vec_matches(&v, &w))
    }

    #[test]
    fn bt_leaves_n_sm() {
        let mut tree = BinaryTree::new();
        tree.insert('c');
        tree.insert('d');
        tree.insert('b');

        let mut v = Vec::<&char>::new();
        tree.leaves(&mut v);

        let mut w = Vec::<&char>::new();
        w.push(&'d');
        w.push(&'b');

        assert!(vec_matches(&v, &w))
    }

    #[test]
    fn bt_leaves_n_lg() {
        let mut tree = BinaryTree::new();
        tree.insert('c');
        tree.insert('e');
        tree.insert('b');
        tree.insert('f');
        tree.insert('d');

        let mut v = Vec::<&char>::new();
        tree.leaves(&mut v);

        let mut w = Vec::<&char>::new();
        w.push(&'f');
        w.push(&'b');
        w.push(&'d');

        assert!(vec_matches(&v, &w))
    }

    #[test]
    fn bt_disassemble() {
        let mut tree = BinaryTree::new();
        tree.insert('b');
        tree.insert('a');
        tree.insert('c');

        let (val, left, right) = tree.disassemble();

        assert_eq!(val, 'b');
        assert_eq!(left.val().unwrap(), &'a');
        assert_eq!(right.val().unwrap(), &'c');
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn bt_assemble() {
        let mut tree = BinaryTree::new();
        let mut left = BinaryTree::new();
        let mut right = BinaryTree::new();

        left.insert('a');
        left.insert('3');
        right.insert('d');
        right.insert('c');

        tree.root.assemble('4', left.root, right.root)
    }

    // "Infix" = in-line assembling with own subtree... mostly to test proper referencing
    #[test]
    fn bt_infix() {
        let mut tree = BinaryTree::new();
        tree.insert('3');
        tree.insert('2');
        tree.insert('1');
        tree.insert('4');

        let mut right = BinaryTree::<char>::new();
        tree.insert('a');
        tree.insert('B');
        tree.insert('3');

        // tree.assemble(
        //     ' ',
        //     *left,
        //     *right.root
        // );

        assert!(tree.has(&'B'));
        // assert_eq!(tree.right().unwrap(), right);
    }

    // #[test]
    // fn bt_str() {
    //     let mut tree = BinaryTree::new();
    //     tree.insert('b');
    //     tree.insert('c');
    //     tree.insert('f');
    //     tree.insert('a');
    //
    //     print!("{:?}", tree)
    // }

    #[test]
    fn bt_reconstruct() {
        let mut tree = BinaryTree::new();
        tree.insert('b');
        tree.insert('c');
        tree.insert('z');

        let (val, left, right) = tree.disassemble();
        tree.assemble(val, left, right);

        assert_eq!(tree.root.val().unwrap(), &'b');
        assert!(tree.root.left().is_none());
        // test right tree
    }
}