// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! The kept emission (the frozen-format sitting's Call 1 (i), part 2): a
//! frozen file written by any 0.1.x gives the same spikes on every later
//! 0.1.y. `tests/kept/0.1.0/` keeps 0.1.0's emission, byte copies of
//! `tests/traces/frozen.rs` and of the 14 plasticity-off traces it
//! replays, and this file replays them at every commit. Part of the test
//! gate (`cargo test --workspace`); alone, `cargo test -p gresil --test
//! kept`.
//!
//! The folder's rules. No writer regenerates it, and within 0.1.x a kept
//! file is never re-copied. A release whose emission changed adds its own
//! `kept/<version>/` beside it. The copies' own header lines describe the
//! live files they were copied from, not the copies. A raise of
//! `STEP_REVISION` makes the kept modules refuse to compile: the loud
//! break (Call 2).
//!
//! It reads the kept folder and the library alone, never `tests/traces/`
//! or its `cases.rs`, so no move of the live files reaches it. Three
//! tests: every kept network, stepped from its arrays, writes its kept
//! trace's text through the library's row writer (`tests/traces.rs`'s
//! `every_frozen_network_writes_its_trace`, on the copies); the step
//! revision is pinned beside one FNV-1a 64 of the 15 files, so re-copying
//! them without raising the revision is red (Call 2); and the folder
//! holds the 15 files and nothing else.

#[rustfmt::skip]
#[path = "kept/0.1.0/frozen.rs"]
mod v0_1_0;

use std::path::Path;

use gresil::trace::row;
use gresil::FixedNetwork;

/// The kept folder of 0.1.0, resolved from the crate root.
const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/kept/0.1.0");

/// Call 2's pin: the step revision the kept files were written for,
/// beside their hash, [`hash_of`] over the whole folder.
const PIN: (u32, u64) = (1, 0x604d_e5c3_956b_d78d);

/// A kept file's text, by name.
fn read(name: &str) -> String {
    let path = Path::new(DIR).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every name in the kept folder, in name order.
fn names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(DIR)
        .expect("the kept folder")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .into_string()
                .expect("a UTF-8 name")
        })
        .collect();
    names.sort();
    names
}

/// FNV-1a 64 (offset `0xcbf2_9ce4_8422_2325`, prime `0x0100_0000_01b3`,
/// as `wiring_hash` in `network.rs`'s tests) over the kept files `names`,
/// in that order: each file's name, its length as eight little-endian
/// bytes, then its bytes. So bytes moved from one file to the next move
/// it too.
fn hash_of(names: &[String]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for name in names {
        let path = Path::new(DIR).join(name);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let len = u64::try_from(bytes.len())
            .expect("a length fits u64")
            .to_le_bytes();
        for &b in name.as_bytes().iter().chain(&len).chain(&bytes) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The first line where two texts part: its number, from 1, and the two
/// lines, `<end>` past a text's last line. `None` when the texts are
/// equal.
fn first_difference(have: &str, want: &str) -> Option<(usize, String, String)> {
    if have == want {
        return None;
    }
    let (mut a, mut b) = (have.lines(), want.lines());
    for n in 1.. {
        match (a.next(), b.next()) {
            (None, None) => {
                return Some((n, "<end>".into(), "<end>; the line endings differ".into()));
            }
            (x, y) if x != y => {
                return Some((n, x.unwrap_or("<end>").into(), y.unwrap_or("<end>").into()));
            }
            _ => {}
        }
    }
    unreachable!("the texts differ, so some line does")
}

#[test]
fn every_kept_network_writes_its_kept_trace() {
    let mut replayed = 0;
    macro_rules! replay {
        ($case:ident) => {{
            use v0_1_0::$case as c;
            let name = stringify!($case).replace('_', "-");
            let mut net = FixedNetwork::new(c::NEURONS, c::SYNAPSES, c::DT_US);
            let mut fired = [false; c::N];
            let mut text = format!("{}\n", c::HEADER);
            let mut step = 0u32;
            for &(count, input) in c::DRIVE {
                for _ in 0..count {
                    let time_us = net.time_us();
                    net.step(&input, &mut fired);
                    if !c::SPIKES_ONLY || fired.contains(&true) {
                        row(&mut text, step, time_us, &fired, net.neurons())
                            .expect("a String takes every write");
                    }
                    step += 1;
                }
            }
            assert_eq!(step, c::STEPS, "{name}: the drive covers the run");
            if let Some((n, have, want)) = first_difference(&read(&format!("{name}.trace")), &text)
            {
                panic!(
                    "{name}: the kept network parts from its kept trace at line {n}:\n  kept:  \
                     {have}\n  fixed: {want}"
                );
            }
            replayed += 1;
        }};
    }
    v0_1_0::for_each_frozen!(replay);
    assert!(replayed > 0, "for_each_frozen! reached no module");
}

#[test]
fn the_step_revision_is_pinned_beside_the_kept_files() {
    let (revision, hash) = (gresil::STEP_REVISION, hash_of(&names()));
    assert!(
        (revision, hash) == PIN,
        "(STEP_REVISION, FNV-1a 64 of tests/kept/0.1.0/) is ({revision}, {hash:#018x}), pinned \
         ({}, {:#018x}): the kept files moved. Within 0.1.x a kept file is never re-copied; a \
         release whose step moves their spikes raises STEP_REVISION and adds its own \
         kept/<version>/ (this file's module doc)",
        PIN.0,
        PIN.1
    );
}

#[test]
fn the_folder_holds_the_kept_files_and_nothing_else() {
    let mut want = vec!["frozen.rs".to_string()];
    macro_rules! trace_of {
        ($case:ident) => {
            want.push(format!("{}.trace", stringify!($case).replace('_', "-")));
        };
    }
    v0_1_0::for_each_frozen!(trace_of);
    want.sort();
    assert_eq!(want.len(), 15, "frozen.rs and its 14 traces");
    assert_eq!(
        names(),
        want,
        "tests/kept/0.1.0/ holds frozen.rs and one trace per kept module, nothing else"
    );
}
