/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tracking of attr()-tainted ranges within CSS strings.

use smallvec::SmallVec;

/// For a CSS string, the range, counted in bytes, that is attr()-tainted.
#[derive(Clone, Debug, Default, MallocSizeOf, PartialEq, ToShmem)]
pub struct AttrTaintedRange {
    /// Start of the range, counted in bytes. Inclusive.
    start: usize,
    /// End of the range, counted in bytes. Exclusive.
    end: usize,
}

impl AttrTaintedRange {
    /// Creates a range within a CSS string that is tainted by attr().
    #[inline(always)]
    pub fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }
}

/// In CSS Values and Units, values produced by `attr()` are considered attr()-tainted, as are
/// functions that contain an attr()-tainted value. Using an attr()-tainted value as or in a <url>
/// makes a declaration invalid at computed-value time.
/// https://drafts.csswg.org/css-values-5/#attr-security
#[derive(Clone, Debug, Default, MallocSizeOf, PartialEq, ToShmem)]
pub struct AttrTaint(SmallVec<[AttrTaintedRange; 1]>);

impl AttrTaint {
    /// For a CSS string, determine whether any `<url>` overlapping this `range`
    /// is disallowed due to attr()-tainting.
    #[inline(always)]
    pub fn should_disallow_urls_in_range(&self, range: &AttrTaintedRange) -> bool {
        self.0
            .iter()
            .any(|r| r.start <= range.end && r.end >= range.start)
    }

    /// Returns true if the attr()-tainted range contains no elements.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Creates a taint covering the whole string up to `end`.
    #[inline(always)]
    pub fn new_fully_tainted(end: usize) -> Self {
        let mut taint = Self::default();
        taint.push(0, end);
        taint
    }

    /// Adds a tainted range.
    #[inline(always)]
    pub fn push(&mut self, start: usize, end: usize) {
        self.0.push(AttrTaintedRange::new(start, end));
    }
}
