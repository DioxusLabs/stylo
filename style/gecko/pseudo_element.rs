/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Gecko's definition of a pseudo-element.
//!
//! Note that a few autogenerated bits of this live in
//! `pseudo_element_definition.mako.rs`. If you touch that file, you probably
//! need to update the checked-in files for Servo.

use crate::gecko_bindings::structs::{self, PseudoStyleType};
use crate::properties::longhands::display::computed_value::T as Display;
use crate::properties::{ComputedValues, PropertyFlags};
use crate::selector_parser::{PseudoElementCascadeType, SelectorImpl};
use crate::str::{starts_with_ignore_ascii_case, string_as_ascii_lowercase};
use crate::string_cache::Atom;
use crate::values::serialize_atom_identifier;
use crate::values::AtomIdent;
use cssparser::ToCss;
use static_prefs::pref;
use std::fmt;

include!(concat!(
    env!("OUT_DIR"),
    "/gecko/pseudo_element_definition.rs"
));

impl ::selectors::parser::PseudoElement for PseudoElement {
    type Impl = SelectorImpl;

    // ::slotted() should support all tree-abiding pseudo-elements, see
    // https://drafts.csswg.org/css-scoping/#slotted-pseudo
    // https://drafts.csswg.org/css-pseudo-4/#treelike
    #[inline]
    fn valid_after_slotted(&self) -> bool {
        matches!(
            *self,
            Self::Before |
                Self::After |
                Self::Marker |
                Self::Placeholder |
                Self::FileSelectorButton |
                Self::DetailsContent
        )
    }

    #[inline]
    fn accepts_state_pseudo_classes(&self) -> bool {
        // Note: if the pseudo element is a descendants of a pseudo element, `only-child` should be
        // allowed after it.
        self.supports_user_action_state() || self.is_in_pseudo_element_tree()
    }

    #[inline]
    fn specificity_count(&self) -> u32 {
        self.specificity_count()
    }

    #[inline]
    fn is_in_pseudo_element_tree(&self) -> bool {
        // All the named view transition pseudo-elements are the descendants of a pseudo-element
        // root.
        self.is_named_view_transition()
    }
}

impl PseudoElement {
    /// Returns the kind of cascade type that a given pseudo is going to use.
    ///
    /// In Gecko we only compute ::before and ::after eagerly. We save the rules
    /// for anonymous boxes separately, so we resolve them as precomputed
    /// pseudos.
    ///
    /// We resolve the others lazily, see `Servo_ResolvePseudoStyle`.
    pub fn cascade_type(&self) -> PseudoElementCascadeType {
        if self.is_eager() {
            debug_assert!(!self.is_anon_box());
            return PseudoElementCascadeType::Eager;
        }

        if self.is_precomputed() {
            return PseudoElementCascadeType::Precomputed;
        }

        PseudoElementCascadeType::Lazy
    }

    /// Gets the canonical index of this eagerly-cascaded pseudo-element.
    #[inline]
    pub fn eager_index(&self) -> usize {
        EAGER_PSEUDOS
            .iter()
            .position(|p| p == self)
            .expect("Not an eager pseudo")
    }

    /// Creates a pseudo-element from an eager index.
    #[inline]
    pub fn from_eager_index(i: usize) -> Self {
        EAGER_PSEUDOS[i].clone()
    }

    /// Whether animations for the current pseudo element are stored in the
    /// parent element.
    #[inline]
    pub fn animations_stored_in_parent(&self) -> bool {
        matches!(*self, Self::Before | Self::After | Self::Marker)
    }

    /// Whether the current pseudo element is ::before or ::after.
    #[inline]
    pub fn is_before_or_after(&self) -> bool {
        matches!(*self, Self::Before | Self::After)
    }

    /// Whether this pseudo-element is the ::before pseudo.
    #[inline]
    pub fn is_before(&self) -> bool {
        *self == PseudoElement::Before
    }

    /// Whether this pseudo-element is the ::after pseudo.
    #[inline]
    pub fn is_after(&self) -> bool {
        *self == PseudoElement::After
    }

    /// Whether this pseudo-element is the ::marker pseudo.
    #[inline]
    pub fn is_marker(&self) -> bool {
        *self == PseudoElement::Marker
    }

    /// Whether this pseudo-element is the ::selection pseudo.
    #[inline]
    pub fn is_selection(&self) -> bool {
        *self == PseudoElement::Selection
    }

    /// Whether this pseudo-element is ::first-letter.
    #[inline]
    pub fn is_first_letter(&self) -> bool {
        *self == PseudoElement::FirstLetter
    }

    /// Whether this pseudo-element is ::first-line.
    #[inline]
    pub fn is_first_line(&self) -> bool {
        *self == PseudoElement::FirstLine
    }

    /// Whether this pseudo-element is the ::-moz-color-swatch pseudo.
    #[inline]
    pub fn is_color_swatch(&self) -> bool {
        *self == PseudoElement::MozColorSwatch
    }

    /// Whether this pseudo-element is lazily-cascaded.
    #[inline]
    pub fn is_lazy(&self) -> bool {
        !self.is_eager() && !self.is_precomputed()
    }

    /// The identifier of the highlight this pseudo-element represents.
    pub fn highlight_name(&self) -> Option<&AtomIdent> {
        match *self {
            Self::Highlight(ref name) => Some(name),
            _ => None,
        }
    }

    /// Whether this pseudo-element is the ::highlight pseudo.
    pub fn is_highlight(&self) -> bool {
        matches!(*self, Self::Highlight(_))
    }

    /// Whether this pseudo-element is the ::target-text pseudo.
    #[inline]
    pub fn is_target_text(&self) -> bool {
        *self == PseudoElement::TargetText
    }

    /// Whether this pseudo-element is a named view transition pseudo-element.
    pub fn is_named_view_transition(&self) -> bool {
        matches!(
            *self,
            Self::ViewTransitionGroup(..) |
                Self::ViewTransitionImagePair(..) |
                Self::ViewTransitionOld(..) |
                Self::ViewTransitionNew(..)
        )
    }

    /// Whether this pseudo-element is "part-like", which means that it inherits from its regular
    /// flat tree parent, which might not be the originating element.
    pub fn is_part_like(&self) -> bool {
        self.is_named_view_transition() || *self == PseudoElement::DetailsContent
    }

    /// The count we contribute to the specificity from this pseudo-element.
    pub fn specificity_count(&self) -> u32 {
        match *self {
            Self::ViewTransitionGroup(ref name) |
            Self::ViewTransitionImagePair(ref name) |
            Self::ViewTransitionOld(ref name) |
            Self::ViewTransitionNew(ref name) => {
                // The specificity of a named view transition pseudo-element selector with a
                // `<custom-ident>` argument is equivalent to a type selector.
                // The specificity of a named view transition pseudo-element selector with a `*`
                // argument is zero.
                (name.0 != atom!("*")) as u32
            },
            _ => 1,
        }
    }

    /// Whether this pseudo-element supports user action selectors.
    pub fn supports_user_action_state(&self) -> bool {
        (self.flags() & structs::CSS_PSEUDO_ELEMENT_SUPPORTS_USER_ACTION_STATE) != 0
    }

    /// Whether this pseudo-element is enabled for all content.
    pub fn enabled_in_content(&self) -> bool {
        match *self {
            Self::Highlight(..) => pref!("dom.customHighlightAPI.enabled"),
            Self::TargetText => pref!("dom.text_fragments.enabled"),
            Self::SliderFill | Self::SliderTrack | Self::SliderThumb => {
                pref!("layout.css.modern-range-pseudos.enabled")
            },
            Self::DetailsContent => {
                pref!("layout.css.details-content.enabled")
            },
            Self::ViewTransition |
            Self::ViewTransitionGroup(..) |
            Self::ViewTransitionImagePair(..) |
            Self::ViewTransitionOld(..) |
            Self::ViewTransitionNew(..) => pref!("dom.viewTransitions.enabled"),
            // If it's not explicitly enabled in UA sheets or chrome, then we're enabled for
            // content.
            _ => (self.flags() & structs::CSS_PSEUDO_ELEMENT_ENABLED_IN_UA_SHEETS_AND_CHROME) == 0,
        }
    }

    /// Whether this pseudo is enabled explicitly in UA sheets.
    pub fn enabled_in_ua_sheets(&self) -> bool {
        (self.flags() & structs::CSS_PSEUDO_ELEMENT_ENABLED_IN_UA_SHEETS) != 0
    }

    /// Whether this pseudo is enabled explicitly in chrome sheets.
    pub fn enabled_in_chrome(&self) -> bool {
        (self.flags() & structs::CSS_PSEUDO_ELEMENT_ENABLED_IN_CHROME) != 0
    }

    /// Whether this pseudo-element skips flex/grid container display-based
    /// fixup.
    #[inline]
    pub fn skip_item_display_fixup(&self) -> bool {
        (self.flags() & structs::CSS_PSEUDO_ELEMENT_IS_FLEX_OR_GRID_ITEM) == 0
    }

    /// Whether this pseudo-element is precomputed.
    #[inline]
    pub fn is_precomputed(&self) -> bool {
        self.is_anon_box() && !self.is_tree_pseudo_element()
    }

    /// Property flag that properties must have to apply to this pseudo-element.
    #[inline]
    pub fn property_restriction(&self) -> Option<PropertyFlags> {
        Some(match *self {
            PseudoElement::FirstLetter => PropertyFlags::APPLIES_TO_FIRST_LETTER,
            PseudoElement::FirstLine => PropertyFlags::APPLIES_TO_FIRST_LINE,
            PseudoElement::Placeholder => PropertyFlags::APPLIES_TO_PLACEHOLDER,
            PseudoElement::Cue => PropertyFlags::APPLIES_TO_CUE,
            PseudoElement::Marker if static_prefs::pref!("layout.css.marker.restricted") => {
                PropertyFlags::APPLIES_TO_MARKER
            },
            _ => return None,
        })
    }

    /// Whether this pseudo-element should actually exist if it has
    /// the given styles.
    pub fn should_exist(&self, style: &ComputedValues) -> bool {
        debug_assert!(self.is_eager());

        if style.get_box().clone_display() == Display::None {
            return false;
        }

        if self.is_before_or_after() && style.ineffective_content_property() {
            return false;
        }

        true
    }
}
