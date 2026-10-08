/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Working out which CSS animations and transitions an element should have from its style,
//! for embedders that own the animation objects.
//!
//! Nothing here holds or mutates animation state: each function returns a description of
//! what the embedder should do.

use crate::dom::TElement;
use crate::properties::animated_properties::AnimationValue;
use crate::properties::longhands::animation_play_state::computed_value::single_value::T as AnimationPlayState;
use crate::properties::{ComputedValues, OwnedPropertyDeclarationId, PropertyDeclarationIdSet};
use crate::selector_parser::PseudoElement;
use crate::servo::animation_compose::{compute_keyframe_values, AnimationPropertySegment};
use crate::servo::animation_timing::EffectTiming;
use crate::shared_lock::SharedRwLockReadGuard;
use crate::stylesheets::keyframes_rule::{KeyframesAnimation, KeyframesStepValue};
use crate::stylist::Stylist;
use crate::values::computed::easing::TimingFunction;
use crate::values::specified::animation::AnimationComposition;
use crate::values::specified::TransitionBehavior;
use crate::Atom;

/// A keyframe whose offset and property values have been computed.
#[derive(Clone, Debug)]
pub struct ComputedKeyframe {
    /// <https://drafts.csswg.org/web-animations-1/#computed-keyframe-offset>
    pub offset: f32,
    /// The easing from this keyframe to the next one.
    pub timing_function: Option<TimingFunction>,
    /// How the values of this keyframe are combined with the underlying value.
    pub composite: AnimationComposition,
    /// The values of this keyframe, with at most one for each property.
    pub values: Vec<AnimationValue>,
}

/// The segments of one property of a keyframe effect, ordered by offset.
#[derive(Clone, Debug)]
pub struct PropertySegments {
    /// The property.
    pub property: OwnedPropertyDeclarationId,
    /// The segments. Never empty, starts at offset 0 and ends at offset 1.
    pub segments: Vec<AnimationPropertySegment>,
}

impl PropertySegments {
    /// The segment to use for the given progress of the effect.
    pub fn segment_for_progress(&self, progress: f64) -> &AnimationPropertySegment {
        let mut index = 0;
        while index + 1 < self.segments.len() && self.segments[index].to_key as f64 <= progress {
            index += 1;
        }
        &self.segments[index]
    }
}

/// Builds the segments of every property used by `keyframes`, which must be ordered by
/// offset. A property without a keyframe at offset 0 or 1 gets a segment endpoint without a
/// value there, which stands for the underlying value.
///
/// The result is ordered by the first use of each property.
pub fn build_property_segments(keyframes: &[ComputedKeyframe]) -> Vec<PropertySegments> {
    struct Endpoint<'a> {
        offset: f32,
        value: Option<&'a AnimationValue>,
        timing_function: Option<&'a TimingFunction>,
        composite: AnimationComposition,
    }

    let mut seen = PropertyDeclarationIdSet::default();
    let mut result = Vec::new();
    for (index, keyframe) in keyframes.iter().enumerate() {
        for value in &keyframe.values {
            let property = value.id();
            if !seen.insert(property) {
                continue;
            }

            let mut endpoints = Vec::new();
            for keyframe in &keyframes[index..] {
                let Some(value) = keyframe.values.iter().find(|value| value.id() == property)
                else {
                    continue;
                };
                if endpoints.is_empty() && keyframe.offset != 0. {
                    endpoints.push(Endpoint {
                        offset: 0.,
                        value: None,
                        timing_function: keyframes
                            .first()
                            .filter(|first| first.offset == 0.)
                            .and_then(|first| first.timing_function.as_ref()),
                        composite: AnimationComposition::Replace,
                    });
                }
                endpoints.push(Endpoint {
                    offset: keyframe.offset,
                    value: Some(value),
                    timing_function: keyframe.timing_function.as_ref(),
                    composite: keyframe.composite,
                });
            }
            if endpoints.last().is_some_and(|last| last.offset != 1.) {
                endpoints.push(Endpoint {
                    offset: 1.,
                    value: None,
                    timing_function: None,
                    composite: AnimationComposition::Replace,
                });
            }
            let segments = endpoints
                .windows(2)
                .map(|pair| AnimationPropertySegment {
                    from_key: pair[0].offset,
                    to_key: pair[1].offset,
                    from_value: pair[0].value.cloned(),
                    to_value: pair[1].value.cloned(),
                    timing_function: pair[0].timing_function.cloned(),
                    from_composite: pair[0].composite,
                    to_composite: pair[1].composite,
                })
                .collect();
            result.push(PropertySegments {
                property: property.to_owned(),
                segments,
            });
        }
    }
    result
}

/// Computes the keyframes of a `@keyframes` rule against `style`.
///
/// `timing_function` and `composition` are the values of `animation-timing-function` and
/// `animation-composition` on the element for this animation, which a keyframe can override.
pub fn compute_css_keyframes<E: TElement>(
    element: E,
    pseudo: Option<&PseudoElement>,
    stylist: &Stylist,
    guard: &SharedRwLockReadGuard,
    style: &ComputedValues,
    parent_style: Option<&ComputedValues>,
    animation: &KeyframesAnimation,
    timing_function: &TimingFunction,
    composition: AnimationComposition,
) -> Vec<ComputedKeyframe> {
    animation
        .steps
        .iter()
        .map(|step| {
            let values = match step.value {
                KeyframesStepValue::ComputedValues => Vec::new(),
                KeyframesStepValue::Declarations { ref block } => compute_keyframe_values(
                    element,
                    pseudo,
                    stylist,
                    style,
                    parent_style,
                    block.read_with(guard),
                ),
            };
            ComputedKeyframe {
                offset: step.start_offset.percentage.0,
                timing_function: Some(match step.get_animation_timing_function(guard) {
                    Some(function) => function.to_computed_value_without_context(),
                    None => timing_function.clone(),
                }),
                composite: step.get_animation_composition(guard).unwrap_or(composition),
                values,
            }
        })
        .collect()
}

/// One entry of the `animation-name` list of an element that names an existing `@keyframes`
/// rule, with the other `animation-*` properties at the same index.
#[derive(Clone, Debug)]
pub struct CssAnimation<'a> {
    /// The index in the `animation-name` list.
    pub index: usize,
    /// The `animation-name`.
    pub name: Atom,
    /// The `@keyframes` rule with that name.
    pub keyframes: &'a KeyframesAnimation,
    /// The timing of the effect, in milliseconds.
    pub timing: EffectTiming,
    /// `animation-timing-function`, which applies between keyframes and not to the effect.
    pub timing_function: TimingFunction,
    /// `animation-composition`.
    pub composition: AnimationComposition,
    /// Whether `animation-play-state` is `paused`.
    pub paused: bool,
}

/// The CSS animations `style` specifies for `element`, in the order of `animation-name`.
///
/// The embedder matches these against its existing CSS animations by name as described in
/// <https://drafts.csswg.org/css-animations-2/#animations>.
pub fn css_animations<'a, E: TElement + 'a>(
    element: E,
    stylist: &'a Stylist,
    style: &'a ComputedValues,
) -> impl Iterator<Item = CssAnimation<'a>> + 'a {
    let ui = style.get_ui();
    ui.animation_name_iter()
        .enumerate()
        .filter_map(move |(index, name)| {
            let name = name.as_atom()?.clone();
            let keyframes = stylist.lookup_keyframes(&name, element)?;
            Some(CssAnimation {
                index,
                name,
                keyframes,
                timing: EffectTiming {
                    delay: ui.animation_delay_mod(index).seconds() as f64 * 1000.,
                    end_delay: 0.,
                    fill: ui.animation_fill_mode_mod(index),
                    iteration_start: 0.,
                    iterations: ui.animation_iteration_count_mod(index).0 as f64,
                    duration: ui.animation_duration_mod(index).seconds() as f64 * 1000.,
                    direction: ui.animation_direction_mod(index),
                },
                timing_function: ui.animation_timing_function_mod(index),
                composition: ui.animation_composition_mod(index),
                paused: ui.animation_play_state_mod(index) == AnimationPlayState::Paused,
            })
        })
}

/// The state of a transition the embedder already has for a property.
#[derive(Clone, Debug)]
pub struct ExistingTransition {
    /// The end value of the transition.
    pub end_value: AnimationValue,
    /// <https://drafts.csswg.org/css-transitions/#transition-reversing-adjusted-start-value>
    pub reversing_adjusted_start_value: AnimationValue,
    /// <https://drafts.csswg.org/css-transitions/#transition-reversing-shortening-factor>
    pub reversing_shortening_factor: f64,
    /// `None` for a [completed transition], otherwise the state of the running transition
    /// at the time of the style change event.
    ///
    /// [completed transition]: https://drafts.csswg.org/css-transitions/#completed-transition
    pub running: Option<RunningTransition>,
}

/// The state of a running transition at the time of the style change event.
#[derive(Clone, Debug)]
pub struct RunningTransition {
    /// The current value of the property in the transition.
    pub current_value: AnimationValue,
    /// The output of the timing function of the transition.
    pub timing_function_output: f64,
}

/// A transition to start.
#[derive(Clone, Debug)]
pub struct NewTransition {
    /// The transitioning property, which is a physical longhand or a custom property.
    pub property: OwnedPropertyDeclarationId,
    /// The start value.
    pub start_value: AnimationValue,
    /// The end value.
    pub end_value: AnimationValue,
    /// The delay between the style change event and the start time, in milliseconds.
    pub delay: f64,
    /// The time between the start time and the end time, in milliseconds.
    pub duration: f64,
    /// The matching `transition-timing-function`.
    pub timing_function: TimingFunction,
    /// <https://drafts.csswg.org/css-transitions/#transition-reversing-adjusted-start-value>
    pub reversing_adjusted_start_value: AnimationValue,
    /// <https://drafts.csswg.org/css-transitions/#transition-reversing-shortening-factor>
    pub reversing_shortening_factor: f64,
}

/// A change to the transitions of an element.
#[derive(Clone, Debug)]
pub enum TransitionUpdate {
    /// Start a transition, after cancelling the running transition for the property and
    /// removing the completed transition for the property, if any.
    Start(NewTransition),
    /// Cancel the running transition for the property.
    Cancel(OwnedPropertyDeclarationId),
    /// Remove the completed transition for the property.
    RemoveCompleted(OwnedPropertyDeclarationId),
}

/// The result of [`transition_updates`].
#[derive(Debug, Default)]
pub struct TransitionUpdates {
    /// The changes to make, with at most one for each property.
    pub updates: Vec<TransitionUpdate>,
    /// The properties with a matching `transition-property` value. The embedder must cancel
    /// or remove its transitions for every other property.
    pub transitioning_properties: PropertyDeclarationIdSet,
}

/// Works out which transitions to start and cancel for a style change, as described in
/// <https://drafts.csswg.org/css-transitions/#starting>.
///
/// `existing_transition` returns the running or completed transition for a property. The
/// caller handles an after-change style of `display: none`, in which all transitions are
/// cancelled.
pub fn transition_updates(
    before_change_style: &ComputedValues,
    after_change_style: &ComputedValues,
    mut existing_transition: impl FnMut(&OwnedPropertyDeclarationId) -> Option<ExistingTransition>,
) -> TransitionUpdates {
    let mut result = TransitionUpdates::default();

    // If a property is specified multiple times, the last matching item of
    // transition-property is the one that is used.
    let mut transition_properties = after_change_style
        .transition_properties()
        .collect::<Vec<_>>();
    transition_properties.reverse();

    let ui = after_change_style.get_ui();
    for transition in transition_properties {
        let property = transition
            .property
            .as_borrowed()
            .to_physical(after_change_style.writing_mode);
        if !result.transitioning_properties.insert(property) {
            continue;
        }

        let index = transition.index;
        let Some(from) = AnimationValue::from_computed_values(property, before_change_style) else {
            continue;
        };
        let Some(to) = AnimationValue::from_computed_values(property, after_change_style) else {
            continue;
        };

        let allow_discrete = ui.transition_behavior_mod(index) == TransitionBehavior::AllowDiscrete;
        let delay = ui.transition_delay_mod(index).seconds() as f64 * 1000.;
        let duration = (ui.transition_duration_mod(index).seconds() as f64 * 1000.).max(0.);
        let combined_duration = duration + delay;
        let transitionable = property.is_animatable()
            && (allow_discrete || !property.is_discrete_animatable())
            && (allow_discrete || from.interpolable_with(&to));

        let owned_property = property.to_owned();
        let existing = existing_transition(&owned_property);
        let running = existing
            .as_ref()
            .and_then(|existing| Some((existing, existing.running.as_ref()?)));
        let completed = existing
            .as_ref()
            .filter(|existing| existing.running.is_none());

        let new_transition =
            |start_value: AnimationValue, end_value: AnimationValue| NewTransition {
                property: owned_property.clone(),
                reversing_adjusted_start_value: start_value.clone(),
                start_value,
                end_value,
                delay,
                duration,
                timing_function: ui.transition_timing_function_mod(index),
                reversing_shortening_factor: 1.,
            };

        // Step 1.
        if running.is_none()
            && from != to
            && transitionable
            && completed.is_none_or(|completed| completed.end_value != to)
            && combined_duration > 0.
        {
            result
                .updates
                .push(TransitionUpdate::Start(new_transition(from, to)));
            continue;
        }

        // Step 2.
        if let Some(completed) = completed {
            if completed.end_value != to {
                result
                    .updates
                    .push(TransitionUpdate::RemoveCompleted(owned_property));
            }
            continue;
        }

        // Step 3 is handled by the caller using `transitioning_properties`.

        // Step 4.
        let Some((existing, running)) = running else {
            continue;
        };
        if existing.end_value == to {
            continue;
        }
        let current_value = &running.current_value;
        // Steps 4.1 and 4.2.
        if *current_value == to
            || combined_duration <= 0.
            || !transitionable
            || !(allow_discrete || current_value.interpolable_with(&to))
        {
            result
                .updates
                .push(TransitionUpdate::Cancel(owned_property));
            continue;
        }

        // Step 4.3.
        if existing.reversing_adjusted_start_value == to {
            let old_factor = existing.reversing_shortening_factor;
            let factor = (running.timing_function_output * old_factor + (1. - old_factor))
                .abs()
                .clamp(0., 1.);
            let mut reversed = new_transition(current_value.clone(), to);
            reversed.reversing_adjusted_start_value = existing.end_value.clone();
            reversed.reversing_shortening_factor = factor;
            if delay < 0. {
                reversed.delay = delay * factor;
            }
            reversed.duration = duration * factor;
            result.updates.push(TransitionUpdate::Start(reversed));
            continue;
        }

        // Step 4.4.
        result.updates.push(TransitionUpdate::Start(new_transition(
            current_value.clone(),
            to,
        )));
    }

    result
}
