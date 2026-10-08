/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computing the values of keyframes and composing keyframe effects, for embedders that
//! implement the [effect stack][stack] themselves.
//!
//! This is the logic of Gecko's `Servo_GetComputedKeyframeValues` and `Servo_AnimationCompose`
//! without the Gecko types.
//!
//! [stack]: https://drafts.csswg.org/web-animations-1/#effect-stack

use crate::applicable_declarations::CascadePriority;
use crate::context::TreeCountingCaches;
use crate::dom::{AttributeTracker, TElement};
use crate::properties::animated_properties::{AnimationValue, AnimationValueMap};
use crate::properties::{
    ComputedValues, KeyframeCustomPropertiesBuilder, OwnedPropertyDeclarationId,
    PropertyDeclaration, PropertyDeclarationBlock, PropertyDeclarationIdSet, StyleBuilder,
};
use crate::selector_parser::PseudoElement;
use crate::stylesheets::container_rule::ContainerSizeQuery;
use crate::stylist::Stylist;
use crate::values::animated::{Animate, Procedure};
use crate::values::computed::easing::TimingFunction;
use crate::values::computed::Context;
use crate::values::generics::easing::BeforeFlag;
use crate::values::specified::animation::AnimationComposition;

/// <https://drafts.csswg.org/web-animations-2/#iteration-composite-operation>
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IterationComposite {
    /// Each iteration is the same.
    #[default]
    Replace,
    /// Each iteration builds on the final value of the previous one.
    Accumulate,
}

/// The interval between two adjacent keyframes for one property.
///
/// An endpoint without a value stands for the underlying value.
#[derive(Clone, Debug)]
pub struct AnimationPropertySegment {
    /// The computed offset of the keyframe the segment starts at.
    pub from_key: f32,
    /// The computed offset of the keyframe the segment ends at.
    pub to_key: f32,
    /// The value at the start of the segment.
    pub from_value: Option<AnimationValue>,
    /// The value at the end of the segment.
    pub to_value: Option<AnimationValue>,
    /// The easing between the two keyframes.
    pub timing_function: Option<TimingFunction>,
    /// How `from_value` is combined with the underlying value.
    pub from_composite: AnimationComposition,
    /// How `to_value` is combined with the underlying value.
    pub to_composite: AnimationComposition,
}

impl AnimationPropertySegment {
    /// The progress within this segment for the given progress of the effect, with the easing
    /// of the segment applied.
    pub fn position(&self, progress: f64, before_flag: BeforeFlag) -> f64 {
        let position =
            (progress - self.from_key as f64) / (self.to_key as f64 - self.from_key as f64);
        match self.timing_function {
            Some(ref function) => function.calculate_output(position, before_flag, 1e-7),
            None => position,
        }
    }
}

/// Compute one of the endpoints for the interpolation interval, compositing it with the
/// underlying value if needed. `None` means "use `endpoint_value` as is".
fn composite_endpoint(
    endpoint_value: Option<&AnimationValue>,
    composite: AnimationComposition,
    underlying_value: Option<&AnimationValue>,
) -> Option<AnimationValue> {
    match endpoint_value {
        Some(endpoint_value) => match composite {
            AnimationComposition::Add => underlying_value?
                .animate(endpoint_value, Procedure::Add)
                .ok(),
            AnimationComposition::Accumulate => underlying_value?
                .animate(endpoint_value, Procedure::Accumulate { count: 1 })
                .ok(),
            AnimationComposition::Replace => None,
        },
        None => underlying_value.cloned(),
    }
}

/// Accumulate one of the endpoints of the animation interval. `None` means "use
/// `endpoint_value` as is".
fn accumulate_endpoint(
    endpoint_value: Option<&AnimationValue>,
    composited_value: Option<AnimationValue>,
    last_value: &AnimationValue,
    current_iteration: u64,
) -> Option<AnimationValue> {
    let count = current_iteration;
    match composited_value {
        Some(endpoint) => last_value
            .animate(&endpoint, Procedure::Accumulate { count })
            .ok()
            .or(Some(endpoint)),
        None => last_value
            .animate(endpoint_value?, Procedure::Accumulate { count })
            .ok(),
    }
}

/// Compose the animation segment, compositing it with `underlying_value` and `last_value` if
/// needed. Returns `None` if one of those is needed but missing.
pub fn compose_animation_segment(
    segment: &AnimationPropertySegment,
    underlying_value: Option<&AnimationValue>,
    last_value: Option<&AnimationValue>,
    iteration_composite: IterationComposite,
    current_iteration: u64,
    total_progress: f64,
    segment_progress: f64,
) -> Option<AnimationValue> {
    let keyframe_from_value = segment.from_value.as_ref();
    let keyframe_to_value = segment.to_value.as_ref();
    let mut composited_from_value = composite_endpoint(
        keyframe_from_value,
        segment.from_composite,
        underlying_value,
    );
    let mut composited_to_value =
        composite_endpoint(keyframe_to_value, segment.to_composite, underlying_value);

    if iteration_composite == IterationComposite::Accumulate && current_iteration > 0 {
        let last_value = last_value.or(underlying_value)?;
        composited_from_value = accumulate_endpoint(
            keyframe_from_value,
            composited_from_value,
            last_value,
            current_iteration,
        );
        composited_to_value = accumulate_endpoint(
            keyframe_to_value,
            composited_to_value,
            last_value,
            current_iteration,
        );
    }

    let from = composited_from_value.as_ref().or(keyframe_from_value)?;
    let to = composited_to_value.as_ref().or(keyframe_to_value)?;

    if segment.to_key == segment.from_key {
        return Some(if total_progress < 0. { from } else { to }.clone());
    }

    Some(
        match from.animate(
            to,
            Procedure::Interpolate {
                progress: segment_progress,
            },
        ) {
            Ok(value) => value,
            Err(()) => if segment_progress < 0.5 { from } else { to }.clone(),
        },
    )
}

/// Compose the segment of one property of an effect on top of the values in `value_map`, which
/// were set by the effects below it in the effect stack.
///
/// `base_value` is the value of the property without any animations, and is only called if it
/// is needed. `last_segment` is the last segment of the property.
pub fn compose(
    value_map: &mut AnimationValueMap,
    base_value: impl FnOnce() -> Option<AnimationValue>,
    property: &OwnedPropertyDeclarationId,
    segment: &AnimationPropertySegment,
    last_segment: &AnimationPropertySegment,
    iteration_composite: IterationComposite,
    progress: f64,
    current_iteration: u64,
    before_flag: BeforeFlag,
) {
    if !property.as_borrowed().is_animatable() {
        return;
    }

    let need_underlying_value = segment.from_value.is_none()
        || segment.to_value.is_none()
        || segment.from_composite != AnimationComposition::Replace
        || segment.to_composite != AnimationComposition::Replace
        || (iteration_composite == IterationComposite::Accumulate
            && current_iteration > 0
            && last_segment.to_value.is_none());

    let base_value_storage;
    let underlying_value = if need_underlying_value {
        match value_map.get(property) {
            Some(value) => Some(value),
            None => {
                base_value_storage = base_value();
                base_value_storage.as_ref()
            },
        }
    } else {
        None
    };
    if need_underlying_value && underlying_value.is_none() {
        return;
    }

    let position = if segment.to_key == segment.from_key {
        progress
    } else {
        segment.position(progress, before_flag)
    };

    if let Some(result) = compose_animation_segment(
        segment,
        underlying_value,
        last_segment.to_value.as_ref(),
        iteration_composite,
        current_iteration,
        progress,
        position,
    ) {
        value_map.insert(property.clone(), result);
    }
}

/// Computes the values of the declarations of one keyframe against `style`, in the order they
/// apply. Each animatable longhand appears at most once, and logical properties are resolved.
///
/// `parent_style` is the style `style` inherits from.
pub fn compute_keyframe_values<E: TElement>(
    element: E,
    pseudo: Option<&PseudoElement>,
    stylist: &Stylist,
    style: &ComputedValues,
    parent_style: Option<&ComputedValues>,
    declarations: &PropertyDeclarationBlock,
) -> Vec<AnimationValue> {
    let mut conditions = Default::default();
    let mut tree_counting_caches = TreeCountingCaches::default();
    let mut context = Context::new_for_animation(
        StyleBuilder::for_derived_style(stylist.device(), Some(stylist), style, parent_style),
        stylist.quirks_mode(),
        &mut conditions,
        ContainerSizeQuery::for_element(element, parent_style, pseudo.is_some()),
        &element,
        &mut tree_counting_caches,
    );

    // FIXME (bug 1883255): The animated value should be better integrated in the cascade.
    {
        let mut builder = KeyframeCustomPropertiesBuilder::new(
            stylist,
            &mut context,
            style.custom_properties().clone(),
        );
        let priority = CascadePriority::same_tree_author_normal_at_root_layer();
        for declaration in declarations.normal_declaration_iter() {
            if let PropertyDeclaration::Custom(ref declaration) = *declaration {
                builder.cascade(&mut context, declaration, priority);
            }
        }
        builder.build(&mut context, &mut AttributeTracker::new(&element));
    }

    let restriction = pseudo.and_then(|pseudo| pseudo.property_restriction());
    let mut seen = PropertyDeclarationIdSet::default();
    let mut values = Vec::new();
    let default_values = stylist.device().default_computed_values();
    for value in declarations.to_animation_value_iter(&mut context, style, default_values) {
        let property = value.id();
        if restriction.is_some_and(|restriction| !property.flags().contains(restriction)) {
            continue;
        }
        // A later declaration wins, for example a physical longhand after the logical one that
        // maps to it.
        if seen.contains(property) {
            values.retain(|other: &AnimationValue| other.id() != property);
        }
        seen.insert(property);
        values.push(value);
    }
    values
}
