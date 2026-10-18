//! Synapse model — clean port from v0.1 `libneuralos/src/core/spiking_neural_network/synapse.rs`.
//!
//! # Audit context
//!
//! The audit flagged this as **Gem #3** — *"the better-engineered STDP rule impl"*
//! (vs `stdp_plasticity.rs` which had the LTP timing filter bug). Port this version.
//!
//! # Invariants (testable)
//!
//! - `Synapse::new(id, id, _)` always refuses, with
//!   [`Error::SelfConnection`] (no self-connections).
//!
//! With the `unstable-stdp` feature (`update_weight`, `STDPRule`, a
//! synapse's bounds and its two counters exist in no other build, and may
//! change or go in a minor release):
//!
//! - `Synapse::new` sets the bounds from the weight's sign: `[0, 2000]`
//!   for a weight of 0 or more, `[-2000, 0]` below 0. The fields are the
//!   crate's, so no caller writes the bounds.
//! - Weight stays within `[min_weight, max_weight]` after every `update_weight` call.
//! - `STDPRule::calculate_weight_change(dt)` returns 0 for `|dt| ≥ tau`, where the
//!   linear factor reaches 0.
//! - LTP (pre before post, dt < 0) returns a non-negative delta.
//! - LTD (post before pre, dt > 0) returns a non-positive delta.
//!
//! # Fixed-point conventions
//!
//! - Weight: `i16`, fixed-point 1000 = 1.0 (range ~[-2000, 2000] typically).
//! - Time: `u32` microseconds.
//!
//! # `no_std`
//!
//! Uses `crate::Error` (no `String`, no alloc) for self-connection rejection.
//! Fully `no_std`-compatible — pairs with `lif_neuron`.

#![allow(clippy::module_name_repetitions)]
// Fixed-point design: narrow types (i16 weight, u32 μs) with
// physics-bounded values. Casts between them are intentional and bounded.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use crate::Error;

/// Fixed-point scale: 1000 = 1.0.
///
/// Public since Stage 3: with `unstable-bridge`,
/// `bridge::wire_gamma_to_substrate` maps imported wire-format scales into
/// the substrate through exactly this constant — the coupling has one home
/// and a pinning test, not a magic 1000.
pub const SCALE: i32 = 1000;

/// A synapse: its two ends and its fixed-point weight, and with
/// `unstable-stdp` the bounds STDP clamps the weight to and two
/// instrumentation counters.
///
/// Its fields are the crate's. [`new`](Self::new) builds it, the bounds
/// from the weight's sign; `update_weight`, behind `unstable-stdp`, is
/// its one method that changes it, and a network writes its own
/// synapses' weights, never their bounds, in its STDP and ternary passes;
/// its `reset` sets their two counters to 0.
/// A getter reads each value. A network's synapses reach a caller as a
/// slice (`SpikingNeuralNetwork::synapses`). Pairs with
/// [`LIFNeuron`](crate::LIFNeuron) for full integrate-and-fire + STDP.
#[derive(Debug, Clone)]
pub struct Synapse {
    /// Presynaptic (source) neuron ID.
    pub(crate) pre_neuron_id: u16,
    /// Postsynaptic (target) neuron ID.
    pub(crate) post_neuron_id: u16,
    /// Synaptic weight (fixed-point, 1000 = 1.0). Negative = inhibitory.
    pub(crate) weight: i16,
    /// Maximum weight (plasticity clamp).
    #[cfg(feature = "unstable-stdp")]
    pub(crate) max_weight: i16,
    /// Minimum weight (plasticity clamp).
    #[cfg(feature = "unstable-stdp")]
    pub(crate) min_weight: i16,
    /// Cumulative raw STDP delta (session G instrumentation): see
    /// [`raw_stdp_delta`](Self::raw_stdp_delta).
    #[cfg(feature = "unstable-stdp")]
    pub(crate) raw_stdp_delta: i64,
    /// Cumulative delta the clamp absorbed: see
    /// [`absorbed_delta`](Self::absorbed_delta).
    #[cfg(feature = "unstable-stdp")]
    pub(crate) absorbed_delta: i64,
}

impl Synapse {
    /// New synapse between `pre_id` and `post_id` with initial `weight`.
    ///
    /// With `unstable-stdp`, the bounds come from the weight's sign:
    /// `[0, 2000]` for a weight of 0 or more (excitatory, AMPA),
    /// `[-2000, 0]` below 0 (inhibitory, GABA), and the counters start at
    /// 0. The weight is kept as given, inside its bounds or not.
    ///
    /// ```
    /// use gresil::Synapse;
    ///
    /// let s = Synapse::new(3, 7, -150)?;
    /// assert_eq!((s.pre(), s.post(), s.weight()), (3, 7, -150));
    /// #[cfg(feature = "unstable-stdp")]
    /// assert_eq!((s.min_weight(), s.max_weight()), (-2000, 0));
    /// #[cfg(feature = "unstable-stdp")]
    /// assert_eq!((s.raw_stdp_delta(), s.absorbed_delta()), (0, 0));
    /// assert_eq!(Synapse::new(3, 7, 2_500)?.weight(), 2_500, "kept as given");
    /// # Ok::<(), gresil::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::SelfConnection`] when `pre_id == post_id`.
    pub fn new(pre_id: u16, post_id: u16, weight: i16) -> Result<Self, Error> {
        if pre_id == post_id {
            return Err(Error::SelfConnection);
        }
        #[cfg(feature = "unstable-stdp")]
        let (max_weight, min_weight) = bounds_by_sign(weight);
        Ok(Self {
            pre_neuron_id: pre_id,
            post_neuron_id: post_id,
            weight,
            #[cfg(feature = "unstable-stdp")]
            max_weight,
            #[cfg(feature = "unstable-stdp")]
            min_weight,
            #[cfg(feature = "unstable-stdp")]
            raw_stdp_delta: 0,
            #[cfg(feature = "unstable-stdp")]
            absorbed_delta: 0,
        })
    }

    /// The presynaptic neuron's id: its spike sends the pulse.
    #[must_use]
    pub fn pre(&self) -> u16 {
        self.pre_neuron_id
    }

    /// The postsynaptic neuron's id: its synaptic current takes the pulse.
    #[must_use]
    pub fn post(&self) -> u16 {
        self.post_neuron_id
    }

    /// The weight (fixed-point, 1000 = 1.0); negative is inhibitory.
    #[must_use]
    pub fn weight(&self) -> i16 {
        self.weight
    }

    /// Unstable: behind `unstable-stdp`.
    /// The largest weight `update_weight` leaves: 2000 for an excitatory
    /// synapse, 0 for an inhibitory one.
    #[cfg(feature = "unstable-stdp")]
    #[must_use]
    pub fn max_weight(&self) -> i16 {
        self.max_weight
    }

    /// Unstable: behind `unstable-stdp`.
    /// The smallest weight `update_weight` leaves: 0 for an excitatory
    /// synapse, -2000 for an inhibitory one.
    #[cfg(feature = "unstable-stdp")]
    #[must_use]
    pub fn min_weight(&self) -> i16 {
        self.min_weight
    }

    /// Unstable: behind `unstable-stdp`.
    /// The signed sum of every delta `update_weight` was given since the
    /// synapse was built or its network's last `reset`, before the clamp
    /// (session G instrumentation, the mechanism-label evidence). Read
    /// it per class (intra vs inter assemblies) to decide whether a bucket's
    /// movement was pairing-driven or clamp/flip-machinery-driven.
    #[cfg(feature = "unstable-stdp")]
    #[must_use]
    pub fn raw_stdp_delta(&self) -> i64 {
        self.raw_stdp_delta
    }

    /// Unstable: behind `unstable-stdp`.
    /// The part of those deltas the `[min_weight, max_weight]` clamp
    /// absorbed: `Σ(delta − applied)`. When it is large beside
    /// [`raw_stdp_delta`](Self::raw_stdp_delta), the bounds' asymmetry, not
    /// the pairing, shaped the trajectory.
    #[cfg(feature = "unstable-stdp")]
    #[must_use]
    pub fn absorbed_delta(&self) -> i64 {
        self.absorbed_delta
    }

    /// Unstable: behind `unstable-stdp`.
    /// Apply a plastic weight delta. Clamps to `[min_weight, max_weight]`.
    /// Tracks the raw delta and the clamp-absorbed remainder (session G).
    #[cfg(feature = "unstable-stdp")]
    pub fn update_weight(&mut self, delta_weight: i16) {
        let target = self
            .weight
            .saturating_add(delta_weight)
            .clamp(self.min_weight, self.max_weight);
        let applied = target - self.weight;
        self.raw_stdp_delta += i64::from(delta_weight);
        self.absorbed_delta += i64::from(delta_weight) - i64::from(applied);
        self.weight = target;
    }
}

/// Test-only visualization helper (was pub; zero callers anywhere —
/// the R8 census deferral, ruled in the 2026-08-22 consolidation:
/// relocated under cfg(test) and pinned, not kept as dead pub
/// surface). It reads `max_weight`, which exists with `unstable-stdp`.
#[cfg(all(test, feature = "unstable-stdp"))]
impl Synapse {
    /// Weight as a percentage of `max_weight`.
    fn normalized_weight(&self) -> i16 {
        let abs_max = self.max_weight.unsigned_abs();
        if abs_max > 0 {
            (i32::from(self.weight) * 100 / i32::from(abs_max)) as i16
        } else {
            0
        }
    }
}

/// The bounds (`max_weight`, `min_weight`) a weight's sign gives a
/// synapse: excitatory from 0 up, inhibitory below.
#[cfg(feature = "unstable-stdp")]
fn bounds_by_sign(weight: i16) -> (i16, i16) {
    if weight >= 0 {
        (2000, 0) // AMPA
    } else {
        (0, -2000) // GABA
    }
}

/// Unstable: behind `unstable-stdp`.
/// Spike-Timing-Dependent Plasticity rule.
///
/// The audit flagged this as **the better STDP implementation** (vs `stdp_plasticity.rs`):
/// a linear stand-in for the exponential decay, in fixed point (the factor is
/// `1000 − |dt|·1000/tau`, clamped at 0), sign-correct (positive dt → LTD,
/// negative dt → LTP).
#[cfg(feature = "unstable-stdp")]
#[derive(Debug, Clone)]
pub struct STDPRule {
    /// LTP time constant (μs) — pre-before-post window.
    pub tau_plus_us: u32,
    /// LTD time constant (μs) — post-before-pre window.
    pub tau_minus_us: u32,
    /// LTP amplitude (fixed-point, 1000 = 1.0).
    pub a_plus: i16,
    /// LTD amplitude (negative).
    pub a_minus: i16,
    /// Overall learning rate (fixed-point, 1000 = 1.0).
    pub learning_rate: u16,
}

#[cfg(feature = "unstable-stdp")]
impl STDPRule {
    /// Default biological STDP: 20ms/20ms tau, asymmetric amplitudes (LTD slightly larger).
    #[must_use]
    pub fn new() -> Self {
        Self {
            tau_plus_us: 20_000,
            tau_minus_us: 20_000,
            a_plus: 50,
            a_minus: -53, // Slightly larger |LTD| than LTP — asymmetry is biological
            learning_rate: 100,
        }
    }

    /// Compute the weight delta for a given spike-time difference.
    ///
    /// # Parameters
    ///
    /// - `dt_us = pre_spike_time - post_spike_time` (positive = post fired first → LTD).
    ///
    /// # Returns
    ///
    /// Weight delta (signed): non-negative for LTP, non-positive for LTD, zero outside the window.
    ///
    /// # Invariants
    ///
    /// - `dt > 0` → returns ≤ 0 (LTD)
    /// - `dt < 0` → returns ≥ 0 (LTP)
    /// - `|dt| ≥ tau` → returns 0 (the linear factor reaches 0)
    ///
    /// # Bug fix vs v0.1
    ///
    /// v0.1 used `factor = SCALE - decay` without clamping. When `|dt| > tau`,
    /// `decay > SCALE` and `factor` goes negative, breaking the sign convention
    /// (LTP returns negative, LTD returns positive). The property test
    /// `prop_stdp_sign_convention` catches this at `dt = -24000`. Fix: clamp
    /// `factor` to `≥ 0` — the linear exp approximation naturally floors at zero.
    ///
    /// # Bug fix (R18 rider, 2026-08-22)
    ///
    /// `dt_us.abs() * SCALE` overflowed i32 at `|dt|` > ~2.15 s (release:
    /// silent wrap → spurious deltas far outside the window, e.g. −560 at
    /// dt = ±2 200 000 μs where 0 is required; debug: panic) — and
    /// `dt_us.abs()` itself is UB-adjacent at `i32::MIN`. The whole
    /// expression now runs in i64, cast BEFORE the abs (total at
    /// `i32::MIN`), which also covers the tail product
    /// `a · factor · learning_rate` overflowing i32 at extreme
    /// `learning_rate` (u16 max). Observable values identical everywhere
    /// the old code didn't overflow.
    #[must_use]
    pub fn calculate_weight_change(&self, dt_us: i32) -> i16 {
        if dt_us > 0 {
            // Post fired before pre → LTD (depress the synapse).
            let decay = (i64::from(dt_us).abs() * i64::from(SCALE)) / i64::from(self.tau_minus_us);
            if decay < 10_000 {
                let factor = (i64::from(SCALE) - decay).max(0); // Clamp to ≥ 0
                ((i64::from(self.a_minus) * factor * i64::from(self.learning_rate))
                    / (i64::from(SCALE) * i64::from(SCALE))) as i16
            } else {
                0
            }
        } else {
            // Pre fired before post (or simultaneous) → LTP (potentiate).
            let decay = (i64::from(dt_us).abs() * i64::from(SCALE)) / i64::from(self.tau_plus_us);
            if decay < 10_000 {
                let factor = (i64::from(SCALE) - decay).max(0); // Clamp to ≥ 0
                ((i64::from(self.a_plus) * factor * i64::from(self.learning_rate))
                    / (i64::from(SCALE) * i64::from(SCALE))) as i16
            } else {
                0
            }
        }
    }
}

#[cfg(feature = "unstable-stdp")]
impl Default for STDPRule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::shadow_unrelated)]
    use super::*;
    use proptest::prelude::*;

    // ----- Unit tests -----

    #[test]
    fn new_excitatory_synapse_has_default_params() {
        let s = Synapse::new(3, 7, 150).expect("valid ids");
        assert_eq!(s.pre(), 3);
        assert_eq!(s.post(), 7);
        assert_eq!(s.weight(), 150);
        #[cfg(feature = "unstable-stdp")]
        {
            assert_eq!(s.max_weight(), 2000); // AMPA
            assert_eq!(s.min_weight(), 0);
            assert_eq!((s.raw_stdp_delta(), s.absorbed_delta()), (0, 0));
        }
    }

    #[test]
    fn negative_weight_makes_inhibitory() {
        let s = Synapse::new(3, 7, -150).expect("valid ids");
        assert_eq!((s.pre(), s.post(), s.weight()), (3, 7, -150));
        #[cfg(feature = "unstable-stdp")]
        {
            assert_eq!(s.max_weight(), 0); // GABA
            assert_eq!(s.min_weight(), -2000);
        }
    }

    /// The bounds follow the weight's sign alone, 0 counting as
    /// excitatory, at both ends of `i16` and on each side of 0.
    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn the_bounds_follow_the_sign() {
        for (weight, bounds) in [
            (i16::MAX, (0, 2000)),
            (1, (0, 2000)),
            (0, (0, 2000)),
            (-1, (-2000, 0)),
            (i16::MIN, (-2000, 0)),
        ] {
            let s = Synapse::new(3, 7, weight).expect("valid ids");
            assert_eq!((s.min_weight(), s.max_weight()), bounds, "weight {weight}");
            assert_eq!(s.weight(), weight, "kept as given");
        }
    }

    #[test]
    fn self_connection_rejected() {
        let err = Synapse::new(5, 5, 100).unwrap_err();
        assert_eq!(err, Error::SelfConnection);
    }

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn normalized_weight_is_percentage_of_max() {
        let mut s = Synapse::new(1, 2, 100).expect("valid ids");
        assert_eq!(s.max_weight(), 2000);
        // 100/2000 = 5%
        assert_eq!(s.normalized_weight(), 5);
        s.weight = s.max_weight;
        assert_eq!(s.normalized_weight(), 100);
        s.weight = -s.max_weight;
        assert_eq!(s.normalized_weight(), -100);
        s.max_weight = 0;
        assert_eq!(s.normalized_weight(), 0, "zero max must not divide by zero");
    }

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn weight_clamped_at_bounds() {
        let mut s = Synapse::new(1, 2, 100).expect("valid ids");
        s.update_weight(10_000); // Way over max
        assert_eq!(s.weight(), s.max_weight());
        // 10,000 given, 1,900 applied: the clamp absorbed 8,100
        assert_eq!((s.raw_stdp_delta(), s.absorbed_delta()), (10_000, 8_100));
        s.update_weight(-10_000); // Way under min
        assert_eq!(s.weight(), s.min_weight());
        // -10,000 given, -2,000 applied: -8,000 more absorbed
        assert_eq!((s.raw_stdp_delta(), s.absorbed_delta()), (0, 100));
    }

    // ----- STDP rule tests -----

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn stdp_ltp_when_pre_before_post() {
        // dt < 0 means pre fired before post → LTP (positive delta).
        let rule = STDPRule::new();
        let dt_us: i32 = -5_000; // Pre 5ms before post
        let delta = rule.calculate_weight_change(dt_us);
        assert!(
            delta >= 0,
            "pre-before-post must produce LTP (>=0), got {delta}"
        );
    }

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn stdp_ltd_when_post_before_pre() {
        // dt > 0 means post fired before pre → LTD (negative delta).
        let rule = STDPRule::new();
        let dt_us: i32 = 5_000;
        let delta = rule.calculate_weight_change(dt_us);
        assert!(
            delta <= 0,
            "post-before-pre must produce LTD (<=0), got {delta}"
        );
    }

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn stdp_zero_outside_window() {
        let rule = STDPRule::new();
        // 10× tau_plus = 200_000 μs → outside window
        let far_dt: i32 = -200_000;
        assert_eq!(rule.calculate_weight_change(far_dt), 0);
        let far_dt_pos: i32 = 200_000;
        assert_eq!(rule.calculate_weight_change(far_dt_pos), 0);
    }

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn stdp_zero_at_zero_dt() {
        // dt = 0 → simultaneous; LTP branch with decay = 0 → factor = SCALE → a_plus · lr.
        let rule = STDPRule::new();
        let delta = rule.calculate_weight_change(0);
        let expected =
            (i32::from(rule.a_plus) * SCALE * i32::from(rule.learning_rate)) / (SCALE * SCALE);
        assert_eq!(delta, expected as i16);
    }

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn stdp_decay_monotonic_with_abs_dt() {
        // Larger |dt| → smaller |delta| (within window).
        let rule = STDPRule::new();
        let small = rule.calculate_weight_change(-1_000).abs();
        let large = rule.calculate_weight_change(-10_000).abs();
        assert!(
            large <= small,
            "|delta| must decay with |dt|: small={small}, large={large}"
        );
    }

    // ----- R18 rider: the i32 dt-overflow fix, pinned -----

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn stdp_zero_just_past_the_old_overflow_boundary() {
        // dt = ±2_200_000 μs: past 10·tau (200 ms) AND past the old
        // i32 product boundary (~2.147 s), where the wrapped decay
        // produced −560 instead of 0. Exact pins, both branches.
        let rule = STDPRule::new();
        assert_eq!(rule.calculate_weight_change(2_200_000), 0, "LTD branch");
        assert_eq!(rule.calculate_weight_change(-2_200_000), 0, "LTP branch");
        // The extremes: cast-before-abs must be total at i32::MIN
        // (`.abs()` on i32::MIN panics in debug, wraps in release).
        assert_eq!(rule.calculate_weight_change(i32::MAX), 0);
        assert_eq!(rule.calculate_weight_change(i32::MIN), 0);
    }

    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn stdp_extreme_learning_rate_does_not_overflow_the_product() {
        // The subsumed tail edge: a · factor · learning_rate in i32
        // overflows at learning_rate = u16::MAX (53 · 1000 · 65535 ≈
        // 3.5e9 > i32::MAX). The i64 expression must carry it.
        let mut rule = STDPRule::new();
        rule.learning_rate = u16::MAX;
        // dt = 0 → factor = SCALE exactly (the maximal product).
        let delta = rule.calculate_weight_change(0);
        let expected = (i64::from(rule.a_plus) * i64::from(SCALE) * i64::from(u16::MAX))
            / (i64::from(SCALE) * i64::from(SCALE));
        assert_eq!(
            i64::from(delta),
            expected,
            "no wrap at lr = u16::MAX, dt = 0"
        );
    }

    // ----- Property tests (Cardano-grade rigor) -----

    proptest! {
        /// Self-connection always rejected, regardless of weight.
        #[test]
        fn prop_self_connection_always_rejected(id in 0u16..=1000, weight in -3000i16..=3000) {
            let result = Synapse::new(id, id, weight);
            prop_assert_eq!(result.err(), Some(Error::SelfConnection));
        }

        /// Weight stays within `[min_weight, max_weight]` after any update,
        /// for every synapse `new` builds: a caller cannot write the
        /// bounds, so the clamp never meets a minimum above the maximum.
        #[cfg(feature = "unstable-stdp")]
        #[test]
        fn prop_weight_clamped(weight in any::<i16>(), delta in any::<i16>()) {
            let mut s = Synapse::new(1, 2, weight).expect("valid ids");
            prop_assert!(s.min_weight() <= s.max_weight());
            s.update_weight(delta);
            prop_assert!(s.weight() >= s.min_weight());
            prop_assert!(s.weight() <= s.max_weight());
        }

        /// STDP sign convention holds across random dt.
        #[cfg(feature = "unstable-stdp")]
        #[test]
        fn prop_stdp_sign_convention(dt_us in -200_000i32..=200_000) {
            let rule = STDPRule::new();
            let delta = rule.calculate_weight_change(dt_us);
            if dt_us > 0 {
                prop_assert!(delta <= 0, "post-before-pre must produce LTD");
            } else if dt_us < 0 {
                prop_assert!(delta >= 0, "pre-before-post must produce LTP");
            }
            // dt == 0 → LTP branch (>= 0), tested separately.
        }

        /// STDP decays to zero outside 10× tau window.
        #[cfg(feature = "unstable-stdp")]
        #[test]
        fn prop_stdp_zero_outside_window(multiplier in 11u32..=100) {
            let rule = STDPRule::new();
            let far_dt_pos = (rule.tau_minus_us * multiplier) as i32;
            let far_dt_neg = -((rule.tau_plus_us * multiplier) as i32);
            prop_assert_eq!(rule.calculate_weight_change(far_dt_pos), 0);
            prop_assert_eq!(rule.calculate_weight_change(far_dt_neg), 0);
        }

        /// R18 rider: across the FULL i32 dt range — beyond the 10·tau
        /// window the delta is exactly 0 (no wrap artifacts at any
        /// magnitude, either sign), and inside the window the sign
        /// convention holds. The old i32 product wrapped at |dt| >
        /// ~2.15 s, producing spurious in-window-looking deltas.
        #[cfg(feature = "unstable-stdp")]
        #[test]
        fn prop_stdp_dt_full_i32_range(dt_us in i32::MIN..=i32::MAX) {
            let rule = STDPRule::new();
            let delta = rule.calculate_weight_change(dt_us);
            let outside =
                i64::from(dt_us).abs() >= 10 * i64::from(rule.tau_plus_us.max(rule.tau_minus_us));
            if outside {
                prop_assert_eq!(delta, 0, "outside the window at dt={}", dt_us);
            } else if dt_us > 0 {
                prop_assert!(delta <= 0, "in-window post-before-pre must be LTD at dt={}", dt_us);
            } else {
                prop_assert!(delta >= 0, "in-window pre-before-post must be LTP at dt={}", dt_us);
            }
        }
    }

    // ----- ISA round 70: a test for the synapse sentence no test held at
    // 645afa8, where a mutation pass left three survivors; run green there
    // and red under each. -----

    /// `STDPRule::new` is the rule its doc and the crate README name: 20 ms
    /// and 20 ms, a₊ 50, a₋ −53, learning rate 100. `stdp_zero_at_zero_dt`
    /// reads `a_plus` from the rule itself, so it cannot see the number.
    #[cfg(feature = "unstable-stdp")]
    #[test]
    fn the_default_rule_is_the_one_its_doc_names() {
        let r = STDPRule::new();
        assert_eq!(
            (
                r.tau_plus_us,
                r.tau_minus_us,
                r.a_plus,
                r.a_minus,
                r.learning_rate
            ),
            (20_000, 20_000, 50, -53, 100)
        );
    }
}
