// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! A stranger's graph frozen for a board: the host half of a program of
//! the starter set, whose board half is `board`.
//!
//! Imports the library's snnTorch fixture (`gresil-nir2json
//! --sim-units` output, in simulation units at 0.1 ms), assembles it, and
//! writes the Rust module a `FixedNetwork` steps: the neurons and the
//! synapses as arrays, and the drive as runs of equal steps, the trace
//! case `snntorch-two-layer`'s first 100 steps. The library's freezer
//! writes the module, its trace header included (`fixed::freeze`, behind
//! `unstable-freeze`, outside the semver promise). The module is
//! `examples/board/graph.rs`: `board` includes it, and the test below
//! compares it with what this program writes, so when the library moves
//! that, the test is red until this program writes the file again,
//! committed with the why.
//!
//! Run: `cargo run -p gresil --features unstable-freeze --example freeze`

use gresil::fixed::freeze;
use gresil::nir::Thousandths;
use gresil::trace::{Kind, Rows};
use gresil::{NirImport, NirImportOptions};

/// The graph: `gresil-nir2json --sim-units` output at 0.1 ms.
const GRAPH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/nir_fixtures/snntorch_two_layer_sim.json"
);

/// The module `board` includes.
const OUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/board/graph.rs");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(OUT, module(&std::fs::read(GRAPH)?)?)?;
    println!("wrote {OUT}");
    Ok(())
}

/// The graph as one module, `stranger`: the input at 1.0 on steps 0–9
/// and 50–59, else 0, through the graph's own encoder.
fn module(json: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let graph = NirImport::from_json(json, NirImportOptions::sim_units(100))?;
    let (net, encoder, _report) = graph.build_network()?;

    let on = encoder.encode(&[&[Thousandths(1_000)]]);
    let off = encoder.encode(&[&[Thousandths(0)]]);
    let drive = [(10, on.clone()), (40, off.clone()), (10, on), (40, off)];
    Ok(freeze::module(
        &net,
        "stranger",
        Kind::Stranger,
        100,
        Rows::All,
        &drive,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_board_graph_is_what_this_writes() {
        let json = std::fs::read(super::GRAPH).expect("the fixture");
        let module = super::module(&json).expect("the fixture imports and freezes");
        let file = std::fs::read_to_string(super::OUT).expect("examples/board/graph.rs");
        assert_eq!(
            file, module,
            "examples/board/graph.rs is not what `freeze` writes: run `cargo run -p \
             gresil --features unstable-freeze --example freeze` and commit it with the why"
        );
    }
}
