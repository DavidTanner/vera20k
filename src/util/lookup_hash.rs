//! The crate's hasher for maps that are only looked up: FxHash's multiply,
//! far cheaper than the standard library's SipHash on the short ids and type
//! names these maps hold, and the same in every process. A map hashed with it
//! must not let its iteration order reach a result.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default)]
pub(crate) struct LookupHasher(u64);

impl Hasher for LookupHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// A lookup-only map hashed with [`LookupHasher`].
pub(crate) type LookupMap<K, V> = HashMap<K, V, BuildHasherDefault<LookupHasher>>;
