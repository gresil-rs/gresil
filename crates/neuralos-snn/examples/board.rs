//! A frozen graph stepped without std: the board half of a program of
//! the starter set, whose host half is `freeze`.
//!
//! `board/graph.rs` is `freeze`'s output: the library's snnTorch fixture
//! as the arrays a `FixedNetwork` steps, and its drive as runs of equal
//! steps. A library, not a binary: on a board the entry point, the panic
//! handler and the linker script are the firmware's (the ESP32-C3's
//! stranger slot, `firmware/esp32c3/`, steps a module of this form). CI
//! checks it for the C3's target, `riscv32imc-unknown-none-elf`. Its test
//! runs on the host: 10 and 5 spikes in 100 steps, the count of the
//! pinned trace the std network writes, over its first 100 steps
//! (`tests/traces/snntorch-two-layer.trace`).
//!
//! Test: `cargo test -p neuralos-snn --example board`
#![cfg_attr(not(test), no_std)]

// The module also carries the step count, the flag and the trace header a
// replay reads; this program steps the arrays alone.
#[allow(dead_code)]
mod graph {
    include!("board/graph.rs");
}

use graph::stranger::{DRIVE, DT_US, N, NEURONS, SYNAPSES};
use neuralos_snn::FixedNetwork;

/// Steps the frozen drive and counts each neuron's spikes.
pub fn run() -> [u32; N] {
    let mut net = FixedNetwork::new(NEURONS, SYNAPSES, DT_US);
    let mut fired = [false; N];
    let mut spikes = [0u32; N];
    for (count, currents) in DRIVE {
        for _ in 0..*count {
            net.step(currents, &mut fired);
            for (n, &f) in spikes.iter_mut().zip(&fired) {
                *n += u32::from(f);
            }
        }
    }
    spikes
}

#[cfg(test)]
mod tests {
    /// The pinned trace's count over its first 100 steps.
    #[test]
    fn the_board_counts_what_the_pinned_trace_counts() {
        assert_eq!(super::run(), [10, 5]);
    }
}
