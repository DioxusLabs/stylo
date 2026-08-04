/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Hash map and set types keyed by precomputed hashes.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// A hasher implementation that doesn't hash anything, because it expects its
/// input to be a suitable u32 hash.
pub struct PrecomputedHasher {
    hash: Option<u32>,
}

impl Default for PrecomputedHasher {
    fn default() -> Self {
        Self { hash: None }
    }
}

/// A simple alias for a hashmap using PrecomputedHasher.
pub type PrecomputedHashMap<K, V> = HashMap<K, V, BuildHasherDefault<PrecomputedHasher>>;

/// A simple alias for a hashset using PrecomputedHasher.
pub type PrecomputedHashSet<K> = HashSet<K, BuildHasherDefault<PrecomputedHasher>>;

impl Hasher for PrecomputedHasher {
    #[inline]
    fn write(&mut self, _: &[u8]) {
        unreachable!(
            "Called into PrecomputedHasher with something that isn't \
             a u32"
        )
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        debug_assert!(self.hash.is_none());
        self.hash = Some(i);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash.expect("PrecomputedHasher wasn't fed?") as u64
    }
}
