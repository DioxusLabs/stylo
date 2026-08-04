/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Types identifying CSS rule types, shared by the style system crates.

use num_derive::FromPrimitive;

/// https://drafts.csswg.org/cssom-1/#dom-cssrule-type
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, FromPrimitive, PartialEq)]
#[repr(u8)]
pub enum CssRuleType {
    // https://drafts.csswg.org/cssom/#the-cssrule-interface
    Style = 1,
    // Charset = 2, // Historical
    Import = 3,
    Media = 4,
    FontFace = 5,
    Page = 6,
    // https://drafts.csswg.org/css-animations-1/#interface-cssrule-idl
    Keyframes = 7,
    Keyframe = 8,
    // https://drafts.csswg.org/cssom/#the-cssrule-interface
    Margin = 9,
    Namespace = 10,
    // https://drafts.csswg.org/css-counter-styles-3/#extentions-to-cssrule-interface
    CounterStyle = 11,
    // https://drafts.csswg.org/css-conditional-3/#extentions-to-cssrule-interface
    Supports = 12,
    // https://www.w3.org/TR/2012/WD-css3-conditional-20120911/#extentions-to-cssrule-interface
    Document = 13,
    // https://drafts.csswg.org/css-fonts/#om-fontfeaturevalues
    FontFeatureValues = 14,
    // After viewport, all rules should return 0 from the API, but we still need
    // a constant somewhere.
    LayerBlock = 16,
    LayerStatement = 17,
    Container = 18,
    FontPaletteValues = 19,
    // 20 is an arbitrary number to use for Property.
    Property = 20,
    Scope = 21,
    // https://drafts.csswg.org/css-transitions-2/#the-cssstartingstylerule-interface
    StartingStyle = 22,
    // https://drafts.csswg.org/css-anchor-position-1/#om-position-try
    PositionTry = 23,
    // https://drafts.csswg.org/css-nesting-1/#nested-declarations-rule
    NestedDeclarations = 24,
    CustomMedia = 25,
    // Internal rule for UA stylesheet appearance-dependent styles.
    AppearanceBase = 26,
    // https://drafts.csswg.org/css-view-transitions-2/#view-transition-rule
    ViewTransition = 27,
}

impl CssRuleType {
    /// Returns a bit that identifies this rule type.
    #[inline]
    pub const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// Set of rule types.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CssRuleTypes(u32);

impl From<CssRuleType> for CssRuleTypes {
    fn from(ty: CssRuleType) -> Self {
        Self(ty.bit())
    }
}

impl CssRuleTypes {
    /// Rules where !important declarations are forbidden.
    pub const IMPORTANT_FORBIDDEN: Self =
        Self(CssRuleType::PositionTry.bit() | CssRuleType::Keyframe.bit());

    /// Rules without element context.
    pub const WITHOUT_ELEMENT_CONTEXT: Self = Self(
        CssRuleType::CounterStyle.bit()
            | CssRuleType::FontFace.bit()
            | CssRuleType::FontFeatureValues.bit()
            | CssRuleType::Page.bit(),
    );

    /// Returns whether the rule is in the current set.
    #[inline]
    pub fn contains(self, ty: CssRuleType) -> bool {
        self.0 & ty.bit() != 0
    }

    /// Returns all the rules specified in the set.
    #[inline]
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Creates a raw CssRuleTypes bitfield.
    #[inline]
    pub fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns whether the rule set is empty.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Inserts a rule type into the set.
    #[inline]
    pub fn insert(&mut self, ty: CssRuleType) {
        self.0 |= ty.bit()
    }

    /// Returns whether any of the types intersect.
    #[inline]
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}
