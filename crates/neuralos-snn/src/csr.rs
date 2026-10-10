//! Compressed Sparse Row (CSR) synapse storage — O(1) per-presynaptic
//! iteration, forward + reverse.
//!
//! Split from `network.rs` at R4(ii) (2026-08-20). Private to the crate
//! since round 46: the network's storage, not API.

/// Compressed Sparse Row (CSR) synapse storage — O(1) per-presynaptic iteration.
///
/// Forward direction: row key = presynaptic neuron. [`connections`]`(pre_id)`
/// iterates that neuron's outgoing edges.
/// Reverse direction: row key = postsynaptic neuron. `incoming(post_id)`
/// iterates that neuron's incoming edges (used by the post-firing LTP half of
/// pairwise STDP).
///
/// Forward parallel arrays (sorted by `pre_id` after [`finalize`]):
/// - `weights[k]`: weight of the edge at sorted position `k`
/// - `col_indices[k]`: postsynaptic neuron ID of that edge
/// - `synapse_indices[k]`: index of that edge in `SpikingNeuralNetwork::synapses`
/// - `pre_ids[k]`: presynaptic neuron ID of that edge
/// - `row_ptrs[n]`: start index into the arrays for presynaptic neuron `n`
///   (length `neuron_count` + 1; last entry = total edge count)
///
/// Reverse parallel arrays (sorted by `post_id` after [`finalize`]):
/// - `rv_pre_ids[k]`: presynaptic neuron ID of the edge at reverse-sorted `k`
/// - `rv_syn_indices[k]`: that edge's index in `SpikingNeuralNetwork::synapses`
/// - `rv_row_ptrs[n]`: start index for postsynaptic neuron `n`
///
/// `weight_index_of[synapse_index]` is the **inverse permutation** of
/// `synapse_indices`: given a synapse index, it returns the forward-sorted
/// position holding that synapse's weight. [`set_weight`] routes through it so a
/// plasticity update hits the correct CSR slot even after [`finalize`] reorders
/// the arrays (without it, `set_weight(syn_idx)` would write `weights[syn_idx]`,
/// which after the counting sort holds a *different* edge's weight).
///
/// **The rows are [`finalize`]'s.** [`add`] appends an edge and marks the
/// matrix dirty ([`is_dirty`]); [`connections`] and `incoming` read the
/// rows the last [`finalize`] built, which hold no edge added since. So a
/// dirty matrix is sorted before it is read: `SpikingNeuralNetwork::step`
/// does it first, and `build_topology` after its builder. [`finalize`] is a
/// stable counting sort (O(edges + neurons)): it reorders the forward
/// arrays by `pre_id`, `pre_ids` with them, builds the reverse arrays by
/// `post_id`, and inverts the synapse-index permutation into
/// `weight_index_of`. It sorts whatever order it finds, so it is right on
/// any state: a second call, or one after more adds, keeps each `pre`'s
/// edges in the order they were added. Until round 64 `add` bumped
/// `row_ptrs` instead, right only for edges added in `pre` order, and
/// `finalize` left `pre_ids` behind, so a second one scrambled the rows; a
/// step read either as it found it and delivered pulses to the wrong
/// targets.
///
/// [`finalize`]: SparseSynapseMatrix::finalize
/// [`connections`]: SparseSynapseMatrix::connections
/// [`add`]: SparseSynapseMatrix::add
/// [`is_dirty`]: SparseSynapseMatrix::is_dirty
/// [`set_weight`]: SparseSynapseMatrix::set_weight
#[derive(Debug, Clone)]
pub struct SparseSynapseMatrix {
    weights: Vec<i16>,
    col_indices: Vec<u16>,
    synapse_indices: Vec<usize>,
    /// Presynaptic neuron ID of each edge, aligned with the other forward
    /// arrays: [`finalize`] sorts it with them, so the next one sorts the
    /// order they hold.
    ///
    /// [`finalize`]: SparseSynapseMatrix::finalize
    pre_ids: Vec<u16>,
    /// Length = `neuron_count` + 1: the rows of the last [`finalize`]. An
    /// edge added since sits past them, in no row, until the next one.
    ///
    /// [`finalize`]: SparseSynapseMatrix::finalize
    row_ptrs: Vec<u32>,
    /// Inverse permutation: `weight_index_of[synapse_index]` = forward
    /// position of that synapse's weight slot. An edge added since the last
    /// [`finalize`] sits at its own index, in insertion order; [`finalize`]
    /// rebuilds the whole as the inverse of `synapse_indices`. Lets
    /// [`set_weight`] stay O(1) and correct on either side of a sort.
    ///
    /// [`finalize`]: SparseSynapseMatrix::finalize
    /// [`set_weight`]: SparseSynapseMatrix::set_weight
    weight_index_of: Vec<usize>,
    /// Reverse CSR (by postsynaptic neuron). Built alongside the forward
    /// sort in [`finalize`]: empty until the first one, when `incoming`
    /// returns nothing, and blind like the forward rows to an edge added
    /// since.
    ///
    /// [`finalize`]: SparseSynapseMatrix::finalize
    rv_row_ptrs: Vec<u32>,
    rv_pre_ids: Vec<u16>,
    rv_syn_indices: Vec<usize>,
    /// Total neuron count (sets `row_ptrs` length).
    neuron_count: u16,
    /// True from an [`add`] until the next [`finalize`] or [`clear`]: the
    /// rows then miss an edge.
    ///
    /// [`add`]: SparseSynapseMatrix::add
    /// [`finalize`]: SparseSynapseMatrix::finalize
    /// [`clear`]: SparseSynapseMatrix::clear
    dirty: bool,
}

impl SparseSynapseMatrix {
    /// New empty CSR matrix sized for `neuron_count` presynaptic neurons,
    /// with room for `capacity` synapses.
    #[must_use]
    pub fn new(neuron_count: u16, capacity: usize) -> Self {
        Self {
            weights: Vec::with_capacity(capacity),
            col_indices: Vec::with_capacity(capacity),
            synapse_indices: Vec::with_capacity(capacity),
            pre_ids: Vec::with_capacity(capacity),
            row_ptrs: vec![0; neuron_count as usize + 1],
            weight_index_of: Vec::with_capacity(capacity),
            rv_row_ptrs: Vec::new(),
            rv_pre_ids: Vec::with_capacity(capacity),
            rv_syn_indices: Vec::with_capacity(capacity),
            neuron_count,
            dirty: false,
        }
    }

    /// Append a synapse and mark the matrix dirty. The edge is in no row of
    /// [`connections`] or `incoming` until the next [`finalize`], which the
    /// step runs on a dirty matrix before it reads one. O(1): the rows are
    /// left to the sort.
    ///
    /// [`finalize`]: Self::finalize
    /// [`connections`]: Self::connections
    pub fn add(&mut self, pre_id: u16, post_id: u16, weight: i16, synapse_index: usize) {
        debug_assert!(
            pre_id < self.neuron_count,
            "pre_id {pre_id} ≥ neuron_count {}",
            self.neuron_count
        );
        self.weights.push(weight);
        self.col_indices.push(post_id);
        self.synapse_indices.push(synapse_index);
        self.pre_ids.push(pre_id);
        // An edge added since the last sort sits at its own index:
        // synapse i lives at weights[i]. finalize() rebuilds the whole
        // post-sort.
        self.weight_index_of.push(synapse_index);
        self.dirty = true;
    }

    /// Build the CSR layout via a stable counting sort by `pre_id`, and
    /// mark the matrix clean. O(edges + neurons). Reorders the forward
    /// parallel arrays, `pre_ids` with them, so each presynaptic neuron's
    /// outgoing edges are contiguous and in the order they were added;
    /// rebuilds `row_ptrs` from real per-neuron out-degrees; builds the
    /// inverse permutation `weight_index_of`, so [`set_weight`] stays
    /// correct post-sort; and builds the reverse CSR (by `post_id`), so
    /// `incoming` works, each post's edges in the order the forward arrays
    /// held them on entry.
    ///
    /// It sorts whatever order it finds, so any number of calls, between
    /// any [`add`]s, is right: a second call keeps every forward slot, and
    /// a call after more adds keeps each `pre`'s earlier edges before its
    /// later ones.
    ///
    /// [`add`]: Self::add
    /// [`set_weight`]: Self::set_weight
    pub fn finalize(&mut self) {
        let n = self.neuron_count as usize;
        let m = self.weights.len();

        // At entry, weights/col_indices/synapse_indices/pre_ids are
        // mutually aligned, in insertion order or in the last sort's with
        // the edges added since after it. Capture the post + syn copies now
        // (before the forward scatter reassigns
        // col_indices/synapse_indices) so the reverse CSR can be
        // counting-sorted by post_id from the same entry-order source.
        let ins_post: Vec<u16> = self.col_indices.clone();
        let ins_syn: Vec<usize> = self.synapse_indices.clone();

        // ----- Forward sort by pre_id -----
        // Count out-degree per presynaptic neuron.
        let mut degree = vec![0u32; n];
        for &pre in &self.pre_ids {
            degree[pre as usize] += 1;
        }
        // Prefix sum → row_ptrs (length n+1, last entry = total edges).
        self.row_ptrs = vec![0; n + 1];
        for (k, &deg) in degree.iter().enumerate() {
            self.row_ptrs[k + 1] = self.row_ptrs[k] + deg;
        }
        // Stable scatter into sorted order.
        let mut new_weights = vec![0i16; m];
        let mut new_col = vec![0u16; m];
        let mut new_syn = vec![0usize; m];
        let mut new_pre = vec![0u16; m];
        let mut cursor = self.row_ptrs.clone();
        for (i, &pre) in self.pre_ids.iter().enumerate() {
            let pos = cursor[pre as usize] as usize;
            new_weights[pos] = self.weights[i];
            new_col[pos] = self.col_indices[i];
            new_syn[pos] = self.synapse_indices[i];
            new_pre[pos] = pre;
            cursor[pre as usize] += 1;
        }
        self.weights = new_weights;
        self.col_indices = new_col;
        self.synapse_indices = new_syn;

        // ----- Inverse permutation: weight_index_of[syn_idx] = sorted position -----
        // After the scatter, synapse_indices[pos] holds the synapse index living
        // at sorted position pos. Invert it so set_weight(syn_idx) lands on the
        // right slot. Without this, set_weight(syn_idx) would write weights[syn_idx]
        // — which after the sort holds a *different* edge's weight (the bd5b098
        // latent bug: plasticity deltas were written to the wrong CSR slots,
        // corrupting propagation weights).
        let mut inv = vec![0usize; m];
        for (pos, &syn_idx) in self.synapse_indices.iter().enumerate() {
            if syn_idx < m {
                inv[syn_idx] = pos;
            }
        }
        self.weight_index_of = inv;

        // ----- Reverse CSR by post_id (for the LTP post-firing path) -----
        // Stable counting-sort the same edges by postsynaptic neuron, carrying
        // pre_id (still the entry order in self.pre_ids) and synapse index
        // (ins_syn, captured above). ins_post is the sort key.
        let mut post_degree = vec![0u32; n];
        for &post in &ins_post {
            let p = post as usize;
            if p < n {
                post_degree[p] += 1;
            }
        }
        let mut rv_row_ptrs = vec![0u32; n + 1];
        for (k, &deg) in post_degree.iter().enumerate() {
            rv_row_ptrs[k + 1] = rv_row_ptrs[k] + deg;
        }
        let mut rv_pre_ids = vec![0u16; m];
        let mut rv_syn_indices = vec![0usize; m];
        let mut rcursor = rv_row_ptrs.clone();
        for i in 0..m {
            let post = ins_post[i] as usize;
            if post < n {
                let pos = rcursor[post] as usize;
                rv_pre_ids[pos] = self.pre_ids[i];
                rv_syn_indices[pos] = ins_syn[i];
                rcursor[post] += 1;
            }
        }
        self.rv_row_ptrs = rv_row_ptrs;
        self.rv_pre_ids = rv_pre_ids;
        self.rv_syn_indices = rv_syn_indices;
        // The keys follow their edges, so the next finalize sorts the order
        // it finds; the reverse sort above still read the entry order.
        self.pre_ids = new_pre;
        self.dirty = false;
    }

    /// Whether an edge came in since the last [`finalize`] or [`clear`]:
    /// the rows then miss it, and the step sorts before it reads them.
    ///
    /// [`finalize`]: Self::finalize
    /// [`clear`]: Self::clear
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Iterate synapses from `pre_id`. Returns `(post_id, weight, synapse_index)` tuples.
    /// O(1) setup, O(out-degree) iteration.
    #[must_use]
    pub fn connections(&self, pre_id: u16) -> SynapseIter<'_> {
        debug_assert!(pre_id < self.neuron_count);
        let start = self.row_ptrs[pre_id as usize] as usize;
        let end = self.row_ptrs[pre_id as usize + 1] as usize;
        SynapseIter {
            weights: &self.weights[start..end],
            col_indices: &self.col_indices[start..end],
            synapse_indices: &self.synapse_indices[start..end],
            pos: 0,
        }
    }

    /// Iterate **incoming** synapses to `post_id` (reverse CSR). Returns
    /// `(pre_id, synapse_index)` tuples. O(1) setup, O(in-degree) iteration.
    /// Empty until the first [`finalize`], and blind to an edge added since
    /// the last one.
    ///
    /// [`finalize`]: Self::finalize
    #[cfg(any(test, feature = "unstable-stdp"))]
    #[must_use]
    pub fn incoming(&self, post_id: u16) -> IncomingIter<'_> {
        if self.rv_row_ptrs.is_empty() {
            return IncomingIter {
                pre_ids: &[],
                syn_indices: &[],
                pos: 0,
            };
        }
        debug_assert!(post_id < self.neuron_count);
        let start = self.rv_row_ptrs[post_id as usize] as usize;
        let end = self.rv_row_ptrs[post_id as usize + 1] as usize;
        IncomingIter {
            pre_ids: &self.rv_pre_ids[start..end],
            syn_indices: &self.rv_syn_indices[start..end],
            pos: 0,
        }
    }

    /// Update the stored weight for a synapse index. Routes through
    /// `weight_index_of` (the inverse permutation) so the write lands on the
    /// correct forward-sorted slot even after [`finalize`] reordered the arrays.
    ///
    /// [`finalize`]: Self::finalize
    pub fn set_weight(&mut self, synapse_index: usize, weight: i16) {
        if let Some(&pos) = self.weight_index_of.get(synapse_index) {
            if let Some(slot) = self.weights.get_mut(pos) {
                *slot = weight;
            }
        }
    }

    /// Drop all stored edges, keeping the neuron-count sizing and capacity,
    /// and mark the matrix clean: no edge, nothing to sort.
    /// [`SpikingNeuralNetwork::build_topology`](crate::network::SpikingNeuralNetwork::build_topology)
    /// clears it before each build, so a rebuild's edges replace the last
    /// build's instead of adding to them.
    pub fn clear(&mut self) {
        self.weights.clear();
        self.col_indices.clear();
        self.synapse_indices.clear();
        self.pre_ids.clear();
        self.weight_index_of.clear();
        self.rv_row_ptrs.clear();
        self.rv_pre_ids.clear();
        self.rv_syn_indices.clear();
        self.row_ptrs.iter_mut().for_each(|p| *p = 0);
        self.dirty = false;
    }

    /// Total synapse count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.weights.len()
    }
}

/// Iterator over a single presynaptic neuron's outgoing synapses.
#[derive(Debug, Clone)]
pub struct SynapseIter<'a> {
    weights: &'a [i16],
    col_indices: &'a [u16],
    synapse_indices: &'a [usize],
    pos: usize,
}

impl Iterator for SynapseIter<'_> {
    type Item = (u16, i16, usize); // (post_id, weight, synapse_index)

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.weights.len() {
            let item = (
                self.col_indices[self.pos],
                self.weights[self.pos],
                self.synapse_indices[self.pos],
            );
            self.pos += 1;
            Some(item)
        } else {
            None
        }
    }
}

/// Iterator over a single postsynaptic neuron's incoming synapses (reverse CSR).
#[cfg(any(test, feature = "unstable-stdp"))]
#[derive(Debug, Clone)]
pub struct IncomingIter<'a> {
    pre_ids: &'a [u16],
    syn_indices: &'a [usize],
    pos: usize,
}

#[cfg(any(test, feature = "unstable-stdp"))]
impl Iterator for IncomingIter<'_> {
    type Item = (u16, usize); // (pre_id, synapse_index)

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.pre_ids.len() {
            let item = (self.pre_ids[self.pos], self.syn_indices[self.pos]);
            self.pos += 1;
            Some(item)
        } else {
            None
        }
    }
}
