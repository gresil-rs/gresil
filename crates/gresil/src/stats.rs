// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! Network-level statistics — computed after each
//! [`step`](crate::network::SpikingNeuralNetwork::step).
//!
//! Split from `network.rs` at R4(ii) (2026-08-20): the orchestrator file
//! held three responsibilities; this module owns the statistics surface.
//! Re-exported through `network` — the published path
//! `gresil::network::NetworkStats` is unchanged.

/// Network-level statistics. Computed after each step. No serde (kept minimal).
///
/// The network builds and writes it, and a caller reads it
/// ([`SpikingNeuralNetwork::stats`](crate::network::SpikingNeuralNetwork::stats)).
/// Not exhaustive: a field may come in a minor release. The neuron and
/// synapse counts are the network's own, `neuron_count()` and
/// `synapse_count()`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct NetworkStats {
    /// Cumulative spikes emitted since construction or last `reset`.
    pub total_spikes: u64,
    /// Unstable: behind `unstable-stdp`, as the three `stdp_pairs_*`
    /// fields are. Cumulative plasticity updates applied since construction
    /// or last `reset`: a network learns only once
    /// `set_plasticity_enabled(true)` was called.
    #[cfg(feature = "unstable-stdp")]
    pub plasticity_events: u64,
    /// Unstable: behind `unstable-stdp`.
    /// STDP pairing histogram (session F instrumentation): in-window
    /// pairings by category — the Hebbian-attribution evidence.
    /// `same_step` = a9a2679 co-fire tie-break (dt = +1, LTD branch),
    /// counted since construction or last `reset`.
    /// Out-of-window pairings contribute delta 0 and are counted in none
    /// of the three `stdp_pairs_*` fields this histogram splits over.
    #[cfg(feature = "unstable-stdp")]
    pub stdp_pairs_same_step: u64,
    /// Unstable: behind `unstable-stdp`.
    /// `post_leads` = post-before-pre within the window (LTD branch),
    /// counted since construction or last `reset`.
    #[cfg(feature = "unstable-stdp")]
    pub stdp_pairs_post_leads: u64,
    /// Unstable: behind `unstable-stdp`.
    /// `pre_leads` = pre-before-post within the window (LTP branch — the
    /// Hebbian channel live transmission opened in session F), counted
    /// since construction or last `reset`.
    #[cfg(feature = "unstable-stdp")]
    pub stdp_pairs_pre_leads: u64,
    /// Mean membrane potential across all neurons (mV). Computed post-step;
    /// reads `-70.0` until the first step after construction or `reset`.
    pub avg_membrane_potential_mv: f64,
    /// Mean firing rate per neuron (Hz). Computed post-step; reads `0.0`
    /// until the first step after construction or `reset`.
    pub firing_rate_hz: f64,
}

impl NetworkStats {
    pub(crate) fn new() -> Self {
        Self {
            total_spikes: 0,
            #[cfg(feature = "unstable-stdp")]
            plasticity_events: 0,
            #[cfg(feature = "unstable-stdp")]
            stdp_pairs_same_step: 0,
            #[cfg(feature = "unstable-stdp")]
            stdp_pairs_post_leads: 0,
            #[cfg(feature = "unstable-stdp")]
            stdp_pairs_pre_leads: 0,
            avg_membrane_potential_mv: -70.0,
            firing_rate_hz: 0.0,
        }
    }
}
