// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! A `no_std` user with STDP on: a program of the starter set.
//!
//! Two neurons and one synapse, the pairs applied by hand: the network
//! that learns (`SpikingNeuralNetwork`) needs `std`, so a board that
//! learns writes this loop itself. An exercise of the API, not a model:
//! the pulse is the weight as a current, and both neurons are driven.
//! Nor is it the network's rule: here the pulse reaches the post neuron
//! in the pre neuron's step, and that pair counts as dt 0, potentiation;
//! `SpikingNeuralNetwork` delivers a pulse a step later and breaks a
//! same-step tie toward depression.
//! Both are on the centi-mV grid, as the crate README's Quick start is:
//! on the mV grid these currents never fire. Each has 5 pA of noise, as
//! `SpikingNeuralNetwork::new` gives its neurons: without it the post
//! neuron fires only in the pre neuron's step, and the depression pass
//! never moves the weight. STDP is behind `unstable-stdp`, outside the
//! semver promise. CI checks this program for the C3's target,
//! `riscv32imc-unknown-none-elf`. Its test runs on the host: the weight
//! goes from 100 to 529 in 10 s.
//!
//! Test: `cargo test -p gresil --features unstable-stdp --example learner`
#![cfg_attr(not(test), no_std)]

use gresil::{LIFNeuron, NeuronType, STDPRule, Synapse, VoltageResolution};

/// Runs `steps` 1 ms steps and returns the synapse's final weight.
pub fn learn(steps: u32) -> gresil::Result<i16> {
    let rule = STDPRule::new();
    let mut synapse = Synapse::new(0, 1, 100)?;
    let mut pre = neuron(0);
    let mut post = neuron(1);
    let (mut last_pre, mut last_post) = (None::<u32>, None::<u32>);

    for step in 0..steps {
        let now = step.wrapping_mul(1_000);
        pre.decay_adaptation_current();
        post.decay_adaptation_current();
        let pre_fired = pre.integrate_and_fire(160, 1_000, now);
        let pulse = if pre_fired { synapse.weight() } else { 0 };
        let post_fired = post.integrate_and_fire(150 + pulse, 1_000, now);

        // `dt = pre - post`: negative is LTP, positive LTD
        if pre_fired {
            if let Some(t) = last_post {
                synapse.update_weight(rule.calculate_weight_change(dt(now, t)));
            }
            last_pre = Some(now);
        }
        if post_fired {
            if let Some(t) = last_pre {
                synapse.update_weight(rule.calculate_weight_change(dt(t, now)));
            }
            last_post = Some(now);
        }
    }
    Ok(synapse.weight())
}

const fn neuron(id: u16) -> LIFNeuron {
    LIFNeuron::new(
        id,
        NeuronType::Excitatory,
        VoltageResolution::CentiMillivolt,
    )
    .with_noise_amplitude_pa(5)
}

fn dt(pre_us: u32, post_us: u32) -> i32 {
    // as the network takes it: the clock wraps at 71.6 min, and a pair
    // under 35.8 min apart is exact across the wrap
    pre_us.wrapping_sub(post_us) as i32
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_weight_goes_from_100_to_529_in_10_s() {
        let weight = super::learn(10_000).expect("pre 0 and post 1 are two neurons");
        assert_eq!(weight, 529);
    }
}
