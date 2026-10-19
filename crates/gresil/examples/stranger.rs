//! A stranger's graph, stepped on the host: the std program of the
//! starter set (`freeze` writes the same graph for a board, and `board`
//! steps it there).
//!
//! The graph is what `gresil-nir2json --sim-units` writes from
//! snnTorch 1.0's own `.nir` export of `Linear → Leaky → Linear → Leaky`
//! (the library's fixture), read at run time as a stranger would read
//! theirs. Its units mark says simulation units, so it imports with
//! `NirImportOptions::sim_units`, at the step it was written at, 0.1 ms;
//! native options are refused, and the error's message names the ones to
//! use. The drive is the trace case `snntorch-two-layer`'s: the input at
//! 1.0 on steps 0–9 and 50–59, else 0, in thousandths, through the graph's
//! own encoder. Over 100 steps the two neurons spike 10 and 5 times, at
//! the steps of the pinned trace's first 100 rows
//! (`tests/traces/snntorch-two-layer.trace`); the test below reads each
//! step. On an error, `main` prints the path and the error's message.
//!
//! Run: `cargo run -p gresil --example stranger [graph.json]`

use std::process::ExitCode;

use gresil::nir::Thousandths;
use gresil::{NirImport, NirImportOptions};

/// The graph read when no path is given: the library's fixture.
const GRAPH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/nir_fixtures/snntorch_two_layer_sim.json"
);

fn main() -> ExitCode {
    let path = std::env::args().nth(1).unwrap_or(GRAPH.into());
    // `?` out of `main` would print the error's `Debug`; its `Display` is
    // the message, with the remedy where the library has one
    match run(&path) {
        Ok(trains) => {
            let counts: Vec<usize> = trains.iter().map(Vec::len).collect();
            println!("spikes per neuron over 100 steps: {counts:?}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{path}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Reads the graph at `path`, imports it, assembles it and steps it 100
/// steps under the drive: each neuron's spike steps.
fn run(path: &str) -> Result<Vec<Vec<u32>>, Box<dyn std::error::Error>> {
    let json = std::fs::read(path)?;
    let graph = NirImport::from_json(&json, NirImportOptions::sim_units(100))?;
    let (mut net, encoder, _report) = graph.build_network()?;

    let mut trains = vec![Vec::new(); usize::from(net.neuron_count())];
    for step in 0..100u32 {
        let input = Thousandths(i32::from(matches!(step, 0..=9 | 50..=59)) * 1_000);
        let currents = encoder.encode(&[&[input]]);
        for spike in net.step(&currents)? {
            trains[usize::from(spike.neuron_id)].push(step);
        }
    }
    Ok(trains)
}

#[cfg(test)]
mod tests {
    /// The pinned trace's spiking steps, its rows 0 to 99: a count alone
    /// misses a spike that moves a step.
    #[test]
    fn it_spikes_where_the_pinned_trace_spikes() {
        let trains = super::run(super::GRAPH).expect("the fixture imports and steps");
        assert_eq!(
            trains,
            [
                vec![1, 3, 5, 7, 9, 51, 53, 55, 57, 59],
                vec![4, 8, 52, 56, 60]
            ]
        );
    }
}
