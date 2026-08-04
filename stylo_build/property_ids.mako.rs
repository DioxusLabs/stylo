/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// This file is a Mako template: http://www.makotemplates.org/
//
// It generates the CSS property identifier types shared by the style system
// crates, and is included into `style_traits::property_ids`.

<% from data import PropertyRestrictions, to_camel_case, RULE_VALUES, SYSTEM_FONT_LONGHANDS, PRIORITARY_PROPERTIES, PRIORITARY_PROPERTY_DEPENDENCIES %>

<%def name="id_set(set_type, ids, is_member)">
<%
    storage = [0] * int((len(ids) - 1 + 32) / 32)
    for i, property in enumerate(ids):
        if is_member(property):
            storage[int(i / 32)] |= 1 << (i % 32)
%>
    ${set_type}::from_storage([${", ".join("0x%x" % word for word in storage)}])
</%def>

<%def name="non_custom_property_id_set(is_member)">
${id_set("NonCustomPropertyIdSet", data.longhands + data.shorthands + data.all_aliases(), is_member)}
</%def>

<%def name="longhand_id_set(is_member)">
${id_set("LonghandIdSet", data.longhands, is_member)}
</%def>

<%def name="prioritary_property_id_set(is_member)">
${id_set("PrioritaryPropertyIdSet", [p for p in data.longhands if p.is_prioritary()], is_member)}
</%def>


/// A module to group various interesting property counts.
pub mod property_counts {
    /// The number of (non-alias) longhand properties.
    pub const LONGHANDS: usize = ${len(data.longhands)};
    /// The number of (non-alias) shorthand properties.
    pub const SHORTHANDS: usize = ${len(data.shorthands)};
    /// The number of aliases.
    pub const ALIASES: usize = ${len(data.all_aliases())};
    /// The number of counted unknown properties.
    pub const COUNTED_UNKNOWN: usize = ${len(data.counted_unknown_properties)};
    /// The number of (non-alias) longhands and shorthands.
    pub const LONGHANDS_AND_SHORTHANDS: usize = LONGHANDS + SHORTHANDS;
    /// The number of non-custom properties.
    pub const NON_CUSTOM: usize = LONGHANDS_AND_SHORTHANDS + ALIASES;
    /// The number of prioritary properties that we have.
    <% longhand_property_names = set(list(map(lambda p: p.name, data.longhands))) %>
    <% enabled_prioritary_properties = PRIORITARY_PROPERTIES.intersection(longhand_property_names) %>
    pub const PRIORITARY: usize = ${len(enabled_prioritary_properties)};
    /// The max number of longhands that a shorthand other than "all" expands to.
    pub const MAX_SHORTHAND_EXPANDED: usize =
        ${max(len(s.sub_properties) for s in data.shorthands_except_all())};
    /// The max amount of longhands that the `all` shorthand will ever contain.
    pub const ALL_SHORTHAND_EXPANDED: usize = ${data.all_shorthand_length};
    /// The number of animatable properties.
    pub const ANIMATABLE: usize = ${sum(1 for prop in data.longhands if prop.animatable)};
}

impl NonCustomPropertyId {
    /// Get the property name.
    #[inline]
    pub fn name(self) -> &'static str {
        static MAP: [&'static str; property_counts::NON_CUSTOM] = [
            % for property in data.longhands + data.shorthands + data.all_aliases():
            "${property.name}",
            % endfor
        ];
        MAP[self.0 as usize]
    }

    /// Returns whether this property is animatable.
    #[inline]
    pub fn is_animatable(self) -> bool {
        static ANIMATABLE: NonCustomPropertyIdSet =
            ${non_custom_property_id_set(lambda p: p.animatable)};
        ANIMATABLE.contains(self)
    }

    /// Whether this property is enabled for all content right now.
    #[inline]
    pub fn enabled_for_all_content(self) -> bool {
        static EXPERIMENTAL: NonCustomPropertyIdSet = ${non_custom_property_id_set(lambda p: p.experimental(engine))};
        static ALWAYS_ENABLED: NonCustomPropertyIdSet = ${non_custom_property_id_set(
            lambda p: (not p.experimental(engine)) and p.enabled_in_content()
        )};

        let passes_pref_check = || {
            % if engine == "gecko":
                gecko_property_enabled(self)
            % else:
                match self.0 {
                % for (index, property) in enumerate(data.longhands + data.shorthands + data.all_aliases()):
                    <% preference = getattr(property, "servo_pref") %>
                    % if preference:
                        ${index} => static_prefs::pref!("${preference}"),
                    % endif %
                % endfor
                    _ => true,
                }
            % endif
        };

        if ALWAYS_ENABLED.contains(self) {
            return true
        }

        if EXPERIMENTAL.contains(self) && passes_pref_check() {
            return true
        }

        false
    }

    /// Returns whether a given rule allows a given property.
    #[inline]
    pub fn allowed_in_rule(self, rule_types: CssRuleTypes) -> bool {
        debug_assert!(
            rule_types.contains(CssRuleType::Keyframe) ||
            rule_types.contains(CssRuleType::Page) ||
            rule_types.contains(CssRuleType::Style) ||
            rule_types.contains(CssRuleType::Scope) ||
            rule_types.contains(CssRuleType::PositionTry),
            "Given rule type does not allow declarations."
        );

        static MAP: [u32; property_counts::NON_CUSTOM] = [
            % for property in data.longhands + data.shorthands + data.all_aliases():
            % for name in RULE_VALUES:
            % if property.rule_types_allowed & RULE_VALUES[name] != 0:
            CssRuleType::${to_camel_case(name)}.bit() |
            % endif
            % endfor
            0,
            % endfor
        ];
        MAP[self.0 as usize] & rule_types.bits() != 0
    }

    /// Statically-known sets of properties explicitly enabled in UA sheets or
    /// chrome contexts, used by the parser layer to implement `allowed_in`.
    #[doc(hidden)]
    #[inline]
    pub fn explicitly_enabled_in_ua_sheets(self) -> bool {
        static ENABLED_IN_UA_SHEETS: NonCustomPropertyIdSet = ${non_custom_property_id_set(
            lambda p: p.explicitly_enabled_in_ua_sheets()
        )};
        ENABLED_IN_UA_SHEETS.contains(self)
    }

    #[doc(hidden)]
    #[inline]
    pub fn explicitly_enabled_in_chrome(self) -> bool {
        static ENABLED_IN_CHROME: NonCustomPropertyIdSet = ${non_custom_property_id_set(
            lambda p: p.explicitly_enabled_in_chrome()
        )};
        ENABLED_IN_CHROME.contains(self)
    }
}

<%
    FIRST_LINE_RESTRICTIONS = PropertyRestrictions.first_line(data)
    FIRST_LETTER_RESTRICTIONS = PropertyRestrictions.first_letter(data)
    MARKER_RESTRICTIONS = PropertyRestrictions.marker(data)
    PLACEHOLDER_RESTRICTIONS = PropertyRestrictions.placeholder(data)
    CUE_RESTRICTIONS = PropertyRestrictions.cue(data)

    def restriction_flags(property):
        name = property.name
        flags = []
        if name in FIRST_LINE_RESTRICTIONS:
            flags.append("APPLIES_TO_FIRST_LINE")
        if name in FIRST_LETTER_RESTRICTIONS:
            flags.append("APPLIES_TO_FIRST_LETTER")
        if name in PLACEHOLDER_RESTRICTIONS:
            flags.append("APPLIES_TO_PLACEHOLDER")
        if name in MARKER_RESTRICTIONS:
            flags.append("APPLIES_TO_MARKER")
        if name in CUE_RESTRICTIONS:
            flags.append("APPLIES_TO_CUE")
        return flags

%>

/// A group for properties which may override each other via logical resolution.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum LogicalGroupId {
    % for i, group in enumerate(data.logical_groups.keys()):
    /// ${group}
    ${to_camel_case(group)} = ${i},
    % endfor
}

impl LogicalGroupId {
    /// Return the list of physical mapped properties for a given logical group.
    #[doc(hidden)]
    pub fn physical_properties(self) -> &'static [LonghandId] {
        static PROPS: [[LonghandId; 4]; ${len(data.logical_groups)}] = [
        % for group, props in data.logical_groups.items():
        [
            <% physical_props = [p for p in props if p.logical][0].all_physical_mapped_properties(data) %>
            % for phys in physical_props:
            LonghandId::${phys.camel_case},
            % endfor
            % for i in range(len(physical_props), 4):
            LonghandId::${physical_props[0].camel_case},
            % endfor
        ],
        % endfor
        ];
        &PROPS[self as usize]
    }
}

/// A set of logical groups.
#[derive(Clone, Copy, Debug, Default, MallocSizeOf, PartialEq)]
pub struct LogicalGroupSet {
    storage: [u32; (${len(data.logical_groups)} - 1 + 32) / 32]
}

impl LogicalGroupSet {
    /// Creates an empty `NonCustomPropertyIdSet`.
    pub fn new() -> Self {
        Self {
            storage: Default::default(),
        }
    }

    /// Return whether the given group is in the set
    #[inline]
    pub fn contains(&self, g: LogicalGroupId) -> bool {
        let bit = g as usize;
        (self.storage[bit / 32] & (1 << (bit % 32))) != 0
    }

    /// Insert a group the set.
    #[inline]
    pub fn insert(&mut self, g: LogicalGroupId) {
        let bit = g as usize;
        self.storage[bit / 32] |= 1 << (bit % 32);
    }
}

/// An id of a property that can be depended on by other properties.
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
#[allow(missing_docs)]
pub enum PrioritaryPropertyId {
    % for p in data.longhands:
    % if p.is_prioritary():
    ${p.camel_case},
    % endif
    % endfor
}

impl PrioritaryPropertyId {
    /// Iterates over all prioritary properties, in declaration (longhand) order.
    #[inline]
    pub fn each() -> impl Iterator<Item = Self> {
        // Safe because `PrioritaryPropertyId` is `#[repr(u8)]` with contiguous discriminants in
        // `0..property_counts::PRIORITARY`.
        (0..property_counts::PRIORITARY as u8).map(|i| unsafe { std::mem::transmute::<u8, Self>(i) })
    }

    /// Converts a PrioritaryPropertyId to a LonghandId.
    #[inline]
    pub fn to_longhand(self) -> LonghandId {
        static PRIORITARY_TO_LONGHAND: [LonghandId; property_counts::PRIORITARY] = [
        % for p in data.longhands:
        % if p.is_prioritary():
            LonghandId::${p.camel_case},
        % endif
        % endfor
        ];
        PRIORITARY_TO_LONGHAND[self as usize]
    }

    /// Converts a LonghandId to a PrioritaryPropertyId.
    #[inline]
    pub fn from_longhand(l: LonghandId) -> Option<Self> {
        static LONGHAND_TO_PRIORITARY: [Option<PrioritaryPropertyId>; property_counts::LONGHANDS] = [
        % for p in data.longhands:
        % if p.is_prioritary():
            Some(PrioritaryPropertyId::${p.camel_case}),
        % else:
            None,
        % endif
        % endfor
        ];
        LONGHAND_TO_PRIORITARY[l as usize]
    }

    /// Returns the set of prioritary properties that must be applied before
    /// this one, i.e. the properties it depends on.
    #[inline]
    pub fn dependencies(self) -> &'static PrioritaryPropertyIdSet {
        static DEPENDENCIES: [PrioritaryPropertyIdSet; property_counts::PRIORITARY] = [
        % for p in data.longhands:
        % if p.is_prioritary():
            ${prioritary_property_id_set(
                lambda dep, p=p: dep.name in PRIORITARY_PROPERTY_DEPENDENCIES[p.name]
            )},
        % endif
        % endfor
        ];
        &DEPENDENCIES[self as usize]
    }
}

impl LonghandIdSet {
    /// The set of non-inherited longhands.
    #[inline]
    pub fn reset() -> &'static Self {
        static RESET: LonghandIdSet = ${longhand_id_set(lambda p: not p.style_struct.inherited)};
        &RESET
    }

    /// The set of longhands animatable in a discrete way.
    #[inline]
    pub fn discrete_animatable() -> &'static Self {
        static DISCRETE_ANIMATABLE: LonghandIdSet = ${longhand_id_set(lambda p: p.animation_type == "discrete")};
        &DISCRETE_ANIMATABLE
    }

    /// The set of logical longhands.
    #[inline]
    pub fn logical() -> &'static Self {
        static LOGICAL: LonghandIdSet = ${longhand_id_set(lambda p: p.logical)};
        &LOGICAL
    }

    /// Returns the set of longhands that are ignored when document colors are
    /// disabled.
    #[inline]
    pub fn ignored_when_colors_disabled() -> &'static Self {
        static IGNORED_WHEN_COLORS_DISABLED: LonghandIdSet = ${longhand_id_set(lambda p: p.ignored_when_colors_disabled)};
        &IGNORED_WHEN_COLORS_DISABLED
    }

    /// Only a few properties are allowed to depend on the visited state of links. When cascading
    /// visited styles, we can save time by only processing these properties.
    pub fn visited_dependent() -> &'static Self {
        static VISITED_DEPENDENT: LonghandIdSet = ${longhand_id_set(lambda p: p.is_visited_dependent())};
        debug_assert!(Self::late_group().contains_all(&VISITED_DEPENDENT));
        &VISITED_DEPENDENT
    }

    /// The set of prioritary longhands.
    #[inline]
    pub fn prioritary_properties() -> &'static Self {
        static PRIORITARY: LonghandIdSet = ${longhand_id_set(lambda p: p.is_prioritary())};
        &PRIORITARY
    }

    /// The set of inherited longhands in the late cascade group.
    #[inline]
    pub fn late_group_only_inherited() -> &'static Self {
        static LATE_GROUP_ONLY_INHERITED: LonghandIdSet = ${longhand_id_set(lambda p: p.style_struct.inherited and not p.is_prioritary())};
        &LATE_GROUP_ONLY_INHERITED
    }

    /// The set of longhands in the late cascade group, i.e. all
    /// non-prioritary longhands.
    #[inline]
    pub fn late_group() -> &'static Self {
        static LATE_GROUP: LonghandIdSet = ${longhand_id_set(lambda p: not p.is_prioritary())};
        &LATE_GROUP
    }

    /// Returns the set of properties that are declared as having no effect on
    /// Gecko <scrollbar> elements or their descendant scrollbar parts.
    #[cfg(debug_assertions)]
    #[cfg(feature = "gecko")]
    #[inline]
    pub fn has_no_effect_on_gecko_scrollbars() -> &'static Self {
        // data.py asserts that has_no_effect_on_gecko_scrollbars is True or
        // False for properties that are inherited and Gecko pref controlled,
        // and is None for all other properties.
        static HAS_NO_EFFECT_ON_SCROLLBARS: LonghandIdSet = ${longhand_id_set(
            lambda p: p.has_effect_on_gecko_scrollbars is False
        )};
        &HAS_NO_EFFECT_ON_SCROLLBARS
    }

    /// Returns the set of margin properties, for the purposes of <h1> use counters / warnings.
    #[inline]
    pub fn margin_properties() -> &'static Self {
        static MARGIN_PROPERTIES: LonghandIdSet = ${longhand_id_set(lambda p: p.logical_group == "margin")};
        &MARGIN_PROPERTIES
    }

    /// Returns the set of border properties for the purpose of disabling native
    /// appearance.
    #[inline]
    pub fn border_background_properties() -> &'static Self {
        static BORDER_BACKGROUND_PROPERTIES: LonghandIdSet = ${longhand_id_set(
            lambda p: (p.logical_group and p.logical_group.startswith("border")) or \
                        p in data.shorthands_by_name["border"].sub_properties or \
                        p in data.shorthands_by_name["background"].sub_properties and \
                        p.name not in ["background-blend-mode", "background-repeat"]
        )};
        &BORDER_BACKGROUND_PROPERTIES
    }

    /// Returns properties that are zoom dependent (basically, that contain lengths).
    #[inline]
    pub fn zoom_dependent() -> &'static Self {
        static ZOOM_DEPENDENT: LonghandIdSet = ${longhand_id_set(lambda p: p.is_zoom_dependent())};
        &ZOOM_DEPENDENT
    }

    /// Note that it's different from zoom_dependent(), as this only includes inherited, physical
    /// properties.
    #[inline]
    pub fn zoom_dependent_inherited_properties() -> &'static Self {
        static ZOOM_DEPENDENT_INHERITED: LonghandIdSet = ${longhand_id_set(lambda p: p.is_inherited_zoom_dependent_property())};
        &ZOOM_DEPENDENT_INHERITED
    }
}

/// An identifier for a given longhand property.
#[derive(Clone, Copy, Eq, Hash, MallocSizeOf, PartialEq, ToShmem)]
#[repr(u16)]
pub enum LonghandId {
    % for i, property in enumerate(data.longhands):
        /// ${property.name}
        ${property.camel_case} = ${i},
    % endfor
}

impl LonghandId {
    /// Returns an iterator over all the shorthands that include this longhand.
    pub fn shorthands(self) -> NonCustomPropertyIterator<ShorthandId> {
        // first generate longhand to shorthands lookup map
        //
        // NOTE(emilio): This currently doesn't exclude the "all" shorthand. It
        // could potentially do so, which would speed up serialization
        // algorithms and what not, I guess.
        <%
            from functools import cmp_to_key
            longhand_to_shorthand_map = {}
            num_sub_properties = {}
            for shorthand in data.shorthands:
                num_sub_properties[shorthand.camel_case] = len(shorthand.sub_properties)
                for sub_property in shorthand.sub_properties:
                    if sub_property.ident not in longhand_to_shorthand_map:
                        longhand_to_shorthand_map[sub_property.ident] = []

                    longhand_to_shorthand_map[sub_property.ident].append(shorthand.camel_case)

            def cmp(a, b):
                return (a > b) - (a < b)

            def preferred_order(x, y):
                # Since we want properties in order from most subproperties to least,
                # reverse the arguments to cmp from the expected order.
                result = cmp(num_sub_properties.get(y, 0), num_sub_properties.get(x, 0))
                if result:
                    return result
                # Fall back to lexicographic comparison.
                return cmp(x, y)

            # Sort the lists of shorthand properties according to preferred order:
            # https://drafts.csswg.org/cssom/#concept-shorthands-preferred-order
            for shorthand_list in longhand_to_shorthand_map.values():
                shorthand_list.sort(key=cmp_to_key(preferred_order))
        %>

        // based on lookup results for each longhand, create result arrays
        static MAP: [&'static [ShorthandId]; property_counts::LONGHANDS] = [
        % for property in data.longhands:
            &[
                % for shorthand in longhand_to_shorthand_map.get(property.ident, []):
                    ShorthandId::${shorthand},
                % endfor
            ],
        % endfor
        ];

        NonCustomPropertyIterator {
            filter: NonCustomPropertyId::from(self).enabled_for_all_content(),
            iter: MAP[self as usize].iter(),
        }
    }

    /// Return the logical group of this longhand property.
    pub fn logical_group(self) -> Option<LogicalGroupId> {
        const LOGICAL_GROUP_IDS: [Option<LogicalGroupId>; property_counts::LONGHANDS] = [
            % for prop in data.longhands:
            % if prop.logical_group:
            Some(LogicalGroupId::${to_camel_case(prop.logical_group)}),
            % else:
            None,
            % endif
            % endfor
        ];
        LOGICAL_GROUP_IDS[self as usize]
    }

    /// Returns PropertyFlags for given longhand property.
    #[inline(always)]
    pub fn flags(self) -> PropertyFlags {
        const FLAGS: [PropertyFlags; property_counts::LONGHANDS] = [
            % for property in data.longhands:
                PropertyFlags::empty()
                % for flag in property.flags + restriction_flags(property):
                    .union(PropertyFlags::${flag})
                % endfor
                ,
            % endfor
        ];
        FLAGS[self as usize]
    }
}

/// An identifier for a given shorthand property.
#[derive(Clone, Copy, Debug, Eq, Hash, MallocSizeOf, PartialEq, ToShmem)]
#[repr(u16)]
pub enum ShorthandId {
    % for i, property in enumerate(data.shorthands):
        /// ${property.name}
        ${property.camel_case} = ${i},
    % endfor
}

impl ShorthandId {
    /// Get the longhand ids that form this shorthand.
    pub fn longhands(self) -> NonCustomPropertyIterator<LonghandId> {
        static MAP: [&'static [LonghandId]; property_counts::SHORTHANDS] = [
        % for property in data.shorthands:
            &[
                % for sub in property.sub_properties:
                    LonghandId::${sub.camel_case},
                % endfor
            ],
        % endfor
        ];
        NonCustomPropertyIterator {
            filter: NonCustomPropertyId::from(self).enabled_for_all_content(),
            iter: MAP[self as usize].iter(),
        }
    }

    /// Returns PropertyFlags for the given shorthand property.
    #[inline]
    pub fn flags(self) -> PropertyFlags {
        const FLAGS: [u16; property_counts::SHORTHANDS] = [
            % for property in data.shorthands:
                % for flag in property.flags:
                    PropertyFlags::${flag}.bits() |
                % endfor
                0,
            % endfor
        ];
        PropertyFlags::from_bits_retain(FLAGS[self as usize])
    }

    /// Returns the order in which this property appears relative to other
    /// shorthands in idl-name-sorting order.
    #[inline]
    pub fn idl_name_sort_order(self) -> u32 {
        <%
            from data import to_idl_name
            ordered = {}
            sorted_shorthands = sorted(data.shorthands, key=lambda p: to_idl_name(p.ident))
            for order, shorthand in enumerate(sorted_shorthands):
                ordered[shorthand.ident] = order
        %>
        static IDL_NAME_SORT_ORDER: [u32; property_counts::SHORTHANDS] = [
            % for property in data.shorthands:
            ${ordered[property.ident]},
            % endfor
        ];
        IDL_NAME_SORT_ORDER[self as usize]
    }
}

/// The counted unknown property list which is used for css use counters.
///
/// FIXME: This should be just #[repr(u8)], but can't be because of ABI issues,
/// see https://bugs.llvm.org/show_bug.cgi?id=44228.
#[derive(Clone, Copy, Debug, Eq, FromPrimitive, Hash, PartialEq)]
#[repr(u32)]
pub enum CountedUnknownProperty {
    % for prop in data.counted_unknown_properties:
    /// ${prop.name}
    ${prop.camel_case},
    % endfor
}

impl CountedUnknownProperty {
    /// Parse the counted unknown property, for testing purposes only.
    pub fn parse_for_testing(property_name: &str) -> Option<Self> {
        ::cssparser::ascii_case_insensitive_phf_map! {
            unknown_ids -> CountedUnknownProperty = {
                % for property in data.counted_unknown_properties:
                "${property.name}" => CountedUnknownProperty::${property.camel_case},
                % endfor
            }
        }
        unknown_ids::get(property_name).cloned()
    }

    /// Returns the underlying index, used for use counter.
    #[inline]
    pub fn bit(self) -> usize {
        self as usize
    }
}

/// An identifier for a given alias property.
#[derive(Clone, Copy, Eq, PartialEq, MallocSizeOf)]
#[repr(u16)]
pub enum AliasId {
    % for i, property in enumerate(data.all_aliases()):
        /// ${property.name}
        ${property.camel_case} = ${i},
    % endfor
}

impl fmt::Debug for AliasId {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let name = NonCustomPropertyId::from(*self).name();
        formatter.write_str(name)
    }
}

impl AliasId {
    /// Returns the property we're aliasing, as a longhand or a shorthand.
    #[inline]
    pub fn aliased_property(self) -> NonCustomPropertyId {
        static MAP: [NonCustomPropertyId; property_counts::ALIASES] = [
        % for alias in data.all_aliases():
            % if alias.original.type() == "longhand":
            NonCustomPropertyId::from_longhand(LonghandId::${alias.original.camel_case}),
            % else:
            <% assert alias.original.type() == "shorthand" %>
            NonCustomPropertyId::from_shorthand(ShorthandId::${alias.original.camel_case}),
            % endif
        % endfor
        ];
        MAP[self as usize]
    }
}
