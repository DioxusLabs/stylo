/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Build-time code generation for the Stylo CSS engine.
//!
//! This crate packages the mako-based property code generation pipeline so
//! that multiple Stylo crates can run it from their build scripts.

use std::env;
use std::path::Path;
use std::process::{exit, Command};
use std::sync::LazyLock;
use walkdir::WalkDir;

/// The python3 executable to use, honoring the `PYTHON3` environment variable.
pub static PYTHON: LazyLock<String> = LazyLock::new(|| {
    env::var("PYTHON3").ok().unwrap_or_else(|| {
        let candidates = if cfg!(windows) {
            ["python.exe"]
        } else {
            ["python3"]
        };
        for &name in &candidates {
            if Command::new(name)
                .arg("--version")
                .output()
                .ok()
                .map_or(false, |out| out.status.success())
            {
                return name.to_owned();
            }
        }
        panic!(
            "Can't find python (tried {})! Try fixing PATH or setting the PYTHON3 env var",
            candidates.join(", ")
        )
    })
});

/// The directory containing the codegen pipeline (this crate's manifest dir).
fn pipeline_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Emit `cargo:rerun-if-changed` for all pipeline inputs.
pub fn emit_rerun_if_changed() {
    for entry in WalkDir::new(pipeline_dir()) {
        let entry = entry.unwrap();
        match entry.path().extension().and_then(|e| e.to_str()) {
            Some("mako") | Some("rs") | Some("py") | Some("zip") | Some("toml") => {
                println!("cargo:rerun-if-changed={}", entry.path().display());
            },
            _ => {},
        }
    }
}

/// Run the property codegen pipeline for the given engine ("servo" or
/// "gecko"), writing the generated files into `OUT_DIR`.
pub fn generate_properties(engine: &str) {
    emit_rerun_if_changed();

    let script = pipeline_dir().join("build.py");

    let status = Command::new(&*PYTHON)
        // `cargo publish` isn't happy with the `__pycache__` files that are created
        // when we run the property generator.
        //
        // TODO(mrobinson): Is this happening because of how we run this script? It
        // would be better to ensure are just placed in the output directory.
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .arg(&script)
        .arg(engine)
        .arg("style-crate")
        .status()
        .unwrap();
    if !status.success() {
        exit(1)
    }
}
