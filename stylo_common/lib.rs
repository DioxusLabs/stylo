/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Implementation support code shared by the Stylo CSS engine crates:
//! parser context, shared locks, error reporting, use counters, and the
//! bindings-dependent types that can't live in `stylo_traits`.

#![deny(missing_docs)]

extern crate self as style_common;

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
#[macro_use]
extern crate malloc_size_of_derive;
#[allow(unused_imports)]
#[macro_use]
extern crate to_shmem_derive;

#[allow(unused_imports)]
mod derives {
    pub(crate) use derive_more::Deref;
    pub(crate) use malloc_size_of_derive::MallocSizeOf;
    pub(crate) use to_shmem_derive::ToShmem;
}

pub mod atom_types;
pub mod attr_taint;
pub mod element_context;
pub mod error_reporting;
#[cfg(feature = "gecko")]
#[allow(unsafe_code)]
pub mod gecko_bindings;
#[cfg(feature = "gecko")]
#[macro_use]
pub mod gecko_string_cache;
pub mod namespaces;
pub mod parser;
pub mod shared_lock;
pub mod thread_state;
pub mod url_extra_data;
pub mod use_counters;
