/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

fn main() {
    let engine = if cfg!(feature = "gecko") {
        "gecko"
    } else {
        "servo"
    };
    println!("cargo:rerun-if-changed=build.rs");
    stylo_build::generate_property_ids(engine);
}
