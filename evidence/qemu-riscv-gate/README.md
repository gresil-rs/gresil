# QEMU riscv64gc gate — the no_std proof (2026-08-21)

The evidence gate for the VISION claim "runs on RISC-V edge silicon":
`neuralos-snn` proved on `riscv64gc` under QEMU, both postures —
bare-metal (Leg A) and linux-user running the REAL test suite (Leg B,
the wire crossing the border: `network.rs` transmission + Leg-C pins
executed on riscv64gc).

## Corrected pre-flight (arrival checks supersede the brief)

The commission said NOTHING qemu was installed (from ISA R17's
2026-08-22 note). Reality at session open, verified before any
install:

- `qemu-user-static` + `qemu-system-misc` 1:8.2.2-0ubuntu1.18 already
  installed (`dpkg -l`: `ii` both) — the principal-side install ran
  before the session opened
- `qemu-system-riscv64` and `qemu-riscv64-static` on PATH, version 8.2.2
- binfmt_misc handlers `qemu-riscv64` registered
  (`/proc/sys/fs/binfmt_misc/`) — riscv64 binaries execute transparently
- The only genuinely missing pieces were the rustup targets (userland)

So the sudo `apt install` line was skipped (ruling at scope time); no
sudo ran this session.

## Commissioning correction (recorded, not hidden)

The commission's Leg B triple — `riscv64gc-unknown-linux-gnu` +
`rust-lld` + static + no system cross-toolchain — was contradictory:
rustup ships no riscv64 glibc sysroot, so the gnu link fails on
missing `crt1.o`/`libc` no matter the linker. Ruling at scope time:
**musl swap** — `riscv64gc-unknown-linux-musl` with rustup's
self-contained static musl libc (`-C link-self-contained=yes -C
target-feature=+crt-static`), linked by `rust-lld`, zero system deps.
Claim language stays "riscv64gc under QEMU"; Leg A none-elf covers
bare-metal. No libc overclaim in either direction.

## Files

| File | What |
|---|---|
| `leg-a.log` | Leg A run transcript — 175/175 cited checks, raster, `LEG A PASS`, `qemu_exit=0` |
| `leg-a-alpha8.log` | Leg A re-run at the alpha.8 stamp (2026-09-14, `main@6f03e1b`), the same harness: 175/175 cited checks, `LEG A PASS`; two header lines prepended at PR G (§ Leg A) |
| `leg-a-alpha9.log` | Leg A re-run at the alpha.9 stamp (2026-10-22, `main@691bb70`), the same harness: 171 cited checks, `LEG A PASS`; its two header lines the run's own (§ Leg A) |
| `leg-b.log` | Leg B full-suite run — 187 unit + 8 integration = 195/195 (matches host count), `cargo_test_exit=0`, wall clock |
| `leg-b-alpha9.log` | Leg B re-run at the alpha.9 stamp (2026-10-22, `main@691bb70`): the full suite with its doctests, 391 passed, 0 failed, exit 0 (§ Leg B) |
| `replay-alpha8.log` | the trace replay (round 33, 2026-09-14): the twelve plasticity-off traces stepped by `FixedNetwork` on bare-metal riscv64gc, as `proofs/qemu-trace-replay/build.sh` captured them from the UART; identical to the tree's files by `tools/esp32c3_trace_diff.py` (§ The trace replay) |
| `replay-alpha9-g.log` | the trace replay re-run at PR G (round 34, 2026-09-14): the fourteen plasticity-off traces, the two D8 witnesses among them, every row written by the library's `neuralos_snn::fixed::row`; identical to the tree's files, `14 cases, 0 red` (§ The trace replay) |
| `replay-r52.log` | the trace replay re-run at round 52 (NIR parity's first PR, 2026-09-28): the fourteen plasticity-off traces, the two D8 witnesses in simulation units at 0.1 ms; identical to the tree's files, `14 cases, 0 red` (§ The trace replay) |
| `replay-r61.log` | the trace replay re-run at round 61 (the noise default and the clock, 2026-10-07): the fourteen plasticity-off traces, `one-neuron-board` without noise; identical to the tree's files, `14 cases, 0 red` (§ The trace replay) |
| `replay-alpha9.log` | the trace replay re-run at the alpha.9 stamp (2026-10-22, `main@691bb70`): the fourteen plasticity-off traces; identical to the tree's files, `14 cases, 0 red` (§ The trace replay) |
| `SHA256SUMS` | pins the ten logs and this README |
| `../../proofs/qemu-riscv-leg-a/` | the committed, rebuildable Leg A harness crate (standalone workspace; repo workspace untouched) |
| `../../proofs/qemu-trace-replay/` | the trace replay's crate (standalone workspace, like Leg A's) and its `build.sh` |

## Leg A — bare-metal none-elf (the edge posture)

Harness: `proofs/qemu-riscv-leg-a` — `no_std`/`no_main`, RAM entry
`0x8000_0000`, no allocator, own `[workspace]` table (excluded from
the repo workspace by construction; `cargo check --workspace` from the
root stays green and never sees it). Exit via the sifive_test
poweroff device (`0x5555` → 0, `0x13333` → 1). GOTCHA found and fixed
live: the test device is at `0x0010_0000` (1 MiB), NOT `0x0100_0000`
— one hex digit, writes silently swallowed into unmapped space.

175 exact-value checks, each citing its source unit test by name
(same numbers, same names; drift would be a finding, never adjusted
to match): lif (mV trace, dead-zone pair, leak convergence,
adaptation decay +2/spike, centi bounds, rectification ratchet) ·
synapse (AMPA/GABA defaults, STDP zero-dt/outside-window, clamping,
self-connection) · bridge (i2_s/q1_0/q2_0 known vectors byte-exact,
the REAL Bonsai Q2_0 first-block census +37/0×43/−48 with decode→encode
byte identity, 12 fp16 widening vectors, γ policy, repack order) ·
trit (boundaries γ=125/γ=5, half-up scale, projection snap,
draw-0/draw-max flips) · kernel (pack 0x52, absmax incl. i16::MIN→
32768 no-wrap, matvec [800, −6207]) · nir quantizers (threshold_q
−5550 on centi grid, dyadic arena [16384, −32767, 8192], scale
1/32767, denormal-loud, full error taxonomy).

Honest shape note: `network.rs`/`csr.rs`/`stats.rs` are std-gated —
the network-level transmission wire is Leg B's job; the raster
(4 neurons × 40 steps, same 300 µA drive: mV grid silent, centi grid
fires) is LIFNeuron-level on target.

```bash
# rebuild + run (all copy-pasteable from repo root)
rustup target add riscv64gc-unknown-none-elf
cd proofs/qemu-riscv-leg-a && cargo build
timeout 120 qemu-system-riscv64 -machine virt -nographic -bios none \
  -kernel target/riscv64gc-unknown-none-elf/debug/qemu-riscv-leg-a
# gate: echo $? == 0 → PASS (1 → FAIL); UART carries the check log
```

The FAIL direction is verified too: a deliberately-broken check was
probed (exit 1, `FAIL [ 26] lif::centi_mode membrane got=-7000
want=-7001`), then restored — the gate bites.

Re-run at the alpha.8 stamp, `leg-a-alpha8.log` is the first run since
2026-08-22, 175/175, same verdict per check (every check line identical
to `leg-a.log`'s; the capture ends at `LEG A PASS`, no `# qemu_exit`
line); its two header lines, `# date:` (the file's mtime) and
`# commit:`, were prepended at PR G (round 34) to the capture of sha
`ab189be96472f449646b8ff7c936c9c69bb9e9f956535de500ff40e35e8720d4`.

Since round 65 (2026-10-11) leg A prints the current in pA, as the
library names it, and the unit test two of its checks cite is
`the_dead_zone_12pa_pair`. Eight printed lines differ from `leg-a.log`,
each in its unit alone, old → new:

| `leg-a.log` | since round 65 |
|---|---|
| `ok [ 15] lif::the_dead_zone_12ua_pair mV` | `ok [ 15] lif::the_dead_zone_12pa_pair mV` |
| `ok [ 16] lif::the_dead_zone_12ua_pair centi` | `ok [ 16] lif::the_dead_zone_12pa_pair centi` |
| `ok [ 33] lif::rectification +12ua ratchets` | `ok [ 33] lif::rectification +12pa ratchets` |
| `ok [ 34] lif::rectification -12ua absorbed` | `ok [ 34] lif::rectification -12pa absorbed` |
| `n0 mV  +600uA` | `n0 mV  +600pA` |
| `n1 mV  +300uA` | `n1 mV  +300pA` |
| `n2 cmV +300uA (same drive as n1, finer ruler)` | `n2 cmV +300pA (same drive as n1, finer ruler)` |
| `n3 cmV  +12uA` | `n3 cmV  +12pA` |

Every value, every raster row and every other printed line are as
recorded. `leg-a.log` and `leg-a-alpha8.log` keep `uA` as printed, and
`leg-b.log`, which lists the suite by test name, keeps the test's old
name as it ran. The "300 µA drive" of this section, above, is 300 pA.

Since round 65 four checks reach their starting state through the
public API, where the unit tests they cite write the fields: the leak
from −90 and from −20 and the adaptation decay from 3, by the step and
the chain, and the centi-mV floor from rest, where its test starts at
−9,999. A comment at each says how; each keeps its name and its value.

Since round 67 (step 7) leg A runs 169 checks. The six that read
`Synapse`'s type and its two time constants, for each sign, went with
those fields: `[ 38]` to `[ 40]` and `[ 43]` to `[ 45]` of `leg-a.log`
(`synapse::default_params type`, `tau_rise`, `tau_decay`, then
`synapse::negative_weight` the same three). Every check after them
prints its index 3 lower from `[ 41]` to `[ 42]` and 6 lower from
`[ 46]` on, with its name and its value as recorded, and the last line
counts `checks: 169`.

Since round 76 leg A runs 171 checks. The two new ones, `[162]` and
`[163]`, follow the denormal checks and cite
`quantize_linear_f64_max_absmax_is_loud`: a Linear weight of
`f64::MAX`, then of `-f64::MAX`, beside a 1.0 is refused with
`BadNumber("weight")`. The eight checks after them,
`nir::rejects_loudly`, print their index 2 higher, `[164]` to `[171]`,
with their names and their values as recorded, and the last line
counts `checks: 171`. Since the same round `main.rs` is in rustfmt's
shape, which moved no printed line.

Re-run at the alpha.9 stamp (2026-10-22, `main@691bb70`),
`leg-a-alpha9.log` is 171 checks, 0 failed, `LEG A PASS`, on QEMU
10.2.1 (`qemu-system-riscv64`), exit 0. Against `leg-a-alpha8.log`,
the check numbers set aside, it differs in the crate's name in its
banner and the line after it (round 74's rename), and where this
section says leg A moved: the four check names and the four raster
labels in pA, round 67's six checks out, round 76's two in, and the
count. Its two header lines are the run's own, `# date:` its start and
`# commit:`, in `leg-a-alpha8.log`'s form; a run of the same code in a
scratch clone prints the same UART bytes.

## Leg B — user-mode, the REAL suite (the wire crosses)

Full workspace test suite of the crate — NOT a subset — built static
musl with rust-lld (no system cross-toolchain) and executed via
binfmt (`qemu-riscv64-static` 8.2.2 underneath):

```bash
rustup target add riscv64gc-unknown-linux-musl
CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-C link-self-contained=yes -C target-feature=+crt-static" \
RUSTDOCFLAGS="-C link-self-contained=yes -C target-feature=+crt-static" \
  cargo test -p gresil --features unstable-stdp --target riscv64gc-unknown-linux-musl
# gate: exit 0, 0 failed; at the alpha.9 stamp 391 passed, 367 tests and
# 24 doctests, 172.6 s wall under TCG with the build; the first run's
# 187 + 8 = 195 took ~14 s
# `--features unstable-stdp` since PR L (2026-09-19): STDP is behind it, and
# without the flag the suite runs with the STDP tests compiled out. The
# first run had no flag to give; Leg B was not re-run at PR L.
# `RUSTDOCFLAGS` since round 72 (2026-10-17): cargo builds the doctests for
# the target too, and rustdoc takes its flags from RUSTDOCFLAGS, never from
# the target's RUSTFLAGS, so without it every doctest fails to link
# (`unable to find library -lgcc_s`, then `-lc`). The first run had no
# doctest to link.
```

Named tests that crossed the border (in `leg-b.log`): the
transmission trio (`transmission_is_live_one_step_delayed_centimv`,
`..._mv_strong_weight`, `transmission_pulses_sum_across_presynaptic_spikes`),
the two Leg-C pins (`plasticity_off_freezes_weights_under_adapting_drive`,
`adaptation_decay_runs_before_integration_exact`), the F1/R4(ii) divisor
pins, and the CSR pins (`sparse_matrix_iter_returns_added_synapses`,
`csr_finalize_recovers_correct_members_for_unsorted_insertion` — CSR is
std-gated, so it crossed here, not in Leg A). Doctests: the crate has
0 (reported `0 passed`, trivially clean). Threads under qemu-user+musl
behaved; `--test-threads=1` was not needed.

Re-run at the alpha.9 stamp (2026-10-22, `main@691bb70`),
`leg-b-alpha9.log` is the command above on QEMU 10.2.1 user-mode, by
binfmt (`/usr/bin/qemu-riscv64`): exit 0, 391 passed, 367 tests in
seven binaries and 24 doctests, 0 failed, 0 ignored; 172.6 s in all,
the build 45.87 s, the load 0.72 at the start. `nir_fixtures`, whose
mutation loop has 60 s for its 10,000 cases, finished in 45.30 s.
Since round 72 the run links the doctests too; the first run's crate
had none. Its two header lines, `# date:` and `# commit:`, are the
run's own, as `leg-a-alpha9.log`'s are.

## Verdicts of record

- Leg A: **PASS** — exit 0, 175/175 cited checks, bare-metal riscv64gc,
  no allocator
- Leg B: **PASS** — full suite 195/195 on riscv64gc under QEMU
  user-mode, exit 0
- Host battery after both legs: unchanged (294 workspace tests green,
  clippy `-D warnings` clean, no_std build green, zero `crates/` edits)

## Parked follow-up (named, not started)

QEMU riscv64 CI leg — runner cost under TCG unmeasured (Leg B was 14 s
warm here, but CI cost needs its own measurement); not added to
ci.yml this session by commission.

## The trace replay (round 33, 2026-09-14)

`proofs/qemu-trace-replay/` is the ESP32-C3 firmware's replay under
QEMU: every plasticity-off case of `crates/neuralos-snn/tests/traces/`
stepped by `FixedNetwork`, bare metal on `riscv64gc-unknown-none-elf`
(Leg A's boot, copied), its header line and its rows written over the
UART by the library's row writer (`tests/traces/row.rs` at round 33,
`neuralos_snn::fixed::row` from PR G, `neuralos_snn::trace::row` since
round 51), then the end line, then the pass code.
Its `build.sh` builds the crate `--release --locked` on the pin
(1.98.1, the spine at `0.1.0-alpha.8`), runs it under
`qemu-system-riscv64` 8.2.2 (`virt`, `-bios none`, a 120 s timeout),
and diffs the capture with the board's tool,
`tools/esp32c3_trace_diff.py`: green only on QEMU exit 0 and
`N cases, 0 red`, N the tree's plasticity-off traces, which the script
counts (twelve at round 33, fourteen since PR G). `replay-alpha8.log` is that capture (1,210 lines,
35,807 bytes), built from the sources of the commit that adds it: the
twelve cases identical, with the board's row counts
(`evidence/esp32c3-bringup/README.md` § Sixth entry), QEMU exit 0.
Three runs gave the same bytes; `build.sh` refuses a log name that
exists, so a re-run writes a new one and compares. Not a CI job: QEMU
is not on the runner image, and Leg A is not one either; it runs by
hand at a release round and is pinned. Check 1 of `docs/ROADMAP.md`
§ 0.1.0 rests on it.

Re-run at PR G (round 34, 2026-09-14), `replay-alpha9-g.log` is the
fourteen cases, 0 red, the two D8 witnesses added and every row written
by the library's `neuralos_snn::fixed::row` (1,512 lines, 41,784 bytes,
two runs the same bytes); `replay-alpha8.log` stays as pinned, the
capture of round 33's tree.

Re-run at round 52 (NIR parity's first PR, 2026-09-28),
`replay-r52.log` is the fourteen cases, 0 red, the two D8 witnesses
imported in simulation units at 0.1 ms (1,512 lines, 41,959 bytes, the
same bytes as a run of the same code in a scratch clone). Against
`replay-alpha9-g.log` it differs at lines 1,210 to 1,511, the two
witness cases, and nowhere else; `replay-alpha9-g.log` stays as
pinned, the capture of round 34's tree.

Re-run at round 61 (the noise default and the clock, 2026-10-07),
`replay-r61.log` is the fourteen cases, 0 red, `one-neuron-board` run
without noise (1,512 lines, 41,959 bytes, the same bytes as a run of the
same code in a scratch clone). Against `replay-r52.log` it differs in
that case alone, 131 of its 147 rows between lines 158 and 299, and
nowhere else; `replay-r52.log` stays as pinned, the capture of round
52's tree.

Re-run at the alpha.9 stamp (2026-10-22, `main@691bb70`),
`replay-alpha9.log` is the fourteen cases, 0 red, on QEMU 10.2.1
(1,512 lines, 41,749 bytes, the same bytes as a run of the same code
in a scratch clone). Against `replay-r61.log` it differs in the two D8
witness cases alone, 143 lines of `snntorch-two-layer` and 45 of
`two-lif-neurons`, their headers equal: round 65 moved those two
traces (`f25b26f`, the adaptation switch, off in simulation units).
`replay-r61.log` stays as pinned, the capture of round 61's tree.

```bash
rustup target add riscv64gc-unknown-none-elf
proofs/qemu-trace-replay/build.sh      # the capture into its target/replay.log, the diff, the verdict
cmp proofs/qemu-trace-replay/target/replay.log evidence/qemu-riscv-gate/replay-alpha9.log   # at the tree that added it; replay-r61.log at round 61's, replay-r52.log at round 52's, replay-alpha9-g.log at round 34's, replay-alpha8.log at round 33's
```
