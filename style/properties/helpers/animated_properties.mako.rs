/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

<%namespace name="helpers" file="/helpers.mako.rs" />

<%
    from data import SYSTEM_FONT_LONGHANDS, to_camel_case
    from itertools import groupby
%>

#[cfg(feature = "gecko")] use crate::gecko_bindings::structs::NonCustomCSSPropertyId;
use crate::properties::{
    longhands::{
        self, visibility::computed_value::T as Visibility,
    },
    CSSWideKeyword, LonghandId,
    PropertyDeclaration, PropertyDeclarationId,
};
#[cfg(feature = "gecko")] use crate::properties::{
    gecko,
    longhands::content_visibility::computed_value::T as ContentVisibility,
    NonCustomPropertyId,
};
use std::ptr;
use std::mem;
use rustc_hash::FxHashMap;
use super::ComputedValues;
#[cfg(feature = "servo")] use crate::context::SharedStyleContext;
use crate::derives::*;
use crate::properties::OwnedPropertyDeclarationId;
use crate::dom::AttributeTracker;
use crate::values::animated::{Animate, Procedure, ToAnimatedValue, ToAnimatedZero};
use crate::values::animated::effects::AnimatedFilter;
#[cfg(feature = "gecko")] use crate::values::computed::TransitionProperty;
use crate::values::computed::{ClipRect, Context};
use crate::values::computed::ToComputedValue;
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use crate::values::generics::effects::Filter;
use void::{self, Void};
use crate::properties_and_values::value::CustomAnimatedValue;
use debug_unreachable::debug_unreachable;

/// Convert NonCustomCSSPropertyId to TransitionProperty
#[cfg(feature = "gecko")]
#[allow(non_upper_case_globals)]
impl From<NonCustomCSSPropertyId> for TransitionProperty {
    fn from(property: NonCustomCSSPropertyId) -> TransitionProperty {
        TransitionProperty::NonCustom(NonCustomPropertyId::from_noncustomcsspropertyid(property).unwrap())
    }
}

/// A collection of AnimationValue that were composed on an element.
/// This HashMap stores the values that are the last AnimationValue to be
/// composed for each TransitionProperty.
pub type AnimationValueMap = FxHashMap<OwnedPropertyDeclarationId, AnimationValue>;

/// An enum to represent a single computed value belonging to an animated
/// property in order to be interpolated with another one. When interpolating,
/// both values need to belong to the same property.
#[derive(Debug, MallocSizeOf)]
#[repr(u16)]
pub enum AnimationValue {
    % for prop in data.longhands:
    /// `${prop.name}`
    % if prop.animatable and not prop.logical:
    ${prop.camel_case}(${prop.animated_type()}),
    % else:
    ${prop.camel_case}(Void),
    % endif
    % endfor
    /// A custom property.
    Custom(CustomAnimatedValue),
}

<%
    animated = []
    unanimated = []
    animated_with_logical = []
    for prop in data.longhands:
        if prop.animatable:
            animated_with_logical.append(prop)
        if prop.animatable and not prop.logical:
            animated.append(prop)
        else:
            unanimated.append(prop)
%>

#[repr(C)]
struct AnimationValueVariantRepr<T> {
    tag: u16,
    value: T
}

impl Clone for AnimationValue {
    #[inline]
    fn clone(&self) -> Self {
        use self::AnimationValue::*;

        <%
            [copy, others] = [list(g) for _, g in groupby(animated, key=lambda x: not x.specified_is_copy())]
        %>

        let self_tag = unsafe { *(self as *const _ as *const u16) };
        if self_tag <= LonghandId::${copy[-1].camel_case} as u16 {
            #[derive(Clone, Copy)]
            #[repr(u16)]
            enum CopyVariants {
                % for prop in copy:
                _${prop.camel_case}(${prop.animated_type()}),
                % endfor
            }

            unsafe {
                let mut out = mem::MaybeUninit::uninit();
                ptr::write(
                    out.as_mut_ptr() as *mut CopyVariants,
                    *(self as *const _ as *const CopyVariants),
                );
                return out.assume_init();
            }
        }

        match *self {
            % for ty, props in groupby(others, key=lambda x: x.animated_type()):
            <% props = list(props) %>
            ${" |\n".join("{}(ref value)".format(prop.camel_case) for prop in props)} => {
                % if len(props) == 1:
                ${props[0].camel_case}(value.clone())
                % else:
                unsafe {
                    let mut out = mem::MaybeUninit::uninit();
                    ptr::write(
                        out.as_mut_ptr() as *mut AnimationValueVariantRepr<${ty}>,
                        AnimationValueVariantRepr {
                            tag: *(self as *const _ as *const u16),
                            value: value.clone(),
                        },
                    );
                    out.assume_init()
                }
                % endif
            }
            % endfor
            Custom(ref animated_value) => Custom(animated_value.clone()),
            _ => unsafe { debug_unreachable!() }
        }
    }
}

impl PartialEq for AnimationValue {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        use self::AnimationValue::*;

        unsafe {
            let this_tag = *(self as *const _ as *const u16);
            let other_tag = *(other as *const _ as *const u16);
            if this_tag != other_tag {
                return false;
            }

            match *self {
                % for ty, props in groupby(animated, key=lambda x: x.animated_type()):
                ${" |\n".join("{}(ref this)".format(prop.camel_case) for prop in props)} => {
                    let other_repr =
                        &*(other as *const _ as *const AnimationValueVariantRepr<${ty}>);
                    *this == other_repr.value
                }
                % endfor
                ${" |\n".join("{}(void)".format(prop.camel_case) for prop in unanimated)} => {
                    void::unreachable(void)
                },
                AnimationValue::Custom(ref this) => {
                    let other_repr =
                        &*(other as *const _ as *const AnimationValueVariantRepr<CustomAnimatedValue>);
                    *this == other_repr.value
                },
            }
        }
    }
}

impl AnimationValue {
    /// Returns the longhand id this animated value corresponds to.
    #[inline]
    pub fn id(&self) -> PropertyDeclarationId<'_> {
        if let AnimationValue::Custom(animated_value) = self {
            return PropertyDeclarationId::Custom(&animated_value.name);
        }

        let id = unsafe { *(self as *const _ as *const LonghandId) };
        debug_assert_eq!(id, match *self {
            % for prop in data.longhands:
            % if prop.animatable and not prop.logical:
            AnimationValue::${prop.camel_case}(..) => LonghandId::${prop.camel_case},
            % else:
            AnimationValue::${prop.camel_case}(void) => void::unreachable(void),
            % endif
            % endfor
            AnimationValue::Custom(..) => unsafe { debug_unreachable!() },
        });
        PropertyDeclarationId::Longhand(id)
    }

    /// Returns whether this value is interpolable with another one.
    pub fn interpolable_with(&self, other: &Self) -> bool {
        self.animate(other, Procedure::Interpolate { progress: 0.5 }).is_ok()
    }

    /// "Uncompute" this animation value in order to be used inside the CSS
    /// cascade.
    pub fn uncompute(&self) -> PropertyDeclaration {
        use crate::properties::longhands;
        use self::AnimationValue::*;

        use super::PropertyDeclarationVariantRepr;

        <%
            keyfunc = lambda x: (x.base_type(), x.specified_type(), x.boxed, x.animation_type != "discrete")
            uncompute_group = {}
            for key, props in groupby(animated, key=keyfunc):
                props = list(props)
                for p in props:
                    uncompute_group[p.ident] = (key, props)
        %>
        type UncomputeFn = fn(&AnimationValue) -> PropertyDeclaration;
        fn uncompute_void(_: &AnimationValue) -> PropertyDeclaration {
            unsafe { debug_unreachable!() }
        }
        % for prop in animated:
        <% (ty, specified, boxed, to_animated), props = uncompute_group[prop.ident] %>
        #[allow(non_snake_case)]
        fn uncompute_${prop.ident}(v: &AnimationValue) -> PropertyDeclaration {
            let AnimationValue::${prop.camel_case}(ref value) = *v else {
                unsafe { debug_unreachable!() }
            };
            % if to_animated:
            let value = ToAnimatedValue::from_animated_value(value.clone());
            % endif
            let value = ${ty}::from_computed_value(&value);
            % if boxed:
            let value = Box::new(value);
            % endif
            % if len(props) == 1:
            PropertyDeclaration::${prop.camel_case}(value)
            % else:
            unsafe {
                let mut out = mem::MaybeUninit::uninit();
                ptr::write(
                    out.as_mut_ptr() as *mut PropertyDeclarationVariantRepr<${specified}>,
                    PropertyDeclarationVariantRepr {
                        tag: *(v as *const _ as *const u16),
                        value,
                    },
                );
                out.assume_init()
            }
            % endif
        }
        % endfor
        static UNCOMPUTE: [UncomputeFn; crate::properties::property_counts::LONGHANDS] = [
            % for prop in data.longhands:
            % if prop.animatable and not prop.logical:
            uncompute_${prop.ident},
            % else:
            uncompute_void,
            % endif
            % endfor
        ];
        if let Custom(ref animated_value) = *self {
            return animated_value.to_declaration();
        }
        let tag = unsafe { *(self as *const _ as *const u16) };
        UNCOMPUTE[tag as usize](self)
    }

    /// Construct an AnimationValue from a property declaration.
    pub fn from_declaration(
        decl: &PropertyDeclaration,
        context: &mut Context,
        style: &ComputedValues,
        initial: &ComputedValues,
        attribute_tracker: &mut AttributeTracker,
    ) -> Option<Self> {
        <%
            keyfunc = lambda x: (
                x.specified_type(),
                x.animated_type(),
                x.boxed,
                x.animation_type not in ["discrete", "none"],
                x.style_struct.inherited,
                x.ident in SYSTEM_FONT_LONGHANDS and engine == "gecko",
            )
        %>

        <%
            from_decl_group = {}
            for key, props in groupby(animated_with_logical, key=keyfunc):
                props = list(props)
                for p in props:
                    from_decl_group[p.ident] = key
        %>
        type FromDeclFn = fn(
            &PropertyDeclaration,
            &mut Context,
            &ComputedValues,
        ) -> Option<AnimationValue>;
        fn from_decl_unanimatable(
            _: &PropertyDeclaration,
            _: &mut Context,
            _: &ComputedValues,
        ) -> Option<AnimationValue> {
            // non animatable properties will get included because of shorthands. ignore.
            None
        }
        % for prop in animated_with_logical:
        <% specified_ty, ty, boxed, to_animated, inherit, system = from_decl_group[prop.ident] %>
        #[allow(non_snake_case)]
        fn from_decl_${prop.ident}(
            decl: &PropertyDeclaration,
            context: &mut Context,
            style: &ComputedValues,
        ) -> Option<AnimationValue> {
            let PropertyDeclaration::${prop.camel_case}(ref value) = *decl else {
                unsafe { debug_unreachable!() }
            };
            let _ = style;
            context.for_non_inherited_property = ${"false" if inherit else "true"};
            % if system:
            if let Some(sf) = value.get_system() {
                gecko::system_font::resolve_system_font(sf, context)
            }
            % endif
            % if boxed:
            let value = (**value).to_computed_value(context);
            % else:
            let value = value.to_computed_value(context);
            % endif
            % if to_animated:
            let value = value.to_animated_value(&crate::values::animated::Context { style });
            % endif

            Some(unsafe {
                let mut out = mem::MaybeUninit::uninit();
                ptr::write(
                    out.as_mut_ptr() as *mut AnimationValueVariantRepr<${ty}>,
                    AnimationValueVariantRepr {
                        tag: LonghandId::${prop.camel_case}.to_physical(context.builder.writing_mode) as u16,
                        value,
                    },
                );
                out.assume_init()
            })
        }
        % endfor
        static FROM_DECL: [FromDeclFn; crate::properties::property_counts::LONGHANDS] = [
            % for prop in data.longhands:
            % if prop.animatable:
            from_decl_${prop.ident},
            % else:
            from_decl_unanimatable,
            % endif
            % endfor
        ];

        type FromKeywordFn = fn(
            CSSWideKeyword,
            &mut Context,
            &ComputedValues,
            &ComputedValues,
        ) -> Option<AnimationValue>;
        fn from_keyword_unanimatable(
            _: CSSWideKeyword,
            _: &mut Context,
            _: &ComputedValues,
            _: &ComputedValues,
        ) -> Option<AnimationValue> {
            None
        }
        % for prop in data.longhands:
        % if prop.animatable and not prop.logical:
        #[allow(non_snake_case)]
        fn from_keyword_${prop.ident}(
            keyword: CSSWideKeyword,
            context: &mut Context,
            style: &ComputedValues,
            initial: &ComputedValues,
        ) -> Option<AnimationValue> {
            let _ = style;
            // FIXME(emilio, bug 1533327): I think revert (and
            // revert-layer) handling is not fine here, but what to
            // do instead?
            //
            // Seems we'd need the computed value as if it was
            // revert, somehow. Treating it as `unset` seems fine
            // for now...
            let style_struct = match keyword {
                % if not prop.style_struct.inherited:
                CSSWideKeyword::Revert |
                CSSWideKeyword::RevertRule |
                CSSWideKeyword::RevertLayer |
                CSSWideKeyword::Unset |
                % endif
                CSSWideKeyword::Initial => {
                    initial.get_${prop.style_struct.name_lower}()
                },
                % if prop.style_struct.inherited:
                CSSWideKeyword::Revert |
                CSSWideKeyword::RevertRule |
                CSSWideKeyword::RevertLayer |
                CSSWideKeyword::Unset |
                % endif
                CSSWideKeyword::Inherit => {
                    context.builder
                           .get_parent_${prop.style_struct.name_lower}()
                },
            };
            let computed = style_struct.clone_${prop.ident}();

            % if prop.animation_type != "discrete":
            let computed = computed.to_animated_value(&crate::values::animated::Context {
                style
            });
            % endif
            Some(AnimationValue::${prop.camel_case}(computed))
        }
        % endif
        % endfor
        static FROM_KEYWORD: [FromKeywordFn; crate::properties::property_counts::LONGHANDS] = [
            % for prop in data.longhands:
            % if prop.animatable and not prop.logical:
            from_keyword_${prop.ident},
            % else:
            from_keyword_unanimatable,
            % endif
            % endfor
        ];

        let animatable = match *decl {
            PropertyDeclaration::CSSWideKeyword(ref declaration) => {
                let id = declaration.id.to_physical(context.builder.writing_mode);
                return FROM_KEYWORD[id as usize](declaration.keyword, context, style, initial);
            },
            PropertyDeclaration::WithVariables(ref declaration) => {
                let mut cache = Default::default();
                let substituted = {
                    let substitution_functions = &context.style().substitution_functions();

                    debug_assert!(
                        context.builder.stylist.is_some(),
                        "Need a Stylist to substitute variables!"
                    );
                    declaration.value.substitute_variables(
                        declaration.id,
                        substitution_functions,
                        context.builder.stylist.unwrap(),
                        context,
                        &mut cache,
                        attribute_tracker,
                    )
                };
                return AnimationValue::from_declaration(
                    &substituted,
                    context,
                    style,
                    initial,
                    attribute_tracker,
                )
            },
            PropertyDeclaration::Custom(ref declaration) => {
                AnimationValue::Custom(CustomAnimatedValue::from_declaration(
                    declaration,
                    context,
                )?)
            },
            _ => {
                let tag = unsafe { *(decl as *const _ as *const u16) };
                if (tag as usize) < crate::properties::property_counts::LONGHANDS {
                    return FROM_DECL[tag as usize](decl, context, style);
                }
                // non animatable properties will get included because of shorthands. ignore.
                return None;
            }
        };
        Some(animatable)
    }

    /// Returns whether the animated value of `property` is different in `before` and `after`, in
    /// order to determine whether a transition should be started.
    /// NOTE(emilio): We don't need to convert to animated values here, if the computed value is
    /// different the animated value should be different too.
    pub fn is_different_for(
        property: PropertyDeclarationId,
        before: &ComputedValues,
        after: &ComputedValues,
    ) -> bool {
        let longhand = match property {
            PropertyDeclarationId::Longhand(id) => id,
            PropertyDeclarationId::Custom(ref name) => {
                // FIXME(bug 1869476): This should use a stylist to determine whether the name
                // corresponds to an inherited custom property and then choose the
                // inherited/non_inherited map accordingly.
                let before = before.custom_properties();
                let before_value = before.inherited.get(*name).or_else(|| before.non_inherited.get(*name));
                let after = after.custom_properties();
                let after_value = after.inherited.get(*name).or_else(|| after.non_inherited.get(*name));
                return before_value != after_value
            }
        };

        type IsDifferentFn = fn(&ComputedValues, &ComputedValues) -> bool;
        fn is_different_unanimatable(_: &ComputedValues, _: &ComputedValues) -> bool {
            false
        }
        % for prop in data.longhands:
        % if prop.animatable and not prop.logical:
        #[allow(non_snake_case)]
        fn is_different_${prop.ident}(before: &ComputedValues, after: &ComputedValues) -> bool {
            !before.${prop.ident}_equals(after)
        }
        % endif
        % endfor
        static IS_DIFFERENT: [IsDifferentFn; crate::properties::property_counts::LONGHANDS] = [
            % for prop in data.longhands:
            % if prop.animatable and not prop.logical:
            is_different_${prop.ident},
            % else:
            is_different_unanimatable,
            % endif
            % endfor
        ];
        IS_DIFFERENT[longhand as usize](before, after)
    }

    /// Get an AnimationValue for an declaration id from a given computed values.
    pub fn from_computed_values(
        property: PropertyDeclarationId,
        style: &ComputedValues,
    ) -> Option<Self> {
        let property = match property {
            PropertyDeclarationId::Longhand(id) => id,
            PropertyDeclarationId::Custom(ref name) => {
                // FIXME(bug 1869476): This should use a stylist to determine whether the name
                // corresponds to an inherited custom property and then choose the
                // inherited/non_inherited map accordingly.
                let p = &style.custom_properties();
                let value = p.inherited.get(*name).or_else(|| p.non_inherited.get(*name));
                return Some(AnimationValue::Custom(CustomAnimatedValue::from_computed(name, value)))
            }
        };

        type FromComputedFn = fn(&ComputedValues) -> Option<AnimationValue>;
        fn from_computed_unanimatable(_: &ComputedValues) -> Option<AnimationValue> {
            None
        }
        % for prop in data.longhands:
        % if prop.animatable and not prop.logical:
        #[allow(non_snake_case)]
        fn from_computed_${prop.ident}(style: &ComputedValues) -> Option<AnimationValue> {
            let computed = style.clone_${prop.ident}();
            Some(AnimationValue::${prop.camel_case}(
            % if prop.animation_type == "discrete":
                computed
            % else:
                computed.to_animated_value(&crate::values::animated::Context { style })
            % endif
            ))
        }
        % endif
        % endfor
        static FROM_COMPUTED: [FromComputedFn; crate::properties::property_counts::LONGHANDS] = [
            % for prop in data.longhands:
            % if prop.animatable and not prop.logical:
            from_computed_${prop.ident},
            % else:
            from_computed_unanimatable,
            % endif
            % endfor
        ];
        FROM_COMPUTED[property as usize](style)
    }

    /// Update `style` with the value of this `AnimationValue`.
    ///
    /// SERVO ONLY: This doesn't properly handle things like updating 'em' units
    /// when animated font-size.
    #[cfg(feature = "servo")]
    pub fn set_in_style_for_servo(&self, style: &mut ComputedValues, context: &SharedStyleContext) {
        type SetInStyleFn = fn(&AnimationValue, &mut ComputedValues);
        fn set_in_style_unanimatable(_: &AnimationValue, _: &mut ComputedValues) {
            unreachable!()
        }
        % for prop in data.longhands:
        % if prop.animatable and not prop.logical:
        #[allow(non_snake_case)]
        fn set_in_style_${prop.ident}(v: &AnimationValue, style: &mut ComputedValues) {
            let AnimationValue::${prop.camel_case}(ref value) = *v else {
                unsafe { debug_unreachable!() }
            };
            let value: longhands::${prop.ident}::computed_value::T =
            % if prop.animation_type != "discrete":
                ToAnimatedValue::from_animated_value(value.clone());
            % else:
                value.clone();
            % endif
            style.mutate_${prop.style_struct.name_lower}().set_${prop.ident}(value);
        }
        % endif
        % endfor
        static SET_IN_STYLE: [SetInStyleFn; crate::properties::property_counts::LONGHANDS] = [
            % for prop in data.longhands:
            % if prop.animatable and not prop.logical:
            set_in_style_${prop.ident},
            % else:
            set_in_style_unanimatable,
            % endif
            % endfor
        ];
        if let AnimationValue::Custom(CustomAnimatedValue { name, value }) = self {
            let registration = context.stylist.get_custom_property_registration(&name);
            match value {
                Some(value) => style.custom_properties.insert(registration, name, value.clone()),
                None => style.custom_properties.remove(registration, name),
            }
            return;
        }
        let tag = unsafe { *(self as *const _ as *const u16) };
        SET_IN_STYLE[tag as usize](self, style)
    }
}

fn animate_discrete<T: Clone>(this: &T, other: &T, procedure: Procedure) -> Result<T, ()> {
    if let Procedure::Interpolate { progress } = procedure {
        Ok(if progress < 0.5 { this.clone() } else { other.clone() })
    } else {
        Err(())
    }
}

impl Animate for AnimationValue {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        Ok(unsafe {
            use self::AnimationValue::*;

            let this_tag = *(self as *const _ as *const u16);
            let other_tag = *(other as *const _ as *const u16);
            if this_tag != other_tag {
                panic!("Unexpected AnimationValue::animate call");
            }

            <%
                keyfunc = lambda x: (x.animated_type(), x.animation_type == "discrete")
                animate_group = {}
                for key, props in groupby(animated, key=keyfunc):
                    for p in props:
                        animate_group[p.ident] = key
            %>
            type AnimateFn = unsafe fn(
                &AnimationValue,
                &AnimationValue,
                Procedure,
            ) -> Result<AnimationValue, ()>;
            unsafe fn animate_unanimatable(
                _: &AnimationValue,
                _: &AnimationValue,
                _: Procedure,
            ) -> Result<AnimationValue, ()> {
                debug_unreachable!()
            }
            % for prop in animated:
            <% ty, discrete = animate_group[prop.ident] %>
            #[allow(non_snake_case)]
            unsafe fn animate_${prop.ident}(
                this: &AnimationValue,
                other: &AnimationValue,
                procedure: Procedure,
            ) -> Result<AnimationValue, ()> {
                let ${prop.camel_case}(ref this_value) = *this else {
                    debug_unreachable!()
                };
                let other_repr =
                    &*(other as *const _ as *const AnimationValueVariantRepr<${ty}>);
                % if discrete:
                let value = animate_discrete(this_value, &other_repr.value, procedure)?;
                % else:
                let value = this_value.animate(&other_repr.value, procedure)?;
                % endif

                let mut out = mem::MaybeUninit::uninit();
                ptr::write(
                    out.as_mut_ptr() as *mut AnimationValueVariantRepr<${ty}>,
                    AnimationValueVariantRepr {
                        tag: *(this as *const _ as *const u16),
                        value,
                    },
                );
                Ok(out.assume_init())
            }
            % endfor
            static ANIMATE: [AnimateFn; crate::properties::property_counts::LONGHANDS] = [
                % for prop in data.longhands:
                % if prop.animatable and not prop.logical:
                animate_${prop.ident},
                % else:
                animate_unanimatable,
                % endif
                % endfor
            ];
            if let Custom(ref self_value) = *self {
                let Custom(ref other_value) = *other else { unreachable!() };
                return Ok(Custom(self_value.animate(other_value, procedure)?));
            }
            ANIMATE[this_tag as usize](self, other, procedure)?
        })
    }
}

<%
    nondiscrete = []
    for prop in animated:
        if prop.animation_type != "discrete":
            nondiscrete.append(prop)
%>

impl ComputeSquaredDistance for AnimationValue {
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        unsafe {
            use self::AnimationValue::*;

            let this_tag = *(self as *const _ as *const u16);
            let other_tag = *(other as *const _ as *const u16);
            if this_tag != other_tag {
                panic!("Unexpected AnimationValue::compute_squared_distance call");
            }

            <%
                distance_group = {}
                for ty, props in groupby(nondiscrete, key=lambda x: x.animated_type()):
                    for p in props:
                        distance_group[p.ident] = ty
            %>
            type DistanceFn = unsafe fn(
                &AnimationValue,
                &AnimationValue,
            ) -> Result<SquaredDistance, ()>;
            unsafe fn distance_err(
                _: &AnimationValue,
                _: &AnimationValue,
            ) -> Result<SquaredDistance, ()> {
                Err(())
            }
            % for prop in nondiscrete:
            <% ty = distance_group[prop.ident] %>
            #[allow(non_snake_case)]
            unsafe fn distance_${prop.ident}(
                this: &AnimationValue,
                other: &AnimationValue,
            ) -> Result<SquaredDistance, ()> {
                let ${prop.camel_case}(ref this_value) = *this else {
                    debug_unreachable!()
                };
                let other_repr =
                    &*(other as *const _ as *const AnimationValueVariantRepr<${ty}>);
                this_value.compute_squared_distance(&other_repr.value)
            }
            % endfor
            <% nondiscrete_idents = set(p.ident for p in nondiscrete) %>
            static DISTANCE: [DistanceFn; crate::properties::property_counts::LONGHANDS] = [
                % for prop in data.longhands:
                % if prop.ident in nondiscrete_idents:
                distance_${prop.ident},
                % else:
                distance_err,
                % endif
                % endfor
            ];
            if (this_tag as usize) >= crate::properties::property_counts::LONGHANDS {
                return Err(());
            }
            DISTANCE[this_tag as usize](self, other)
        }
    }
}

impl ToAnimatedZero for AnimationValue {
    #[inline]
    fn to_animated_zero(&self) -> Result<Self, ()> {
        match *self {
            % for prop in data.longhands:
            % if prop.animatable and not prop.logical and prop.animation_type != "discrete":
            AnimationValue::${prop.camel_case}(ref base) => {
                Ok(AnimationValue::${prop.camel_case}(base.to_animated_zero()?))
            },
            % endif
            % endfor
            AnimationValue::Custom(..) => {
                // TODO(bug 1869185): For some non-universal registered custom properties, it may make sense to implement this.
                Err(())
            },
            _ => Err(()),
        }
    }
}

/// <https://drafts.csswg.org/web-animations-1/#animating-visibility>
impl Animate for Visibility {
    #[inline]
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match procedure {
            Procedure::Interpolate { .. } => {
                let (this_weight, other_weight) = procedure.weights();
                match (*self, *other) {
                    (Visibility::Visible, _) => {
                        Ok(if this_weight > 0.0 { *self } else { *other })
                    },
                    (_, Visibility::Visible) => {
                        Ok(if other_weight > 0.0 { *other } else { *self })
                    },
                    _ => Err(()),
                }
            },
            _ => Err(()),
        }
    }
}

impl ComputeSquaredDistance for Visibility {
    #[inline]
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        Ok(SquaredDistance::from_sqrt(if *self == *other { 0. } else { 1. }))
    }
}

impl ToAnimatedZero for Visibility {
    #[inline]
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

/// <https://drafts.csswg.org/css-contain-3/#content-visibility-animation>
#[cfg(feature = "gecko")]
impl Animate for ContentVisibility {
    #[inline]
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match procedure {
            Procedure::Interpolate { .. } => {
                let (this_weight, other_weight) = procedure.weights();
                match (*self, *other) {
                    (ContentVisibility::Hidden, _) => {
                        Ok(if other_weight > 0.0 { *other } else { *self })
                    },
                    (_, ContentVisibility::Hidden) => {
                        Ok(if this_weight > 0.0 { *self } else { *other })
                    },
                    _ => Err(()),
                }
            },
            _ => Err(()),
        }
    }
}

#[cfg(feature = "gecko")]
impl ComputeSquaredDistance for ContentVisibility {
    #[inline]
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        Ok(SquaredDistance::from_sqrt(if *self == *other { 0. } else { 1. }))
    }
}

#[cfg(feature = "gecko")]
impl ToAnimatedZero for ContentVisibility {
    #[inline]
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

/// <https://drafts.csswg.org/css-transitions/#animtype-rect>
impl Animate for ClipRect {
    #[inline]
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        use crate::values::computed::LengthOrAuto;
        let animate_component = |this: &LengthOrAuto, other: &LengthOrAuto| {
            let result = this.animate(other, procedure)?;
            if let Procedure::Interpolate { .. } = procedure {
                return Ok(result);
            }
            if result.is_auto() {
                // FIXME(emilio): Why? A couple SMIL tests fail without this,
                // but it seems extremely fishy.
                return Err(());
            }
            Ok(result)
        };

        Ok(ClipRect {
            top: animate_component(&self.top, &other.top)?,
            right: animate_component(&self.right, &other.right)?,
            bottom: animate_component(&self.bottom, &other.bottom)?,
            left: animate_component(&self.left, &other.left)?,
        })
    }
}

<%
    FILTER_FUNCTIONS = [ 'Blur', 'Brightness', 'Contrast', 'Grayscale',
                         'HueRotate', 'Invert', 'Opacity', 'Saturate',
                         'Sepia' ]
%>

/// <https://drafts.fxtf.org/filters/#animation-of-filters>
impl Animate for AnimatedFilter {
    fn animate(
        &self,
        other: &Self,
        procedure: Procedure,
    ) -> Result<Self, ()> {
        use crate::values::animated::animate_multiplicative_factor;
        match (self, other) {
            % for func in ['Blur', 'DropShadow', 'Grayscale', 'HueRotate', 'Invert', 'Sepia']:
            (&Filter::${func}(ref this), &Filter::${func}(ref other)) => {
                Ok(Filter::${func}(this.animate(other, procedure)?))
            },
            % endfor
            % for func in ['Brightness', 'Contrast', 'Opacity', 'Saturate']:
            (&Filter::${func}(this), &Filter::${func}(other)) => {
                Ok(Filter::${func}(animate_multiplicative_factor(this.0, other.0, procedure)?.into()))
            },
            % endfor
            _ => Err(()),
        }
    }
}

/// <http://dev.w3.org/csswg/css-transforms/#none-transform-animation>
impl ToAnimatedZero for AnimatedFilter {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        match *self {
            % for func in ['Blur', 'DropShadow', 'Grayscale', 'HueRotate', 'Invert', 'Sepia']:
            Filter::${func}(ref this) => Ok(Filter::${func}(this.to_animated_zero()?)),
            % endfor
            % for func in ['Brightness', 'Contrast', 'Opacity', 'Saturate']:
            Filter::${func}(_) => Ok(Filter::${func}(1.0.into())),
            % endfor
            _ => Err(()),
        }
    }
}
