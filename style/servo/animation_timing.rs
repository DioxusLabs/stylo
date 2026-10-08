/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The timing calculations of the Web Animations timing model, as pure functions.
//!
//! <https://drafts.csswg.org/web-animations-1/#timing-model>
//!
//! Times are in milliseconds. An unresolved time is `None`.

use crate::values::computed::easing::TimingFunction;
use crate::values::computed::{AnimationDirection, AnimationFillMode};
use crate::values::generics::easing::BeforeFlag;

/// The timing properties of an animation effect.
///
/// <https://drafts.csswg.org/web-animations-1/#animation-effects>
#[derive(Clone, Debug, PartialEq)]
pub struct EffectTiming {
    /// <https://drafts.csswg.org/web-animations-1/#start-delay>
    pub delay: f64,
    /// <https://drafts.csswg.org/web-animations-1/#end-delay>
    pub end_delay: f64,
    /// <https://drafts.csswg.org/web-animations-1/#fill-mode>
    pub fill: AnimationFillMode,
    /// <https://drafts.csswg.org/web-animations-1/#iteration-start>
    pub iteration_start: f64,
    /// <https://drafts.csswg.org/web-animations-1/#iteration-count>
    ///
    /// May be infinite.
    pub iterations: f64,
    /// <https://drafts.csswg.org/web-animations-1/#iteration-duration>
    ///
    /// May be infinite.
    pub duration: f64,
    /// <https://drafts.csswg.org/web-animations-1/#playback-direction>
    pub direction: AnimationDirection,
}

impl Default for EffectTiming {
    fn default() -> Self {
        Self {
            delay: 0.,
            end_delay: 0.,
            fill: AnimationFillMode::None,
            iteration_start: 0.,
            iterations: 1.,
            duration: 0.,
            direction: AnimationDirection::Normal,
        }
    }
}

/// <https://drafts.csswg.org/web-animations-1/#animation-effect-phases-and-states>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// The local time is unresolved.
    Idle,
    /// The local time is before the active interval.
    Before,
    /// The local time is within the active interval.
    Active,
    /// The local time is after the active interval.
    After,
}

/// The direction in which the current iteration is played.
///
/// <https://drafts.csswg.org/web-animations-1/#directed-progress>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentDirection {
    /// The iteration is played from start to end.
    Forwards,
    /// The iteration is played from end to start.
    Reverse,
}

impl EffectTiming {
    /// <https://drafts.csswg.org/web-animations-1/#active-duration>
    pub fn active_duration(&self) -> f64 {
        if self.duration == 0. || self.iterations == 0. {
            return 0.;
        }
        self.duration * self.iterations
    }

    /// <https://drafts.csswg.org/web-animations-1/#end-time>
    pub fn end_time(&self) -> f64 {
        (self.delay + self.active_duration() + self.end_delay).max(0.)
    }

    /// The phase of the effect at `local_time`.
    ///
    /// `playback_rate` is that of the associated animation: when it is negative, a local time
    /// on a phase boundary belongs to the earlier phase.
    ///
    /// <https://drafts.csswg.org/web-animations-1/#animation-effect-phases-and-states>
    pub fn phase(&self, local_time: Option<f64>, playback_rate: f64) -> Phase {
        let Some(local_time) = local_time else {
            return Phase::Idle;
        };
        let backwards = playback_rate < 0.;
        let end_time = self.end_time();
        let before_active_boundary = self.delay.min(end_time).max(0.);
        if local_time < before_active_boundary
            || (backwards && local_time == before_active_boundary)
        {
            return Phase::Before;
        }
        let active_after_boundary = (self.delay + self.active_duration()).min(end_time).max(0.);
        if local_time > active_after_boundary || (!backwards && local_time == active_after_boundary)
        {
            return Phase::After;
        }
        Phase::Active
    }

    /// <https://drafts.csswg.org/web-animations-1/#active-time>
    pub fn active_time(&self, local_time: Option<f64>, phase: Phase) -> Option<f64> {
        let local_time = local_time?;
        let fill = self.fill;
        match phase {
            Phase::Idle => None,
            Phase::Before => {
                if !matches!(fill, AnimationFillMode::Backwards | AnimationFillMode::Both) {
                    return None;
                }
                Some((local_time - self.delay).max(0.))
            },
            Phase::Active => Some(local_time - self.delay),
            Phase::After => {
                if !matches!(fill, AnimationFillMode::Forwards | AnimationFillMode::Both) {
                    return None;
                }
                Some(
                    (local_time - self.delay)
                        .min(self.active_duration())
                        .max(0.),
                )
            },
        }
    }

    /// <https://drafts.csswg.org/web-animations-1/#overall-progress>
    pub fn overall_progress(&self, active_time: Option<f64>, phase: Phase) -> Option<f64> {
        let active_time = active_time?;
        let overall_progress = if self.duration == 0. {
            if phase == Phase::Before {
                0.
            } else {
                self.iterations
            }
        } else {
            active_time / self.duration
        };
        Some(overall_progress + self.iteration_start)
    }

    /// <https://drafts.csswg.org/web-animations-1/#simple-iteration-progress>
    pub fn simple_iteration_progress(
        &self,
        overall_progress: Option<f64>,
        active_time: Option<f64>,
        phase: Phase,
    ) -> Option<f64> {
        let overall_progress = overall_progress?;
        let simple_iteration_progress = if overall_progress.is_infinite() {
            self.iteration_start % 1.
        } else {
            overall_progress % 1.
        };
        if simple_iteration_progress == 0.
            && matches!(phase, Phase::Active | Phase::After)
            && active_time == Some(self.active_duration())
            && self.iterations != 0.
        {
            return Some(1.);
        }
        Some(simple_iteration_progress)
    }

    /// <https://drafts.csswg.org/web-animations-1/#current-iteration>
    ///
    /// The result may be infinite.
    pub fn current_iteration(
        &self,
        overall_progress: Option<f64>,
        simple_iteration_progress: Option<f64>,
        phase: Phase,
    ) -> Option<f64> {
        let overall_progress = overall_progress?;
        if phase == Phase::After && self.iterations.is_infinite() {
            return Some(f64::INFINITY);
        }
        if simple_iteration_progress == Some(1.) {
            return Some(overall_progress.floor() - 1.);
        }
        Some(overall_progress.floor())
    }

    /// The direction in which the iteration `current_iteration` is played.
    ///
    /// <https://drafts.csswg.org/web-animations-1/#directed-progress>
    pub fn current_direction(&self, current_iteration: f64) -> CurrentDirection {
        let mut d = match self.direction {
            AnimationDirection::Normal => return CurrentDirection::Forwards,
            AnimationDirection::Reverse => return CurrentDirection::Reverse,
            AnimationDirection::Alternate => current_iteration,
            AnimationDirection::AlternateReverse => current_iteration + 1.,
        };
        if d.is_infinite() {
            return CurrentDirection::Forwards;
        }
        d %= 2.;
        if d == 0. {
            CurrentDirection::Forwards
        } else {
            CurrentDirection::Reverse
        }
    }
}

/// <https://drafts.csswg.org/web-animations-1/#directed-progress>
pub fn directed_progress(
    simple_iteration_progress: Option<f64>,
    current_direction: CurrentDirection,
) -> Option<f64> {
    let simple_iteration_progress = simple_iteration_progress?;
    Some(match current_direction {
        CurrentDirection::Forwards => simple_iteration_progress,
        CurrentDirection::Reverse => 1. - simple_iteration_progress,
    })
}

/// The before flag passed to the easing function.
///
/// <https://drafts.csswg.org/web-animations-1/#transformed-progress>
pub fn before_flag(phase: Phase, current_direction: CurrentDirection) -> BeforeFlag {
    match (phase, current_direction) {
        (Phase::Before, CurrentDirection::Forwards) | (Phase::After, CurrentDirection::Reverse) => {
            BeforeFlag::Set
        },
        _ => BeforeFlag::Unset,
    }
}

/// <https://drafts.csswg.org/web-animations-1/#transformed-progress>
pub fn transformed_progress(
    directed_progress: Option<f64>,
    before_flag: BeforeFlag,
    easing: &TimingFunction,
) -> Option<f64> {
    /// The precision Gecko uses when evaluating easing functions.
    const EPSILON: f64 = 1e-7;
    Some(easing.calculate_output(directed_progress?, before_flag, EPSILON))
}

/// The results of the timing calculations for an effect at a given local time.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedTiming {
    /// <https://drafts.csswg.org/web-animations-1/#animation-effect-phases-and-states>
    pub phase: Phase,
    /// <https://drafts.csswg.org/web-animations-1/#active-time>
    pub active_time: Option<f64>,
    /// <https://drafts.csswg.org/web-animations-1/#current-iteration>
    pub current_iteration: Option<f64>,
    /// The direction of the current iteration, or `Forwards` if there is none.
    pub current_direction: CurrentDirection,
    /// <https://drafts.csswg.org/web-animations-1/#transformed-progress>
    pub progress: Option<f64>,
}

impl ComputedTiming {
    /// Runs the timing calculations for an effect with the given `timing` and `easing`, whose
    /// animation has the given `playback_rate`, at `local_time`.
    pub fn new(
        timing: &EffectTiming,
        easing: &TimingFunction,
        local_time: Option<f64>,
        playback_rate: f64,
    ) -> Self {
        let phase = timing.phase(local_time, playback_rate);
        let active_time = timing.active_time(local_time, phase);
        let overall_progress = timing.overall_progress(active_time, phase);
        let simple_iteration_progress =
            timing.simple_iteration_progress(overall_progress, active_time, phase);
        let current_iteration =
            timing.current_iteration(overall_progress, simple_iteration_progress, phase);
        let current_direction = current_iteration.map_or(CurrentDirection::Forwards, |iteration| {
            timing.current_direction(iteration)
        });
        let directed_progress = directed_progress(simple_iteration_progress, current_direction);
        let progress = transformed_progress(
            directed_progress,
            before_flag(phase, current_direction),
            easing,
        );
        Self {
            phase,
            active_time,
            current_iteration,
            current_direction,
            progress,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::generics::easing::{StepPosition, TimingKeyword};

    const INF: f64 = f64::INFINITY;
    const LINEAR: TimingFunction = TimingFunction::Keyword(TimingKeyword::Linear);

    fn compute(timing: &EffectTiming, local_time: f64, playback_rate: f64) -> ComputedTiming {
        ComputedTiming::new(timing, &LINEAR, Some(local_time), playback_rate)
    }

    fn assert_phases(timing: EffectTiming, playback_rate: f64, expected: &[(f64, Phase)]) {
        for &(local_time, phase) in expected {
            assert_eq!(
                timing.phase(Some(local_time), playback_rate),
                phase,
                "{timing:?} at {local_time}"
            );
        }
    }

    // web-animations/timing-model/animation-effects/phases-and-states.html
    #[test]
    fn phases() {
        use Phase::*;
        let timing = |duration, delay, end_delay| EffectTiming {
            duration,
            delay,
            end_delay,
            ..Default::default()
        };
        assert_eq!(timing(1., 0., 0.).phase(None, 1.), Idle);
        assert_phases(
            timing(1., 0., 0.),
            1.,
            &[(-1., Before), (0., Active), (1., After)],
        );
        assert_phases(
            timing(1., 1., 0.),
            1.,
            &[(0., Before), (1., Active), (2., After)],
        );
        assert_phases(
            timing(1., -1., 0.),
            1.,
            &[(-2., Before), (-1., Before), (0., After)],
        );
        assert_phases(
            timing(1., 0., 1.),
            1.,
            &[(-1., Before), (0., Active), (1., After), (2., After)],
        );
        assert_phases(
            timing(2., 0., -1.),
            1.,
            &[(-1., Before), (0., Active), (0.9, Active), (1., After)],
        );
        assert_phases(
            timing(1., 0., -1.),
            1.,
            &[(-1., Before), (0., After), (1., After)],
        );
        assert_phases(
            timing(1., 0., -2.),
            1.,
            &[(-2., Before), (-1., Before), (0., After)],
        );
        assert_phases(
            timing(2., 1., -1.),
            1.,
            &[(0., Before), (1., Active), (2., After)],
        );
        assert_phases(
            timing(1., -1., -1.),
            1.,
            &[(-2., Before), (-1., Before), (0., After)],
        );
        assert_phases(
            timing(1., -1., -2.),
            1.,
            &[(-3., Before), (-2., Before), (-1., Before), (0., After)],
        );
        assert_phases(
            timing(1., 0., 0.),
            -1.,
            &[(-1., Before), (0., Before), (1., Active), (2., After)],
        );
    }

    // web-animations/timing-model/animation-effects/active-time.html
    #[test]
    fn active_time() {
        let active_time = |fill, local_time| {
            let timing = EffectTiming {
                delay: 1.,
                duration: 100.,
                iterations: 2.,
                fill,
                ..Default::default()
            };
            compute(&timing, local_time, 1.).active_time
        };
        use AnimationFillMode::*;
        assert_eq!(active_time(None, 0.), Option::None);
        assert_eq!(active_time(Forwards, 0.), Option::None);
        assert_eq!(active_time(Backwards, 0.), Some(0.));
        assert_eq!(active_time(Both, 0.), Some(0.));
        assert_eq!(active_time(None, 51.), Some(50.));
        assert_eq!(active_time(None, 300.), Option::None);
        assert_eq!(active_time(Backwards, 300.), Option::None);
        assert_eq!(active_time(Forwards, 300.), Some(200.));
        assert_eq!(active_time(Both, 300.), Some(200.));

        assert_eq!(
            ComputedTiming::new(&EffectTiming::default(), &LINEAR, Option::None, 1.).active_time,
            Option::None
        );

        // The current iteration and progress once the animation has finished.
        let finished = |duration, iterations, delay, end_delay| {
            let timing = EffectTiming {
                delay,
                end_delay,
                duration,
                iterations,
                fill: Forwards,
                ..Default::default()
            };
            let computed = compute(&timing, timing.end_time(), 1.);
            (
                computed.current_iteration.unwrap(),
                (computed.progress.unwrap() * 1000.).round() / 1000.,
            )
        };
        assert_eq!(finished(1000., 2.3, 500., 0.), (2., 0.3));
        assert_eq!(finished(0., INF, 0., 0.), (INF, 1.));
        assert_eq!(finished(1000., 2.3, 500., 4000.), (2., 0.3));
        assert_eq!(finished(1000., 2.3, 500., -800.), (1., 0.5));
        assert_eq!(finished(1000., 2.3, 500., -2500.), (0., 0.));
        assert_eq!(finished(1000., 2.3, 500., -4000.), (0., 0.));
    }

    /// Checks the progress and current iteration of an effect with a delay of 1 and
    /// `fill: both` in the before phase, at the start of the active phase and in the after
    /// phase, as `assert_computed_timing_for_each_phase` does in WPT.
    ///
    /// Each expectation is `(progress, current iteration)`. `None` means that the phase is
    /// never reached.
    #[track_caller]
    fn assert_each_phase(
        (iterations, iteration_start, duration): (f64, f64, f64),
        before: (f64, f64),
        active: Option<(f64, f64)>,
        after: Option<(f64, f64)>,
    ) {
        let timing = EffectTiming {
            delay: 1.,
            fill: AnimationFillMode::Both,
            iteration_start,
            iterations,
            duration,
            ..Default::default()
        };
        let result = |computed: ComputedTiming| {
            (
                computed.progress.unwrap(),
                computed.current_iteration.unwrap(),
            )
        };

        let computed = compute(&timing, 0., 1.);
        assert_eq!(computed.phase, Phase::Before);
        assert_eq!(result(computed), before, "before phase");

        let computed = compute(&timing, 1., 1.);
        match active {
            Some(active) => {
                assert_eq!(computed.phase, Phase::Active);
                assert_eq!(result(computed), active, "active phase");
            },
            Option::None => assert_eq!(computed.phase, Phase::After),
        }

        let end_time = timing.end_time();
        match after {
            Some(after) => {
                let computed = compute(&timing, end_time + 1., 1.);
                assert_eq!(computed.phase, Phase::After);
                assert_eq!(result(computed), after, "after phase");
            },
            Option::None => assert!(end_time.is_infinite()),
        }
    }

    // web-animations/timing-model/animation-effects/simple-iteration-progress.html
    // web-animations/timing-model/animation-effects/current-iteration.html
    #[test]
    fn progress_and_current_iteration() {
        // Zero iterations.
        assert_each_phase((0., 0., 0.), (0., 0.), None, Some((0., 0.)));
        assert_each_phase((0., 0., 100.), (0., 0.), None, Some((0., 0.)));
        assert_each_phase((0., 0., INF), (0., 0.), None, Some((0., 0.)));
        assert_each_phase((0., 2.5, 0.), (0.5, 2.), None, Some((0.5, 2.)));
        assert_each_phase((0., 2.5, 100.), (0.5, 2.), None, Some((0.5, 2.)));
        assert_each_phase((0., 2.5, INF), (0.5, 2.), None, Some((0.5, 2.)));
        assert_each_phase((0., 3., 0.), (0., 3.), None, Some((0., 3.)));
        assert_each_phase((0., 3., 100.), (0., 3.), None, Some((0., 3.)));
        assert_each_phase((0., 3., INF), (0., 3.), None, Some((0., 3.)));

        // Integer iterations.
        assert_each_phase((3., 0., 0.), (0., 0.), None, Some((1., 2.)));
        assert_each_phase((3., 0., 100.), (0., 0.), Some((0., 0.)), Some((1., 2.)));
        assert_each_phase((3., 0., INF), (0., 0.), Some((0., 0.)), None);
        assert_each_phase((3., 2.5, 0.), (0.5, 2.), None, Some((0.5, 5.)));
        assert_each_phase((3., 2.5, 100.), (0.5, 2.), Some((0.5, 2.)), Some((0.5, 5.)));
        assert_each_phase((3., 2.5, INF), (0.5, 2.), Some((0.5, 2.)), None);
        assert_each_phase((3., 3., 0.), (0., 3.), None, Some((1., 5.)));
        assert_each_phase((3., 3., 100.), (0., 3.), Some((0., 3.)), Some((1., 5.)));
        assert_each_phase((3., 3., INF), (0., 3.), Some((0., 3.)), None);

        // Fractional iterations.
        assert_each_phase((3.5, 0., 0.), (0., 0.), None, Some((0.5, 3.)));
        assert_each_phase((3.5, 0., 100.), (0., 0.), Some((0., 0.)), Some((0.5, 3.)));
        assert_each_phase((3.5, 0., INF), (0., 0.), Some((0., 0.)), None);
        assert_each_phase((3.5, 2.5, 0.), (0.5, 2.), None, Some((1., 5.)));
        assert_each_phase((3.5, 2.5, 100.), (0.5, 2.), Some((0.5, 2.)), Some((1., 5.)));
        assert_each_phase((3.5, 2.5, INF), (0.5, 2.), Some((0.5, 2.)), None);
        assert_each_phase((3.5, 3., 0.), (0., 3.), None, Some((0.5, 6.)));
        assert_each_phase((3.5, 3., 100.), (0., 3.), Some((0., 3.)), Some((0.5, 6.)));
        assert_each_phase((3.5, 3., INF), (0., 3.), Some((0., 3.)), None);

        // Infinite iterations.
        assert_each_phase((INF, 0., 0.), (0., 0.), None, Some((1., INF)));
        assert_each_phase((INF, 0., 100.), (0., 0.), Some((0., 0.)), None);
        assert_each_phase((INF, 0., INF), (0., 0.), Some((0., 0.)), None);
        assert_each_phase((INF, 2.5, 0.), (0.5, 2.), None, Some((0.5, INF)));
        assert_each_phase((INF, 2.5, 100.), (0.5, 2.), Some((0.5, 2.)), None);
        assert_each_phase((INF, 2.5, INF), (0.5, 2.), Some((0.5, 2.)), None);
        assert_each_phase((INF, 3., 0.), (0., 3.), None, Some((1., INF)));
        assert_each_phase((INF, 3., 100.), (0., 3.), Some((0., 3.)), None);
        assert_each_phase((INF, 3., INF), (0., 3.), Some((0., 3.)), None);
    }

    // The negative playback rate cases of the same two files.
    #[test]
    fn progress_and_current_iteration_playing_backwards() {
        let at = |duration, iterations, local_time| {
            let timing = EffectTiming {
                delay: 1.,
                fill: AnimationFillMode::Both,
                iterations,
                duration,
                ..Default::default()
            };
            let computed = compute(&timing, local_time, -1.);
            (
                computed.phase,
                computed.progress.unwrap(),
                computed.current_iteration.unwrap(),
            )
        };
        use Phase::*;
        assert_eq!(at(1., 1., 1.), (Before, 0., 0.));
        assert_eq!(at(1., 1., 2.), (Active, 1., 0.));
        assert_eq!(at(1., 1., 3.), (After, 1., 0.));
        assert_eq!(at(1., 2., 1.), (Before, 0., 0.));
        assert_eq!(at(1., 2., 3.), (Active, 1., 1.));
        assert_eq!(at(1., 2., 4.), (After, 1., 1.));
        assert_eq!(at(0., 1., 1.), (Before, 0., 0.));
        assert_eq!(at(0., 1., 2.), (After, 1., 0.));
        assert_eq!(at(0., 0., 1.), (Before, 0., 0.));
        assert_eq!(at(0., 0., 2.), (After, 0., 0.));
    }

    #[test]
    fn unresolved_without_fill() {
        let timing = EffectTiming {
            delay: 1.,
            duration: 100.,
            ..Default::default()
        };
        for local_time in [0., 200.] {
            let computed = compute(&timing, local_time, 1.);
            assert_eq!(computed.progress, None);
            assert_eq!(computed.current_iteration, None);
        }
    }

    #[test]
    fn direction() {
        use AnimationDirection::*;
        let progress = |direction, local_time| {
            let timing = EffectTiming {
                duration: 100.,
                iterations: 3.,
                direction,
                fill: AnimationFillMode::Both,
                ..Default::default()
            };
            compute(&timing, local_time, 1.).progress.unwrap()
        };
        // First, second and third iteration, then the after phase.
        let at = |direction| [25., 125., 225., 300.].map(|time| progress(direction, time));
        assert_eq!(at(Normal), [0.25, 0.25, 0.25, 1.]);
        assert_eq!(at(Reverse), [0.75, 0.75, 0.75, 0.]);
        assert_eq!(at(Alternate), [0.25, 0.75, 0.25, 1.]);
        assert_eq!(at(AlternateReverse), [0.75, 0.25, 0.75, 0.]);
    }

    #[test]
    fn infinite_current_iteration_plays_forwards() {
        let timing = EffectTiming {
            iterations: INF,
            direction: AnimationDirection::AlternateReverse,
            ..Default::default()
        };
        assert_eq!(timing.current_direction(INF), CurrentDirection::Forwards);
    }

    #[test]
    fn step_easing_uses_before_flag() {
        let easing = TimingFunction::Steps(2, StepPosition::End);
        let progress = |direction, local_time| {
            let timing = EffectTiming {
                delay: 10.,
                duration: 100.,
                direction,
                fill: AnimationFillMode::Both,
                ..Default::default()
            };
            ComputedTiming::new(&timing, &easing, Some(local_time), 1.)
                .progress
                .unwrap()
        };
        // The directed progress is on a step boundary in every case.
        assert_eq!(progress(AnimationDirection::Normal, 0.), 0.);
        assert_eq!(progress(AnimationDirection::Normal, 10.), 0.);
        assert_eq!(progress(AnimationDirection::Normal, 60.), 0.5);
        assert_eq!(progress(AnimationDirection::Normal, 200.), 1.);
        assert_eq!(progress(AnimationDirection::Reverse, 0.), 1.);
        assert_eq!(progress(AnimationDirection::Reverse, 200.), 0.);

        let easing = TimingFunction::Steps(2, StepPosition::Start);
        let timing = EffectTiming {
            delay: 10.,
            duration: 100.,
            fill: AnimationFillMode::Both,
            ..Default::default()
        };
        let progress = |local_time| {
            ComputedTiming::new(&timing, &easing, Some(local_time), 1.)
                .progress
                .unwrap()
        };
        assert_eq!(progress(0.), 0.);
        assert_eq!(progress(10.), 0.5);
    }
}
