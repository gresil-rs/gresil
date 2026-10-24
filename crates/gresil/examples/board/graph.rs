pub mod stranger {
    use gresil::{FixedSynapse, LIFNeuron, NeuronType, VoltageResolution};
    const _: () = assert!(gresil::STEP_REVISION == 1);

    pub const N: usize = 2;
    pub const S: usize = 1;
    pub const DT_US: u32 = 100;
    pub const STEPS: u32 = 100;
    pub const SPIKES_ONLY: bool = false;
    pub const HEADER: &str = "# gresil-trace v1 case=stranger kind=stranger n=2 dt_us=100 res=cmV plasticity=off divisor=10 steps=100 rows=all";
    pub const NEURONS: [LIFNeuron; N] = [
        LIFNeuron::new(0, NeuronType::Excitatory, VoltageResolution::CentiMillivolt).with_resting_potential(0).with_threshold(1001).with_reset_potential(0).with_tau_membrane_us(5000).with_refractory_period_us(0).with_resistance_mohm(500).with_noise_amplitude_pa(0).with_adaptation_enabled(false),
        LIFNeuron::new(1, NeuronType::Excitatory, VoltageResolution::CentiMillivolt).with_resting_potential(0).with_threshold(1001).with_reset_potential(0).with_tau_membrane_us(5000).with_refractory_period_us(0).with_resistance_mohm(500).with_noise_amplitude_pa(0).with_adaptation_enabled(false),
    ];
    pub const SYNAPSES: [FixedSynapse; S] = [
        FixedSynapse::new(0, 1, 1000),
    ];
    pub const DRIVE: &[(u32, [i16; N])] = &[
        (10, [1000, 0]),
        (40, [0, 0]),
        (10, [1000, 0]),
        (40, [0, 0]),
    ];
}
