/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::env;

fn main() {
    let gecko = cfg!(feature = "gecko");
    let servo = cfg!(feature = "servo");
    let engine = match (gecko, servo) {
        (true, false) => "gecko",
        (false, true) => "servo",
        _ => panic!(
            "\n\n\
             The style crate requires enabling one of its 'servo' or 'gecko' feature flags. \
             \n\n"
        ),
    };
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:out_dir={}", env::var("OUT_DIR").unwrap());
    stylo_build::generate_properties(engine);
}
