// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! The command line: `gresil-trace-diff [--trace FILE]... LOG`, against
//! the tree's traces, `crates/gresil/tests/traces/`, found from this
//! crate's place in the workspace at build time, as the Python diff found
//! them from its own. So a binary copied out of its tree reads that
//! tree's traces. And cargo runs a build it finds fresh, so from a target
//! dir two trees share, or in a tree moved with its target dir, `cargo
//! run` can run a binary built in another place. It names the place it
//! runs it for in `CARGO_MANIFEST_DIR`; when that is not the place the
//! binary was built in, the binary refuses, exit 1, with one line on
//! stderr that names both and says to touch `src/lib.rs` and run again:
//! the binary depends on the library, so cargo builds both anew. A path
//! through a symlink to the same place is the same place; run by its
//! path, the variable unset, the binary reads the built-in tree. Run
//! from the repository's root: `firmware/esp32c3/` and
//! `proofs/qemu-trace-replay/` are workspaces of their own, without this
//! crate, and from either one cargo reads a `.cargo/config.toml` that
//! builds for its bare-metal target, the chip's or QEMU's. The diff is
//! the library's (`lib.rs`); exit 0 every case identical · 1 a
//! difference, a LOG or a tree trace that cannot be read, or the
//! refusal · 2 usage.

#![forbid(unsafe_code)]

use std::ffi::OsStr;
use std::path::Path;
use std::process::ExitCode;

/// The tree's trace files.
const TRACES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../gresil/tests/traces");

/// Why the binary does not judge for the place `cargo run` ran it for,
/// `ran`, if it does not: `ran` and the place it was built in resolve to
/// two directories, or one of them to none. Nothing when `ran` is unset.
fn refusal(ran: Option<&OsStr>) -> Option<String> {
    let ran = Path::new(ran?);
    let built = Path::new(env!("CARGO_MANIFEST_DIR"));
    if let (Ok(a), Ok(b)) = (built.canonicalize(), ran.canonicalize())
        && a == b
    {
        return None;
    }
    Some(format!(
        "gresil-trace-diff: built in {}, run by cargo for {}: its traces are another tree's; touch {} and run again",
        built.display(),
        ran.display(),
        ran.join("src/lib.rs").display()
    ))
}

fn main() -> ExitCode {
    if let Some(why) = refusal(std::env::var_os("CARGO_MANIFEST_DIR").as_deref()) {
        eprintln!("{why}");
        return ExitCode::from(1);
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let report = gresil_trace_diff::run(&args, Path::new(TRACES));
    print!("{}", report.stdout);
    eprint!("{}", report.stderr);
    ExitCode::from(report.code)
}
