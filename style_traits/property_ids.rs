/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS property identifiers, shared by the style system crates.
//!
//! The per-property tables are generated from the property data by the
//! `stylo_build` crate; see `property_ids.mako.rs`.

#![allow(unsafe_code)]

use crate::rule_types::{CssRuleType, CssRuleTypes};
use crate::{CssWriter, ToCss};
use malloc_size_of::{MallocSizeOf, MallocSizeOfOps};
use num_derive::FromPrimitive;
use std::fmt::{self, Write};
use std::mem;

include!(concat!(env!("OUT_DIR"), "/property_ids.rs"));

/// The type of the function used to check whether a property is enabled via
/// Gecko preferences.
#[cfg(feature = "gecko")]
pub type GeckoPropertyEnabledFn = fn(NonCustomPropertyId) -> bool;

#[cfg(feature = "gecko")]
static GECKO_PROPERTY_ENABLED_FN: std::sync::atomic::AtomicPtr<()> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

/// Registers the function used to check whether a property is enabled via
/// Gecko preferences. Must be called before any property enabled checks, i.e.
/// during style system initialization.
#[cfg(feature = "gecko")]
pub fn set_gecko_property_enabled_fn(f: GeckoPropertyEnabledFn) {
    GECKO_PROPERTY_ENABLED_FN.store(f as *mut (), std::sync::atomic::Ordering::Release);
}

#[cfg(feature = "gecko")]
fn gecko_property_enabled(id: NonCustomPropertyId) -> bool {
    let ptr = GECKO_PROPERTY_ENABLED_FN.load(std::sync::atomic::Ordering::Acquire);
    debug_assert!(!ptr.is_null(), "Gecko property-enabled hook not registered");
    if ptr.is_null() {
        return false;
    }
    let f: GeckoPropertyEnabledFn = unsafe { mem::transmute(ptr) };
    f(id)
}

bitflags::bitflags! {
    /// A set of flags for properties.
    #[derive(Clone, Copy)]
    pub struct PropertyFlags: u16 {
        /// This longhand property applies to ::first-letter.
        const APPLIES_TO_FIRST_LETTER = 1 << 1;
        /// This longhand property applies to ::first-line.
        const APPLIES_TO_FIRST_LINE = 1 << 2;
        /// This longhand property applies to ::placeholder.
        const APPLIES_TO_PLACEHOLDER = 1 << 3;
        ///  This longhand property applies to ::cue.
        const APPLIES_TO_CUE = 1 << 4;
        /// This longhand property applies to ::marker.
        const APPLIES_TO_MARKER = 1 << 5;
        /// This property is a legacy shorthand.
        ///
        /// https://drafts.csswg.org/css-cascade/#legacy-shorthand
        const IS_LEGACY_SHORTHAND = 1 << 6;

        /* The following flags are currently not used in Rust code, they
         * only need to be listed in corresponding properties so that
         * they can be checked in the C++ side via ServoCSSPropList.h. */

        /// This property can be animated on the compositor.
        const CAN_ANIMATE_ON_COMPOSITOR = 0;
        /// See data.py's documentation about the affects_flags.
        const AFFECTS_LAYOUT = 0;
        #[allow(missing_docs)]
        const AFFECTS_OVERFLOW = 0;
        #[allow(missing_docs)]
        const AFFECTS_PAINT = 0;
    }
}

/// A longhand or shorthand property.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ToShmem, MallocSizeOf)]
#[repr(C)]
pub struct NonCustomPropertyId(u16);

impl ToCss for NonCustomPropertyId {
    #[inline]
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str(self.name())
    }
}

impl NonCustomPropertyId {
    /// Returns the underlying index, used for use counter.
    pub fn bit(self) -> usize {
        self.0 as usize
    }

    /// Builds a `NonCustomPropertyId` from its underlying index. The caller
    /// must guarantee `index < property_counts::NON_CUSTOM`.
    #[doc(hidden)]
    #[inline]
    pub const fn from_index(index: u16) -> Self {
        debug_assert!((index as usize) < property_counts::NON_CUSTOM);
        Self(index)
    }

    /// Resolves the alias of a given property if needed.
    pub fn unaliased(self) -> Self {
        let Some(alias_id) = self.as_alias() else {
            return self;
        };
        alias_id.aliased_property()
    }

    /// Returns a longhand id, if this property is one.
    #[inline]
    pub fn as_longhand(self) -> Option<LonghandId> {
        if self.0 < property_counts::LONGHANDS as u16 {
            return Some(unsafe { mem::transmute(self.0 as u16) });
        }
        None
    }

    /// Returns a shorthand id, if this property is one.
    #[inline]
    pub fn as_shorthand(self) -> Option<ShorthandId> {
        if self.0 >= property_counts::LONGHANDS as u16
            && self.0 < property_counts::LONGHANDS_AND_SHORTHANDS as u16
        {
            return Some(unsafe { mem::transmute(self.0 - (property_counts::LONGHANDS as u16)) });
        }
        None
    }

    /// Returns an alias id, if this property is one.
    #[inline]
    pub fn as_alias(self) -> Option<AliasId> {
        debug_assert!((self.0 as usize) < property_counts::NON_CUSTOM);
        if self.0 >= property_counts::LONGHANDS_AND_SHORTHANDS as u16 {
            return Some(unsafe {
                mem::transmute(self.0 - (property_counts::LONGHANDS_AND_SHORTHANDS as u16))
            });
        }
        None
    }

    /// Returns either a longhand or a shorthand, resolving aliases.
    #[inline]
    pub fn longhand_or_shorthand(self) -> Result<LonghandId, ShorthandId> {
        let id = self.unaliased();
        match id.as_longhand() {
            Some(lh) => Ok(lh),
            None => Err(id.as_shorthand().unwrap()),
        }
    }

    /// Converts a longhand id into a non-custom property id.
    #[inline]
    pub const fn from_longhand(id: LonghandId) -> Self {
        Self(id as u16)
    }

    /// Converts a shorthand id into a non-custom property id.
    #[inline]
    pub const fn from_shorthand(id: ShorthandId) -> Self {
        Self((id as u16) + (property_counts::LONGHANDS as u16))
    }

    /// Converts an alias id into a non-custom property id.
    #[inline]
    pub const fn from_alias(id: AliasId) -> Self {
        Self((id as u16) + (property_counts::LONGHANDS_AND_SHORTHANDS as u16))
    }

    #[cfg(feature = "servo")]
    /// Iterate over all non-custom properties in arbitrary order.
    pub fn iter() -> impl Iterator<Item = Self> {
        (0..property_counts::NON_CUSTOM as u16).map(|index| Self(index))
    }
}

impl From<LonghandId> for NonCustomPropertyId {
    #[inline]
    fn from(id: LonghandId) -> Self {
        Self::from_longhand(id)
    }
}

impl From<ShorthandId> for NonCustomPropertyId {
    #[inline]
    fn from(id: ShorthandId) -> Self {
        Self::from_shorthand(id)
    }
}

impl From<AliasId> for NonCustomPropertyId {
    #[inline]
    fn from(id: AliasId) -> Self {
        Self::from_alias(id)
    }
}

impl ToCss for LonghandId {
    #[inline]
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str(self.name())
    }
}

impl fmt::Debug for LonghandId {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

impl LonghandId {
    /// Get the name of this longhand property.
    #[inline]
    pub fn name(&self) -> &'static str {
        NonCustomPropertyId::from(*self).name()
    }

    /// Returns whether the longhand property is inherited by default.
    #[inline]
    pub fn inherited(self) -> bool {
        !LonghandIdSet::reset().contains(self)
    }

    /// Returns whether the longhand property is zoom-dependent.
    #[inline]
    pub fn zoom_dependent(self) -> bool {
        LonghandIdSet::zoom_dependent().contains(self)
    }

    /// Returns true if the property is one that is ignored when document
    /// colors are disabled.
    #[inline]
    pub fn ignored_when_document_colors_disabled(self) -> bool {
        LonghandIdSet::ignored_when_colors_disabled().contains(self)
    }

    /// Returns whether this longhand is `non_custom` or is a longhand of it.
    pub fn is_or_is_longhand_of(self, non_custom: NonCustomPropertyId) -> bool {
        match non_custom.longhand_or_shorthand() {
            Ok(lh) => self == lh,
            Err(sh) => self.is_longhand_of(sh),
        }
    }

    /// Returns whether this longhand is a longhand of `shorthand`.
    pub fn is_longhand_of(self, shorthand: ShorthandId) -> bool {
        self.shorthands().any(|s| s == shorthand)
    }

    /// Returns whether this property is animatable.
    #[inline]
    pub fn is_animatable(self) -> bool {
        NonCustomPropertyId::from(self).is_animatable()
    }

    /// Returns whether this property is animatable in a discrete way.
    #[inline]
    pub fn is_discrete_animatable(self) -> bool {
        LonghandIdSet::discrete_animatable().contains(self)
    }

    /// Return whether this property is logical.
    #[inline]
    pub fn is_logical(self) -> bool {
        LonghandIdSet::logical().contains(self)
    }
}

impl ToCss for ShorthandId {
    #[inline]
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str(self.name())
    }
}

impl ShorthandId {
    /// Get the name for this shorthand property.
    #[inline]
    pub fn name(&self) -> &'static str {
        NonCustomPropertyId::from(*self).name()
    }

    /// Returns whether this property is a legacy shorthand.
    #[inline]
    pub fn is_legacy_shorthand(self) -> bool {
        self.flags().contains(PropertyFlags::IS_LEGACY_SHORTHAND)
    }
}

/// A trait for property-id-like types that can be stored compactly in an
/// `IdSet` bitfield.
pub trait IndexedId: Copy {
    /// The number of distinct ids, i.e. the number of bits the set needs.
    const COUNT: usize;
    /// Builds an id from its index in the set. The caller must guarantee that
    /// `index < Self::COUNT`.
    unsafe fn from_index_release_unchecked(index: usize) -> Self;
    /// Returns the index of this id in the set.
    fn to_index(self) -> usize;
}

impl IndexedId for NonCustomPropertyId {
    const COUNT: usize = property_counts::NON_CUSTOM;

    #[inline(always)]
    unsafe fn from_index_release_unchecked(index: usize) -> Self {
        debug_assert!(index < Self::COUNT);
        NonCustomPropertyId(index as u16)
    }

    #[inline(always)]
    fn to_index(self) -> usize {
        self.0 as usize
    }
}

impl IndexedId for PrioritaryPropertyId {
    const COUNT: usize = property_counts::PRIORITARY;

    #[inline(always)]
    unsafe fn from_index_release_unchecked(index: usize) -> Self {
        debug_assert!(index < Self::COUNT);
        std::mem::transmute(index as u8)
    }

    #[inline(always)]
    fn to_index(self) -> usize {
        self as usize
    }
}

impl IndexedId for LonghandId {
    const COUNT: usize = property_counts::LONGHANDS;

    #[inline(always)]
    unsafe fn from_index_release_unchecked(index: usize) -> Self {
        debug_assert!(index < Self::COUNT);
        std::mem::transmute(index as u16)
    }

    #[inline(always)]
    fn to_index(self) -> usize {
        self as usize
    }
}

/// A set of non-custom properties.
pub type NonCustomPropertyIdSet =
    IdSet<NonCustomPropertyId, { (property_counts::NON_CUSTOM - 1 + 32) / 32 }>;
/// An iterator over non-custom properties.
pub type NonCustomPropertyIdSetIterator<'a> = IdSetIterator<'a, NonCustomPropertyId>;
/// A set of prioritary properties.
pub type PrioritaryPropertyIdSet =
    IdSet<PrioritaryPropertyId, { (property_counts::PRIORITARY - 1 + 32) / 32 }>;
/// An iterator over prioritary properties.
pub type PrioritaryPropertyIdSetIterator<'a> = IdSetIterator<'a, PrioritaryPropertyId>;
/// A set of longhand properties.
pub type LonghandIdSet = IdSet<LonghandId, { (property_counts::LONGHANDS - 1 + 32) / 32 }>;
/// An iterator over longhand properties.
pub type LonghandIdSetIterator<'a> = IdSetIterator<'a, LonghandId>;

/// A set of ids indexed in a bitfield. `W` is the number of `u32` chunks needed to store
/// `Id::COUNT` bits, and is filled in by the type aliases above.
///
/// TODO(emilio): It'd be nice for the const parameter to be COUNT (or even not be there and pull
/// from Id::COUNT), but that can't be done in stable rust yet, see:
/// https://github.com/rust-lang/rust/issues/76560
pub struct IdSet<Id: IndexedId, const W: usize> {
    storage: [u32; W],
    _phantom: std::marker::PhantomData<Id>,
}

impl<Id: IndexedId, const W: usize> Clone for IdSet<Id, W> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<Id: IndexedId, const W: usize> Copy for IdSet<Id, W> {}

impl<Id: IndexedId, const W: usize> Default for IdSet<Id, W> {
    #[inline]
    fn default() -> Self {
        Self {
            storage: [0; W],
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<Id: IndexedId, const W: usize> PartialEq for IdSet<Id, W> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.storage == other.storage
    }
}

impl<Id: IndexedId, const W: usize> fmt::Debug for IdSet<Id, W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.storage.fmt(f)
    }
}

impl<Id: IndexedId, const W: usize> MallocSizeOf for IdSet<Id, W> {
    #[inline(always)]
    fn size_of(&self, _: &mut MallocSizeOfOps) -> usize {
        0
    }
}

impl<Id: IndexedId, const W: usize> IdSet<Id, W> {
    /// Creates an empty `IdSet`.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a set from its raw bitfield storage.
    #[doc(hidden)]
    pub const fn from_storage(storage: [u32; W]) -> Self {
        Self {
            storage,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Insert an id in the set.
    #[inline]
    pub fn insert(&mut self, id: Id) {
        let bit = id.to_index();
        self.storage[bit / 32] |= 1 << (bit % 32);
    }

    /// Remove the given id from the set.
    #[inline]
    pub fn remove(&mut self, id: Id) {
        let bit = id.to_index();
        self.storage[bit / 32] &= !(1 << (bit % 32));
    }

    /// Return whether the given id is in the set.
    #[inline]
    pub fn contains(&self, id: Id) -> bool {
        let bit = id.to_index();
        (self.storage[bit / 32] & (1 << (bit % 32))) != 0
    }

    /// Iterate over the current id set.
    pub fn iter(&self) -> IdSetIterator<'_, Id> {
        IdSetIterator {
            chunks: &self.storage,
            cur_chunk: 0,
            cur_bit: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Returns whether this set contains at least every id that `other` also contains.
    pub fn contains_all(&self, other: &Self) -> bool {
        for (self_cell, other_cell) in self.storage.iter().zip(other.storage.iter()) {
            if (*self_cell & *other_cell) != *other_cell {
                return false;
            }
        }
        true
    }

    /// Returns whether this set contains any id that `other` also contains.
    pub fn contains_any(&self, other: &Self) -> bool {
        for (self_cell, other_cell) in self.storage.iter().zip(other.storage.iter()) {
            if (*self_cell & *other_cell) != 0 {
                return true;
            }
        }
        false
    }

    /// Remove all the given ids from the set.
    #[inline]
    pub fn remove_all(&mut self, other: &Self) {
        for (self_cell, other_cell) in self.storage.iter_mut().zip(other.storage.iter()) {
            *self_cell &= !*other_cell;
        }
    }

    /// Clear all bits
    #[inline]
    pub fn clear(&mut self) {
        for cell in &mut self.storage {
            *cell = 0
        }
    }

    /// Returns whether the set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.storage.iter().all(|c| *c == 0)
    }
}

to_shmem::impl_trivial_to_shmem!(LonghandIdSet);
impl LonghandIdSet {
    /// Return whether this set contains any reset longhand.
    #[inline]
    pub fn contains_any_reset(&self) -> bool {
        self.contains_any(Self::reset())
    }
}

/// An iterator over a set of ids.
pub struct IdSetIterator<'a, Id: IndexedId> {
    chunks: &'a [u32],
    cur_chunk: u32,
    cur_bit: u32, // [0..31], note that zero means the end-most bit
    _phantom: std::marker::PhantomData<Id>,
}

impl<'a, Id: IndexedId> Iterator for IdSetIterator<'a, Id> {
    type Item = Id;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            debug_assert!(self.cur_bit < 32);
            let cur_chunk = self.cur_chunk;
            let cur_bit = self.cur_bit;
            let chunk = *self.chunks.get(cur_chunk as usize)?;
            let next_bit = (chunk >> cur_bit).trailing_zeros();
            if next_bit == 32 {
                // Totally empty chunk, skip it.
                self.cur_bit = 0;
                self.cur_chunk += 1;
                continue;
            }
            debug_assert!(cur_bit + next_bit < 32);
            let index = (cur_chunk * 32 + cur_bit + next_bit) as usize;
            debug_assert!(index < Id::COUNT);
            let id = unsafe { Id::from_index_release_unchecked(index) };
            self.cur_bit += next_bit + 1;
            if self.cur_bit == 32 {
                self.cur_bit = 0;
                self.cur_chunk += 1;
            }
            return Some(id);
        }
    }
}

/// An iterator over all the property ids that are enabled for a given
/// shorthand, if that shorthand is enabled for all content too.
pub struct NonCustomPropertyIterator<Item: 'static> {
    filter: bool,
    iter: std::slice::Iter<'static, Item>,
}

impl<Item> Iterator for NonCustomPropertyIterator<Item>
where
    Item: 'static + Copy + Into<NonCustomPropertyId>,
{
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = *self.iter.next()?;
            if !self.filter || id.into().enabled_for_all_content() {
                return Some(id);
            }
        }
    }
}
