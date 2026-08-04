/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Common [values][values] used in CSS.
//!
//! [values]: https://drafts.csswg.org/css-values/

#![deny(missing_docs)]

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::typed_om::{KeywordValue, NumericType, NumericValue, ToTyped, TypedValue, UnitValue};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use crate::values::generics::position::IsTreeScoped;
use crate::Atom;
pub use cssparser::{serialize_identifier, serialize_name, CowRcStr, Parser};
pub use cssparser::{SourceLocation, Token};
use selectors::parser::SelectorParseErrorKind;
use std::fmt::{self, Debug, Write};
use style_traits::{CssString, CssWriter, ParseError, StyleParseErrorKind, ToCss};
use thin_vec::ThinVec;
use to_shmem::impl_trivial_to_shmem;

pub use crate::url::CssUrl;

pub mod animated;
pub mod computed;
pub mod distance;
pub mod generics;
pub mod resolved;
pub mod specified;
pub mod tagged_numeric;

/// A CSS float value.
pub type CSSFloat = f32;

/// Normalizes a float value to zero after a set of operations that might turn
/// it into NaN.
#[inline]
pub fn normalize(v: CSSFloat) -> CSSFloat {
    if v.is_nan() {
        0.0
    } else {
        v
    }
}

/// A CSS integer value.
pub type CSSInteger = i32;

pub use style_common::atom_types::{serialize_atom_identifier, serialize_atom_name};

/// Serialize a number with calc, and NaN/infinity handling (if enabled)
pub fn serialize_number<W>(v: f32, dest: &mut CssWriter<W>) -> fmt::Result
where
    W: Write,
{
    serialize_specified_dimension(v, "", /* was_calc = */ false, dest)
}

/// Serialize a specified dimension with unit, calc, and NaN/infinity handling (if enabled)
pub fn serialize_specified_dimension<W>(
    v: f32,
    unit: &str,
    was_calc: bool,
    dest: &mut CssWriter<W>,
) -> fmt::Result
where
    W: Write,
{
    if was_calc {
        dest.write_str("calc(")?;
    }

    if !v.is_finite() {
        // https://drafts.csswg.org/css-values/#calc-error-constants:
        // "While not technically numbers, these keywords act as numeric values,
        // similar to e and pi. Thus to get an infinite length, for example,
        // requires an expression like calc(infinity * 1px)."

        if v.is_nan() {
            dest.write_str("NaN")?;
        } else if v == f32::INFINITY {
            dest.write_str("infinity")?;
        } else if v == f32::NEG_INFINITY {
            dest.write_str("-infinity")?;
        }

        if !unit.is_empty() {
            dest.write_str(" * 1")?;
        }
    } else {
        v.to_css(dest)?;
    }

    dest.write_str(unit)?;

    if was_calc {
        dest.write_char(')')?;
    }
    Ok(())
}

#[cfg(feature = "servo")]
pub use style_common::atom_types::GenericAtomIdent;
pub use style_common::atom_types::{AtomIdent, AtomString};

/// Serialize a value into percentage.
pub fn serialize_percentage<W>(value: CSSFloat, dest: &mut CssWriter<W>) -> fmt::Result
where
    W: Write,
{
    serialize_specified_dimension(value * 100., "%", /* was_calc = */ false, dest)
}

/// Serialize a value into normalized (no NaN/inf serialization) percentage.
pub fn serialize_normalized_percentage<W>(value: CSSFloat, dest: &mut CssWriter<W>) -> fmt::Result
where
    W: Write,
{
    (value * 100.).to_css(dest)?;
    dest.write_char('%')
}

/// Reify a percentage.
pub fn reify_percentage(value: CSSFloat, dest: &mut ThinVec<TypedValue>) -> Result<(), ()> {
    let numeric_value = NumericValue::Unit(UnitValue {
        numeric_type: NumericType::percent(),
        value: value * 100.,
        unit: CssString::from("percent"),
    });

    dest.push(TypedValue::Numeric(numeric_value));
    Ok(())
}

/// Convenience void type to disable some properties and values through types.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
)]
pub enum Impossible {}

// FIXME(nox): This should be derived but the derive code cannot cope
// with uninhabited enums.
impl ComputeSquaredDistance for Impossible {
    #[inline]
    fn compute_squared_distance(&self, _other: &Self) -> Result<SquaredDistance, ()> {
        match *self {}
    }
}

impl_trivial_to_shmem!(Impossible);

impl Parse for Impossible {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }
}

/// A struct representing one of two kinds of values.
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    MallocSizeOf,
    PartialEq,
    Parse,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToAnimatedZero,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
pub enum Either<A, B> {
    /// The first value.
    First(A),
    /// The second kind of value.
    Second(B),
}

impl<A: Debug, B: Debug> Debug for Either<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Either::First(ref v) => v.fmt(f),
            Either::Second(ref v) => v.fmt(f),
        }
    }
}

/// <https://drafts.csswg.org/css-values-4/#custom-idents>
#[derive(
    Clone,
    Debug,
    Default,
    Deserialize,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    Serialize,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C)]
pub struct CustomIdent(pub Atom);

impl CustomIdent {
    /// Parse a <custom-ident>
    ///
    /// TODO(zrhoffman, bug 1844501): Use CustomIdent::parse in more places instead of
    /// CustomIdent::from_ident.
    pub fn parse<'i, 't>(
        input: &mut Parser<'i, 't>,
        invalid: &[&str],
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        CustomIdent::from_ident(location, ident, invalid)
    }

    /// Parse an already-tokenizer identifier
    pub fn from_ident<'i>(
        location: SourceLocation,
        ident: &CowRcStr<'i>,
        excluding: &[&str],
    ) -> Result<Self, ParseError<'i>> {
        if !Self::is_valid(ident, excluding) {
            return Err(
                location.new_custom_error(SelectorParseErrorKind::UnexpectedIdent(ident.clone()))
            );
        }
        if excluding.iter().any(|s| ident.eq_ignore_ascii_case(s)) {
            Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError))
        } else {
            Ok(CustomIdent(Atom::from(ident.as_ref())))
        }
    }

    fn is_valid(ident: &str, excluding: &[&str]) -> bool {
        use crate::properties::CSSWideKeyword;
        // https://drafts.csswg.org/css-values-4/#custom-idents:
        //
        //     The CSS-wide keywords are not valid <custom-ident>s. The default
        //     keyword is reserved and is also not a valid <custom-ident>.
        if CSSWideKeyword::from_ident(ident).is_ok() || ident.eq_ignore_ascii_case("default") {
            return false;
        }

        // https://drafts.csswg.org/css-values-4/#custom-idents:
        //
        //     Excluded keywords are excluded in all ASCII case permutations.
        !excluding.iter().any(|s| ident.eq_ignore_ascii_case(s))
    }
}

impl ToCss for CustomIdent {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        serialize_atom_identifier(&self.0, dest)
    }
}

impl ToTyped for CustomIdent {
    fn to_typed(&self, dest: &mut ThinVec<TypedValue>) -> Result<(), ()> {
        // This shouldn't escape identifiers. See bug 2023533.
        let s = ToCss::to_css_cssstring(self);
        dest.push(TypedValue::Keyword(KeywordValue(s)));
        Ok(())
    }
}

/// <https://www.w3.org/TR/css-values-4/#dashed-idents>
/// This is simply an Atom, but will only parse if the identifier starts with "--".
#[repr(transparent)]
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    Serialize,
    Deserialize,
)]
pub struct DashedIdent(pub Atom);

impl DashedIdent {
    /// Parse an already-tokenizer identifier
    pub fn from_ident<'i>(
        location: SourceLocation,
        ident: &CowRcStr<'i>,
    ) -> Result<Self, ParseError<'i>> {
        if !ident.starts_with("--") {
            return Err(
                location.new_custom_error(SelectorParseErrorKind::UnexpectedIdent(ident.clone()))
            );
        }
        Ok(Self(Atom::from(ident.as_ref())))
    }

    /// Special value for internal use. Useful where we can't use Option<>.
    pub fn empty() -> Self {
        Self(atom!(""))
    }

    /// Check for special internal value.
    pub fn is_empty(&self) -> bool {
        self.0 == atom!("")
    }

    /// Returns an atom with the same value, but without the starting "--".
    ///
    /// # Panics
    ///
    /// Panics when used on the special `DashedIdent::empty()`.
    pub(crate) fn undashed(&self) -> Atom {
        assert!(!self.is_empty(), "Can't undash the empty DashedIdent");
        #[cfg(feature = "gecko")]
        let name = &self.0.as_slice()[2..];
        #[cfg(feature = "servo")]
        let name = &self.0[2..];
        Atom::from(name)
    }
}

impl IsTreeScoped for DashedIdent {
    fn is_tree_scoped(&self) -> bool {
        !self.is_empty()
    }
}

impl Parse for DashedIdent {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        Self::from_ident(location, ident)
    }
}

impl ToCss for DashedIdent {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        serialize_atom_identifier(&self.0, dest)
    }
}

/// The <keyframes-name>.
///
/// <https://drafts.csswg.org/css-animations/#typedef-keyframes-name>
///
/// We use a single atom for this. Empty atom represents `none` animation.
#[repr(transparent)]
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    MallocSizeOf,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
pub struct KeyframesName(Atom);

impl KeyframesName {
    /// <https://drafts.csswg.org/css-animations/#dom-csskeyframesrule-name>
    pub fn from_ident(value: &str) -> Self {
        Self(Atom::from(value))
    }

    /// Returns the `none` value.
    pub fn none() -> Self {
        Self(atom!(""))
    }

    /// Returns whether this is the special `none` value.
    pub fn is_none(&self) -> bool {
        self.0 == atom!("")
    }

    /// Create a new KeyframesName from Atom.
    #[cfg(feature = "gecko")]
    pub fn from_atom(atom: Atom) -> Self {
        Self(atom)
    }

    /// The name as an Atom
    pub fn as_atom(&self) -> &Atom {
        &self.0
    }
}

impl Parse for KeyframesName {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        Ok(match *input.next()? {
            Token::Ident(ref s) => Self(CustomIdent::from_ident(location, s, &["none"])?.0),
            // Note that empty <string> should be rejected.
            Token::QuotedString(ref s) if !s.as_ref().is_empty() => Self(Atom::from(s.as_ref())),
            ref t => return Err(location.new_unexpected_token_error(t.clone())),
        })
    }
}

impl ToCss for KeyframesName {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if self.is_none() {
            return dest.write_str("none");
        }

        fn serialize<W: Write>(string: &str, dest: &mut CssWriter<W>) -> fmt::Result {
            if CustomIdent::is_valid(string, &["none"]) {
                serialize_identifier(string, dest)
            } else {
                string.to_css(dest)
            }
        }

        #[cfg(feature = "gecko")]
        return self.0.with_str(|s| serialize(s, dest));

        #[cfg(feature = "servo")]
        return serialize(self.0.as_ref(), dest);
    }
}

impl ToTyped for KeyframesName {
    fn to_typed(&self, dest: &mut ThinVec<TypedValue>) -> Result<(), ()> {
        let s = ToCss::to_css_cssstring(self);
        dest.push(TypedValue::Keyword(KeywordValue(s)));
        Ok(())
    }
}
