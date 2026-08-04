/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared list types for the generated vector longhands.
//!
//! Each vector longhand (`animation-*`, `background-*`, `box-shadow`, ...)
//! stores its value as a list of single values. These generic types implement
//! the list machinery once, instead of once per longhand in the generated
//! code.

use crate::derives::*;
use crate::values::animated::{lists, Animate, Procedure, ToAnimatedZero};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use crate::OwnedSlice;
use smallvec::SmallVec;

/// A non-empty, comma-separated list of values, as used by vector longhands
/// with a non-empty initial value. Animates as a repeatable list.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, ToAnimatedValue, ToResolvedValue, ToCss, ToTyped,
)]
#[css(comma)]
pub struct NonEmptyCommaList<T>(#[css(iterable)] pub SmallVec<[T; 1]>);

/// A possibly-empty, comma-separated list of values, serialized as `none`
/// when empty. Animates with-zero, like shadow lists.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, ToAnimatedValue, ToResolvedValue, ToCss, ToTyped,
)]
#[css(comma)]
pub struct EmptyCommaList<T>(#[css(if_empty = "none", iterable)] pub OwnedSlice<T>);

/// A possibly-empty, space-separated list of values, serialized as `none`
/// when empty. Animates with-zero, like filter lists.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, ToAnimatedValue, ToResolvedValue, ToCss, ToTyped,
)]
pub struct EmptySpaceList<T>(#[css(if_empty = "none", iterable)] pub OwnedSlice<T>);

macro_rules! list_impls {
    ($ty:ident, $underlying:ty, $animation_type:ident,
     animate($($animate_bound:tt)*), distance($($distance_bound:tt)*)) => {
        impl<T> ToAnimatedZero for $ty<T> {
            fn to_animated_zero(&self) -> Result<Self, ()> {
                Err(())
            }
        }

        impl<T: $($animate_bound)*> Animate for $ty<T> {
            fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
                Ok($ty(lists::$animation_type::animate(
                    &self.0, &other.0, procedure,
                )?))
            }
        }

        impl<T: $($distance_bound)*> ComputeSquaredDistance for $ty<T> {
            fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
                lists::$animation_type::squared_distance(&self.0, &other.0)
            }
        }

        impl<T> From<$ty<T>> for $underlying {
            #[inline]
            fn from(l: $ty<T>) -> Self {
                l.0
            }
        }

        impl<T> From<$underlying> for $ty<T> {
            #[inline]
            fn from(l: $underlying) -> Self {
                $ty(l)
            }
        }
    };
}

list_impls!(
    NonEmptyCommaList,
    SmallVec<[T; 1]>,
    repeatable_list,
    animate(Animate),
    distance(ComputeSquaredDistance)
);
list_impls!(
    EmptyCommaList,
    OwnedSlice<T>,
    with_zero,
    animate(Animate + Clone + ToAnimatedZero),
    distance(ComputeSquaredDistance + ToAnimatedZero)
);
list_impls!(
    EmptySpaceList,
    OwnedSlice<T>,
    with_zero,
    animate(Animate + Clone + ToAnimatedZero),
    distance(ComputeSquaredDistance + ToAnimatedZero)
);
