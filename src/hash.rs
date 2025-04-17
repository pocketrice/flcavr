#![no_std]

use core::hash::{BuildHasher, Hasher};

// Step 1: Implement the Hasher
#[derive(Default)]
pub struct NaiveXORHasher {
    state: u64,
}

impl Hasher for NaiveXORHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= u64::from(byte);
            // Rotate to mix bits
            self.state = self.state.rotate_left(5);
        }
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

// Step 2: Implement the BuildHasher
#[derive(Default)]
pub struct NaiveXORHasherBuilder;

impl BuildHasher for NaiveXORHasherBuilder {
    type Hasher = NaiveXORHasher;

    fn build_hasher(&self) -> Self::Hasher {
        NaiveXORHasher::default()
    }
}