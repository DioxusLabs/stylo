/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A lean, crate-local replacement for the `bitflags!` macro.
//!
//! The `bitflags` 2.x crate expands to roughly 2,500 lines of code per
//! invocation (hidden internal types, forwarding macros, iterators with name
//! tables, several formatting impls, and empty extension-trait hooks). With
//! over 50 bitflags types in this crate that adds up to >135,000 lines /
//! ~5 MB of generated code that the compiler has to parse, expand and
//! type-check on every build.
//!
//! This macro generates only the small API subset that this crate actually
//! uses (~100 lines per invocation), while keeping the same invocation
//! syntax, type/const/method names and semantics, so call sites are
//! unchanged.

/// Defines a bitflags type, mirroring the syntax and API subset of the
/// `bitflags` crate that this crate uses.
///
/// Two forms are supported, just like `bitflags` 2.x:
///
/// * The struct-defining form. Note that unlike with the `bitflags` crate,
///   `Debug` must not be derived; a `Debug` impl that lists the contained
///   flags by name is always generated.
///
/// ```ignore
/// bitflags! {
///     /// Docs.
///     #[derive(Clone, Copy)]
///     pub struct MyFlags: u8 {
///         /// Docs.
///         const A = 1 << 0;
///         const B = 1 << 1;
///     }
/// }
/// ```
///
/// * The impl-only form, where the newtype struct (and its derives) are
///   written manually:
///
/// ```ignore
/// #[derive(Clone, Copy, Debug, PartialEq)]
/// pub struct MyFlags(u8);
/// bitflags! {
///     impl MyFlags: u8 {
///         const A = 1 << 0;
///     }
/// }
/// ```
macro_rules! bitflags {
    (
        $(#[$attr:meta])*
        $vis:vis struct $Name:ident : $T:ty {
            $( $(#[$fattr:meta])* const $Flag:ident = $val:expr; )*
        }
    ) => {
        $(#[$attr])*
        $vis struct $Name($T);

        bitflags!(@impl $Name : $T { $( $(#[$fattr])* const $Flag = $val; )* });

        impl ::core::fmt::Debug for $Name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                f.write_str(concat!(stringify!($Name), "("))?;
                crate::flags::debug_flags(f, self.0 as u64, Self::FLAG_TABLE_U64)?;
                f.write_str(")")
            }
        }
    };
    (
        impl $Name:ident : $T:ty {
            $( $(#[$fattr:meta])* const $Flag:ident = $val:expr; )*
        }
    ) => {
        bitflags!(@impl $Name : $T { $( $(#[$fattr])* const $Flag = $val; )* });
    };
    (@impl $Name:ident : $T:ty {
        $( $(#[$fattr:meta])* const $Flag:ident = $val:expr; )*
    }) => {
        #[allow(dead_code)]
        impl $Name {
            $( $(#[$fattr])* pub const $Flag: Self = Self($val); )*

            #[allow(unused_doc_comments, unused_attributes)]
            const FLAG_TABLE: &'static [(&'static str, $Name)] = &[
                $( $(#[$fattr])* (stringify!($Flag), $Name::$Flag), )*
            ];

            #[allow(unused_doc_comments, unused_attributes)]
            const FLAG_TABLE_U64: &'static [(&'static str, u64)] = &[
                $( $(#[$fattr])* (stringify!($Flag), $val as u64), )*
            ];

            /// Returns an empty set of flags.
            #[inline]
            pub const fn empty() -> Self {
                Self(0)
            }

            /// Returns the union of all defined flags.
            #[inline]
            pub const fn all() -> Self {
                let mut bits = 0;
                let mut i = 0;
                while i < Self::FLAG_TABLE.len() {
                    bits |= Self::FLAG_TABLE[i].1.0;
                    i += 1;
                }
                Self(bits)
            }

            /// Returns the underlying bits of this set of flags.
            #[inline]
            pub const fn bits(&self) -> $T {
                self.0
            }

            /// Converts from underlying bits, returning `None` if any bits
            /// don't correspond to a defined flag.
            #[inline]
            pub const fn from_bits(bits: $T) -> Option<Self> {
                if bits & !Self::all().0 == 0 {
                    Some(Self(bits))
                } else {
                    None
                }
            }

            /// Converts from underlying bits, truncating bits that don't
            /// correspond to a defined flag.
            #[inline]
            pub const fn from_bits_truncate(bits: $T) -> Self {
                Self(bits & Self::all().0)
            }

            /// Converts from underlying bits, keeping all bits as-is.
            #[inline]
            pub const fn from_bits_retain(bits: $T) -> Self {
                Self(bits)
            }

            /// Returns whether no flags are set.
            #[inline]
            pub const fn is_empty(&self) -> bool {
                self.0 == 0
            }

            /// Returns whether all defined flags are set.
            #[inline]
            pub const fn is_all(&self) -> bool {
                self.0 & Self::all().0 == Self::all().0
            }

            /// Returns whether `self` has any flags in common with `other`.
            #[inline]
            pub const fn intersects(&self, other: Self) -> bool {
                self.0 & other.0 != 0
            }

            /// Returns whether `self` contains all flags in `other`.
            #[inline]
            pub const fn contains(&self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }

            /// Returns the intersection of `self` and `other`.
            #[inline]
            #[must_use]
            pub const fn intersection(self, other: Self) -> Self {
                Self(self.0 & other.0)
            }

            /// Returns the union of `self` and `other`.
            #[inline]
            #[must_use]
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            /// Returns the flags in `self` that are not in `other`.
            #[inline]
            #[must_use]
            pub const fn difference(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }

            /// Returns the flags in exactly one of `self` and `other`.
            #[inline]
            #[must_use]
            pub const fn symmetric_difference(self, other: Self) -> Self {
                Self(self.0 ^ other.0)
            }

            /// Returns the defined flags that are not in `self`.
            #[inline]
            #[must_use]
            pub const fn complement(self) -> Self {
                Self(!self.0 & Self::all().0)
            }

            /// Inserts the flags in `other` into `self`.
            #[inline]
            pub fn insert(&mut self, other: Self) {
                self.0 |= other.0;
            }

            /// Removes the flags in `other` from `self`.
            #[inline]
            pub fn remove(&mut self, other: Self) {
                self.0 &= !other.0;
            }

            /// Toggles the flags in `other` in `self`.
            #[inline]
            pub fn toggle(&mut self, other: Self) {
                self.0 ^= other.0;
            }

            /// Inserts or removes the flags in `other` based on `value`.
            #[inline]
            pub fn set(&mut self, other: Self, value: bool) {
                if value {
                    self.insert(other);
                } else {
                    self.remove(other);
                }
            }

            /// Iterates over the defined flags contained in `self`.
            #[inline]
            pub fn iter(&self) -> impl Iterator<Item = Self> {
                self.iter_names().map(|(_, flag)| flag)
            }

            /// Iterates over `(name, flag)` pairs for the defined flags
            /// contained in `self`.
            #[inline]
            pub fn iter_names(&self) -> impl Iterator<Item = (&'static str, Self)> {
                let bits = self.0;
                Self::FLAG_TABLE
                    .iter()
                    .filter(move |(_, flag)| flag.0 != 0 && bits & flag.0 == flag.0)
                    .map(|(name, flag)| (*name, Self(flag.0)))
            }
        }

        impl ::core::ops::BitOr for $Name {
            type Output = Self;
            #[inline]
            fn bitor(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }
        }
        impl ::core::ops::BitOrAssign for $Name {
            #[inline]
            fn bitor_assign(&mut self, other: Self) {
                self.0 |= other.0;
            }
        }
        impl ::core::ops::BitAnd for $Name {
            type Output = Self;
            #[inline]
            fn bitand(self, other: Self) -> Self {
                Self(self.0 & other.0)
            }
        }
        impl ::core::ops::BitAndAssign for $Name {
            #[inline]
            fn bitand_assign(&mut self, other: Self) {
                self.0 &= other.0;
            }
        }
        impl ::core::ops::BitXor for $Name {
            type Output = Self;
            #[inline]
            fn bitxor(self, other: Self) -> Self {
                Self(self.0 ^ other.0)
            }
        }
        impl ::core::ops::BitXorAssign for $Name {
            #[inline]
            fn bitxor_assign(&mut self, other: Self) {
                self.0 ^= other.0;
            }
        }
        impl ::core::ops::Sub for $Name {
            type Output = Self;
            #[inline]
            fn sub(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }
        }
        impl ::core::ops::SubAssign for $Name {
            #[inline]
            fn sub_assign(&mut self, other: Self) {
                self.0 &= !other.0;
            }
        }
        impl ::core::ops::Not for $Name {
            type Output = Self;
            #[inline]
            fn not(self) -> Self {
                Self(!self.0 & Self::all().0)
            }
        }
    };
}

/// Writes a ` | `-separated list of the names of the flags contained in
/// `bits`, mirroring the output of the `Debug` impl generated by the
/// `bitflags` crate.
pub(crate) fn debug_flags(
    f: &mut ::core::fmt::Formatter,
    mut bits: u64,
    table: &[(&'static str, u64)],
) -> ::core::fmt::Result {
    let mut first = true;
    for &(name, flag) in table {
        if flag != 0 && bits & flag == flag {
            if !first {
                f.write_str(" | ")?;
            }
            first = false;
            f.write_str(name)?;
            bits &= !flag;
        }
    }
    if bits != 0 {
        if !first {
            f.write_str(" | ")?;
        }
        first = false;
        write!(f, "{:#x}", bits)?;
    }
    if first {
        f.write_str("0x0")?;
    }
    Ok(())
}
