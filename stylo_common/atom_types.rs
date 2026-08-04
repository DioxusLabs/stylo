/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Atom-based identifier and string types shared by the style system crates.

#![allow(unsafe_code)]

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use cssparser::{serialize_identifier, serialize_name, Parser};
use precomputed_hash::PrecomputedHash;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError};

#[cfg(feature = "gecko")]
pub use crate::gecko_string_cache as string_cache;
#[cfg(feature = "gecko")]
pub use crate::gecko_string_cache::Atom;
/// The namespace prefix type for Gecko, which is just an atom.
#[cfg(feature = "gecko")]
pub type Prefix = AtomIdent;
/// The local name of an element for Gecko, which is just an atom.
#[cfg(feature = "gecko")]
pub type LocalName = AtomIdent;
#[cfg(feature = "gecko")]
pub use crate::gecko_string_cache::Namespace;

#[cfg(feature = "servo")]
pub use stylo_atoms::Atom;

#[cfg(feature = "servo")]
#[allow(missing_docs)]
pub type LocalName = GenericAtomIdent<web_atoms::LocalNameStaticSet>;
#[cfg(feature = "servo")]
#[allow(missing_docs)]
pub type Namespace = GenericAtomIdent<web_atoms::NamespaceStaticSet>;
#[cfg(feature = "servo")]
#[allow(missing_docs)]
pub type Prefix = GenericAtomIdent<web_atoms::PrefixStaticSet>;

/// Serialize an identifier which is represented as an atom.
#[cfg(feature = "gecko")]
pub fn serialize_atom_identifier<W>(ident: &Atom, dest: &mut W) -> fmt::Result
where
    W: Write,
{
    ident.with_str(|s| serialize_identifier(s, dest))
}

/// Serialize an identifier which is represented as an atom.
#[cfg(feature = "servo")]
pub fn serialize_atom_identifier<Static, W>(
    ident: &::string_cache::Atom<Static>,
    dest: &mut W,
) -> fmt::Result
where
    Static: string_cache::StaticAtomSet,
    W: Write,
{
    serialize_identifier(&ident, dest)
}

/// Serialize a name which is represented as an Atom.
#[cfg(feature = "gecko")]
pub fn serialize_atom_name<W>(ident: &Atom, dest: &mut W) -> fmt::Result
where
    W: Write,
{
    ident.with_str(|s| serialize_name(s, dest))
}

/// Serialize a name which is represented as an Atom.
#[cfg(feature = "servo")]
pub fn serialize_atom_name<Static, W>(
    ident: &::string_cache::Atom<Static>,
    dest: &mut W,
) -> fmt::Result
where
    Static: string_cache::StaticAtomSet,
    W: Write,
{
    serialize_name(&ident, dest)
}

/// A CSS string stored as an `Atom`.
#[repr(transparent)]
#[derive(Clone, Debug, Default, Deref, Eq, Hash, MallocSizeOf, PartialEq, ToShmem)]
pub struct AtomString(pub Atom);

impl style_traits::SpecifiedValueInfo for AtomString {}

#[cfg(feature = "servo")]
impl AsRef<str> for AtomString {
    fn as_ref(&self) -> &str {
        &*self.0
    }
}

impl Parse for AtomString {
    fn parse<'i>(_: &ParserContext, input: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(Self(Atom::from(input.expect_string()?.as_ref())))
    }
}

impl cssparser::ToCss for AtomString {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: Write,
    {
        // Wrap in quotes to form a string literal
        dest.write_char('"')?;
        #[cfg(feature = "servo")]
        {
            cssparser::CssStringWriter::new(dest).write_str(self.as_ref())?;
        }
        #[cfg(feature = "gecko")]
        {
            self.0
                .with_str(|s| cssparser::CssStringWriter::new(dest).write_str(s))?;
        }
        dest.write_char('"')
    }
}

impl style_traits::ToCss for AtomString {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        cssparser::ToCss::to_css(self, dest)
    }
}

impl PrecomputedHash for AtomString {
    #[inline]
    fn precomputed_hash(&self) -> u32 {
        self.0.precomputed_hash()
    }
}

impl<'a> From<&'a str> for AtomString {
    #[inline]
    fn from(string: &str) -> Self {
        Self(Atom::from(string))
    }
}

/// A generic CSS `<ident>` stored as an `Atom`.
#[cfg(feature = "servo")]
#[repr(transparent)]
#[derive(Deref)]
pub struct GenericAtomIdent<Set>(pub string_cache::Atom<Set>)
where
    Set: string_cache::StaticAtomSet;

/// A generic CSS `<ident>` stored as an `Atom`, for the default atom set.
#[cfg(feature = "servo")]
pub type AtomIdent = GenericAtomIdent<stylo_atoms::AtomStaticSet>;

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> style_traits::SpecifiedValueInfo for GenericAtomIdent<Set> {}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> Default for GenericAtomIdent<Set> {
    fn default() -> Self {
        Self(string_cache::Atom::default())
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> std::fmt::Debug for GenericAtomIdent<Set> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> std::hash::Hash for GenericAtomIdent<Set> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> Eq for GenericAtomIdent<Set> {}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> PartialEq for GenericAtomIdent<Set> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> Clone for GenericAtomIdent<Set> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> to_shmem::ToShmem for GenericAtomIdent<Set> {
    fn to_shmem(&self, builder: &mut to_shmem::SharedMemoryBuilder) -> to_shmem::Result<Self> {
        use std::mem::ManuallyDrop;

        let atom = self.0.to_shmem(builder)?;
        Ok(ManuallyDrop::new(Self(ManuallyDrop::into_inner(atom))))
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> malloc_size_of::MallocSizeOf for GenericAtomIdent<Set> {
    fn size_of(&self, ops: &mut malloc_size_of::MallocSizeOfOps) -> usize {
        self.0.size_of(ops)
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> cssparser::ToCss for GenericAtomIdent<Set> {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: Write,
    {
        serialize_atom_identifier(&self.0, dest)
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> style_traits::ToCss for GenericAtomIdent<Set> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        serialize_atom_identifier(&self.0, dest)
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> PrecomputedHash for GenericAtomIdent<Set> {
    #[inline]
    fn precomputed_hash(&self) -> u32 {
        self.0.precomputed_hash()
    }
}

#[cfg(feature = "servo")]
impl<'a, Set: string_cache::StaticAtomSet> From<&'a str> for GenericAtomIdent<Set> {
    #[inline]
    fn from(string: &str) -> Self {
        Self(string_cache::Atom::from(string))
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> std::borrow::Borrow<string_cache::Atom<Set>>
    for GenericAtomIdent<Set>
{
    #[inline]
    fn borrow(&self) -> &string_cache::Atom<Set> {
        &self.0
    }
}

#[cfg(feature = "servo")]
impl<Set: string_cache::StaticAtomSet> GenericAtomIdent<Set> {
    /// Constructs a new GenericAtomIdent.
    #[inline]
    pub fn new(atom: string_cache::Atom<Set>) -> Self {
        Self(atom)
    }

    /// Cast an atom ref to an AtomIdent ref.
    #[inline]
    pub fn cast<'a>(atom: &'a string_cache::Atom<Set>) -> &'a Self {
        let ptr = atom as *const _ as *const Self;
        // safety: repr(transparent)
        unsafe { &*ptr }
    }
}

/// A CSS `<ident>` stored as an `Atom`.
#[cfg(feature = "gecko")]
#[repr(transparent)]
#[derive(Clone, Debug, Default, Deref, Eq, Hash, MallocSizeOf, PartialEq, ToShmem)]
pub struct AtomIdent(pub Atom);

#[cfg(feature = "gecko")]
impl style_traits::SpecifiedValueInfo for AtomIdent {}

#[cfg(feature = "gecko")]
impl cssparser::ToCss for AtomIdent {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: Write,
    {
        serialize_atom_identifier(&self.0, dest)
    }
}

#[cfg(feature = "gecko")]
impl style_traits::ToCss for AtomIdent {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        cssparser::ToCss::to_css(self, dest)
    }
}

#[cfg(feature = "gecko")]
impl PrecomputedHash for AtomIdent {
    #[inline]
    fn precomputed_hash(&self) -> u32 {
        self.0.precomputed_hash()
    }
}

#[cfg(feature = "gecko")]
impl<'a> From<&'a str> for AtomIdent {
    #[inline]
    fn from(string: &str) -> Self {
        Self(Atom::from(string))
    }
}

#[cfg(feature = "gecko")]
impl AtomIdent {
    /// Constructs a new AtomIdent.
    #[inline]
    pub fn new(atom: Atom) -> Self {
        Self(atom)
    }

    /// Like `Atom::with` but for `AtomIdent`.
    pub unsafe fn with<F, R>(ptr: *const crate::gecko_bindings::structs::nsAtom, callback: F) -> R
    where
        F: FnOnce(&Self) -> R,
    {
        Atom::with(ptr, |atom: &Atom| {
            // safety: repr(transparent)
            let atom = atom as *const Atom as *const AtomIdent;
            callback(&*atom)
        })
    }

    /// Cast an atom ref to an AtomIdent ref.
    #[inline]
    pub fn cast<'a>(atom: &'a Atom) -> &'a Self {
        let ptr = atom as *const _ as *const Self;
        // safety: repr(transparent)
        unsafe { &*ptr }
    }
}

#[cfg(feature = "gecko")]
impl std::borrow::Borrow<crate::gecko_string_cache::WeakAtom> for AtomIdent {
    #[inline]
    fn borrow(&self) -> &crate::gecko_string_cache::WeakAtom {
        self.0.borrow()
    }
}
