#![cfg_attr(not(test), no_std)]

use alloc::boxed::Box;
use core::cmp::Ordering;
// Adapted from https://google.github.io/comprehensive-rust/smart-pointers/exercise.html

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

    fn len(&self) -> usize {
        match &self.0 {
            None => 0,
            Some(n) => n.left.len() + n.right.len() + 1,
        }
    }

    fn disassemble(&mut self) -> (T, Subtree<T>, Subtree<T>) {
        let contents = &self;

        self = &mut Self::new(); // TODO bad tags?

        (contents.unwrap().value, contents.unwrap().left, contents.unwrap().right)
    }

    fn assemble(&mut self, value: &T, left: Subtree<T>, right: Subtree<T>) {
        self = &mut Self(
            Some(Box::new(Node::assemble(value, left, right)))
        )
    }
}

#[derive(Debug)]
pub struct BinaryTree<T: Ord> {
    root: Subtree<T>
}

impl<T: Ord> BinaryTree<T> {
    pub fn new() -> Self {
        Self { root: Subtree::new() }
    }

    fn insert(&mut self, value: T) {
        self.root.insert(value);
    }

    fn has(&self, value: &T) -> bool {
        self.root.has(value)
    }

    fn len(&self) -> usize {
        self.root.len()
    }
}