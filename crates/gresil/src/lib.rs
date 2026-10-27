// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![cfg_attr(not(feature = "simd"), forbid(unsafe_code))]
#![cfg_attr(feature = "simd", deny(unsafe_code))]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![doc = include_str!("../README.md")]

#[cfg(feature = "unstable-bridge")]
pub mod bridge;
#[cfg(feature = "std")]
mod csr;
pub mod fixed;
#[cfg(feature = "unstable-bridge")]
pub mod kernel;
pub mod lif_neuron;
#[cfg(feature = "std")]
pub mod network;
pub mod nir;
#[cfg(feature = "simd")]
#[allow(unsafe_code)]
pub mod simd;
pub mod spike_recorder;
#[cfg(feature = "std")]
mod stats;
pub mod synapse;
pub mod trace;
pub mod trit;

pub use fixed::{FixedNetwork, FixedSynapse};
pub use lif_neuron::{LIFNeuron, NeuronType, VoltageResolution};
#[cfg(feature = "std")]
pub use network::{NetworkTopology, SpikingNeuralNetwork};
#[cfg(feature = "std")]
pub use nir::NirImport;
pub use nir::NirImportOptions;
#[cfg(feature = "unstable-stdp")]
pub use synapse::STDPRule;
pub use synapse::Synapse;

/// The revision of the step: the code that turns a frozen file's
/// neurons, synapses and drive into spikes, [`FixedNetwork::step`] and
/// the [`LIFNeuron::integrate_and_fire`] it calls. A release that moves
/// a spike of a frozen file raises it.
///
/// Through 0.1.x it stays 1: a frozen file written by any 0.1.x gives
/// the same spikes on every later 0.1.y, and `tests/kept.rs` replays
/// the files written at 0.1.0, kept as they were, at every commit.
///
/// Every module the freezer writes (`fixed::freeze::module`, behind
/// `unstable-freeze`) asserts, after its `use` line, the revision it was
/// written for. After a raise, an old file refuses to compile instead of
/// giving other spikes:
///
/// ```
/// const _: () = assert!(gresil::STEP_REVISION == 1);
/// ```
pub const STEP_REVISION: u32 = 1;

/// Crate-level error type.
///
/// The network's fallible functions, `Synapse::new`, `FixedNetwork::try_from`
/// and `trace::header` return [`Result<T>`](Result) via this type; `nir`'s
/// return [`NirError`](crate::nir::NirError), the bridge's and the kernel's
/// `BridgeError`, and `trace::row` a `fmt::Result`. Infallible accessors
/// return plain values. No `unwrap()` outside tests (v0.1 lesson); the
/// `expect`s outside them, in `nir`'s graph assembly and in
/// `freeze::module`, each follow the check they name, and a `NirImport`
/// edited after it was built can still reach the assembly's
/// (`NirImport::build_network` § Panics).
///
/// One variant per rule: a refusal returns the variant of the rule it
/// breaks, and a function's `# Errors` names the variants it returns. Not
/// exhaustive: a new rule may add a variant in a minor release. The
/// variants' order and their integer values (an `as` cast) are not
/// promised. A call that breaks two rules returns the variant of one of
/// them, and which one is not promised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A synapse from a neuron to itself ([`Synapse::new`], and
    /// `SpikingNeuralNetwork::add_synapse` through it).
    SelfConnection,
    /// A network of no neuron, or of more than 65,535: ids are `u16`
    /// (`SpikingNeuralNetwork::new` and `from_neurons`).
    NeuronCountOutOfRange,
    /// A time step of zero (`SpikingNeuralNetwork::new` and
    /// `from_neurons`).
    ZeroTimeStep,
    /// A neuron whose membrane cannot move at the network's time step: a
    /// membrane time constant of 0 or above 1,000 time steps, where
    /// `dt_over_tau` truncates to 0 (`SpikingNeuralNetwork::new` and
    /// `from_neurons`).
    FrozenMembrane,
    /// A neuron id not below the network's neuron count
    /// (`SpikingNeuralNetwork::add_synapse`).
    NeuronIdOutOfRange,
    /// A synaptic input divisor of 0 or above 32,767
    /// (`SpikingNeuralNetwork::set_synaptic_input_divisor`): the step
    /// divides by it as an `i16`.
    DivisorOutOfRange,
    /// A `Feedforward` layer list with a layer of no neuron, or whose sizes
    /// do not sum to the neuron count
    /// (`SpikingNeuralNetwork::build_topology`).
    BadLayerSizes,
    /// A `Balanced` ratio that leaves the network no excitatory or no
    /// inhibitory neuron (`SpikingNeuralNetwork::build_topology`).
    MissingNeuronType,
    /// A `Random` connectivity or a `SmallWorld` rewiring probability
    /// outside 0 to 1, NaN included
    /// (`SpikingNeuralNetwork::build_topology`).
    ProbabilityOutOfRange,
    /// A network with plasticity enabled, which a [`FixedNetwork`] does not
    /// have (`FixedNetwork::try_from`).
    PlasticityEnabled,
    /// A network whose neuron count is not the [`FixedNetwork`]'s `N`
    /// (`FixedNetwork::try_from`).
    NeuronCountMismatch,
    /// A network whose synapse count is not the [`FixedNetwork`]'s `S`
    /// (`FixedNetwork::try_from`).
    SynapseCountMismatch,
    /// Neurons on more than one voltage grid: a network keeps one
    /// (`SpikingNeuralNetwork::from_neurons`, and `trace::header` as a
    /// guard).
    MixedVoltageGrids,
    /// A trace case name that is empty or holds anything but lowercase
    /// ASCII letters, digits and `-` (`trace::header`).
    BadCaseName,
    /// A neuron that is not at rest: its membrane off its resting
    /// potential, refractory time left, a current or a stamp, none of which
    /// `LIFNeuron::new` and the `with_*` chain build
    /// (`SpikingNeuralNetwork::from_neurons`).
    NeuronNotAtRest,
    /// A neuron whose id is not its position among the neurons given
    /// (`SpikingNeuralNetwork::from_neurons`).
    NeuronIdMismatch,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::SelfConnection => "a synapse from a neuron to itself",
            Self::NeuronCountOutOfRange => "a neuron count outside 1 to 65,535",
            Self::ZeroTimeStep => "a time step of zero",
            Self::FrozenMembrane => "a membrane time constant of 0 or above 1,000 time steps",
            Self::NeuronIdOutOfRange => "a neuron id not below the neuron count",
            Self::DivisorOutOfRange => "a synaptic input divisor outside 1 to 32,767",
            Self::BadLayerSizes => {
                "a layer of no neuron, or layer sizes that do not sum to the neuron count"
            }
            Self::MissingNeuronType => {
                "a balanced network with no excitatory or no inhibitory neuron"
            }
            Self::ProbabilityOutOfRange => "a connectivity or rewiring probability outside 0 to 1",
            Self::PlasticityEnabled => "a network with plasticity enabled",
            Self::NeuronCountMismatch => "a neuron count other than the fixed network's N",
            Self::SynapseCountMismatch => "a synapse count other than the fixed network's S",
            Self::MixedVoltageGrids => "neurons on more than one voltage grid",
            Self::BadCaseName => "a case name that is empty or outside [a-z0-9-]",
            Self::NeuronNotAtRest => "a neuron that is not at rest",
            Self::NeuronIdMismatch => "a neuron id other than its position",
        })
    }
}

impl core::error::Error for Error {}

// `core` and not `std`: the trait moved to `core` in 1.81, below the 1.92
// floor, so the impl holds in a `no_std` build — the target this library
// exists for. The assertion is its pin: without the impl it is `E0277`, in
// every configuration.
const _: fn() = || {
    fn a<T: core::error::Error>() {}
    a::<Error>();
};

// One byte. The assertion is its pin: a variant that grows the type
// fails the build here, in every configuration.
const _: () = assert!(core::mem::size_of::<Error>() == 1);

/// Crate-wide `Result` alias.
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::Error;
    use std::collections::BTreeSet;

    /// Each variant's text: its own, lowercase and with no trailing
    /// punctuation, as the API guidelines ask of an error's `Display`.
    #[test]
    fn each_variant_prints_its_rule() {
        let texts = [
            (Error::SelfConnection, "a synapse from a neuron to itself"),
            (
                Error::NeuronCountOutOfRange,
                "a neuron count outside 1 to 65,535",
            ),
            (Error::ZeroTimeStep, "a time step of zero"),
            (
                Error::FrozenMembrane,
                "a membrane time constant of 0 or above 1,000 time steps",
            ),
            (
                Error::NeuronIdOutOfRange,
                "a neuron id not below the neuron count",
            ),
            (
                Error::DivisorOutOfRange,
                "a synaptic input divisor outside 1 to 32,767",
            ),
            (
                Error::BadLayerSizes,
                "a layer of no neuron, or layer sizes that do not sum to the neuron count",
            ),
            (
                Error::MissingNeuronType,
                "a balanced network with no excitatory or no inhibitory neuron",
            ),
            (
                Error::ProbabilityOutOfRange,
                "a connectivity or rewiring probability outside 0 to 1",
            ),
            (
                Error::PlasticityEnabled,
                "a network with plasticity enabled",
            ),
            (
                Error::NeuronCountMismatch,
                "a neuron count other than the fixed network's N",
            ),
            (
                Error::SynapseCountMismatch,
                "a synapse count other than the fixed network's S",
            ),
            (
                Error::MixedVoltageGrids,
                "neurons on more than one voltage grid",
            ),
            (
                Error::BadCaseName,
                "a case name that is empty or outside [a-z0-9-]",
            ),
            (Error::NeuronNotAtRest, "a neuron that is not at rest"),
            (
                Error::NeuronIdMismatch,
                "a neuron id other than its position",
            ),
        ];
        for (error, text) in texts {
            // No wildcard: a new variant does not compile here until it has
            // its line above.
            match error {
                Error::SelfConnection
                | Error::NeuronCountOutOfRange
                | Error::ZeroTimeStep
                | Error::FrozenMembrane
                | Error::NeuronIdOutOfRange
                | Error::DivisorOutOfRange
                | Error::BadLayerSizes
                | Error::MissingNeuronType
                | Error::ProbabilityOutOfRange
                | Error::PlasticityEnabled
                | Error::NeuronCountMismatch
                | Error::SynapseCountMismatch
                | Error::MixedVoltageGrids
                | Error::BadCaseName
                | Error::NeuronNotAtRest
                | Error::NeuronIdMismatch => assert_eq!(error.to_string(), text),
            }
            assert!(text.starts_with(char::is_lowercase), "{text}");
            assert!(!text.ends_with(['.', '!', '?']), "{text}");
        }
        let distinct: BTreeSet<&str> = texts.iter().map(|&(_, text)| text).collect();
        assert_eq!(distinct.len(), texts.len(), "two variants print alike");
    }
}
