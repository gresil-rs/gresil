// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! The kept conversions (the frozen-format sitting's Call 1 (ii)): the
//! same `.nir`, converted with the same options, gives the same output on
//! every 0.1.x. `tests/kept.tsv` holds, for each tracked `.nir` in three
//! modes (native; `--sim-units`; `--sim-units --freeze` with the default
//! drive, the module named after the `.nir`'s stem as `stranger.sh` names
//! it), the FNV-1a 64 of each output, or the exit code of its refusal. It
//! was written once, from the binary, by a command outside the tree; no
//! test writes it.
//!
//! Each run goes through the binary, `CARGO_BIN_EXE_gresil-nir2json`, the
//! command line a user types, into a folder of its own under the target
//! dir. The stem names the module and the trace's `case=`, so it is
//! inside the pinned bytes. A `--freeze` run's JSON is the `--sim-units`
//! run's, byte for byte. Not kept: the sidecar `<output>.meta.json`,
//! which holds the converter's version and the source path as typed.
//!
//! A red names the file, the mode and the output. From 0.1.0 a conversion
//! that moves ships behind a new flag until 0.2.0 (Call 1 (ii)); before
//! 0.1.0, its rows are re-pinned in their own commit. A refusal that
//! starts to convert is not ruled yet: it is decided when it first
//! happens. And the three folders that hold `.nir` files,
//! `crates/gresil-nir2json/tests/fixtures/` and below,
//! `crates/neuralos-rt/tests/nir_fixtures/` and `evidence/nir-hdf5-gate/`,
//! hold none without its rows, so a new fixture cannot join unpinned.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The three modes, as the rows name them.
const MODES: [&str; 3] = ["native", "sim-units", "freeze"];

/// The folders that hold the tree's `.nir` files, from the repository
/// root.
const FOLDERS: [&str; 3] = [
    "crates/gresil-nir2json/tests/fixtures",
    "crates/neuralos-rt/tests/nir_fixtures",
    "evidence/nir-hdf5-gate",
];

/// The manifest's header row.
const HEADER: &str = "path\tmode\toutput\tvalue";

/// What the rule says when a kept conversion moves.
const RULE: &str = "From 0.1.0 a conversion that moves ships behind a new flag until 0.2.0 \
                    (the frozen-format sitting's Call 1 (ii)); before 0.1.0, its rows are \
                    re-pinned in their own commit.";

/// A path from the repository root.
fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

/// FNV-1a 64 (offset `0xcbf2_9ce4_8422_2325`, prime `0x0100_0000_01b3`),
/// as `wiring_hash` in the library's `network.rs` tests.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// What the manifest keeps for one file in one mode.
enum Kept {
    /// Each output and its hash: `json`, or with `--freeze` `rs` and
    /// `trace`.
    Converts(Vec<(String, u64)>),
    /// The refusal's exit code.
    Refused(i32),
}

/// `tests/kept.tsv`, read: each file and mode with what it keeps, in the
/// rows' order. A row out of form panics, naming its line.
fn manifest() -> Vec<(String, String, Kept)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/kept.tsv");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some(HEADER), "kept.tsv's header row");
    let mut kept: Vec<(String, String, Kept)> = Vec::new();
    for (n, line) in (2..).zip(lines) {
        let [path, mode, output, value] = line.split('\t').collect::<Vec<_>>()[..] else {
            panic!("kept.tsv line {n}: four tab-separated fields: {line:?}");
        };
        assert!(
            MODES.contains(&mode),
            "kept.tsv line {n}: {path}: {mode:?} is no mode"
        );
        let at = kept.iter().position(|(p, m, _)| p == path && m == mode);
        if output == "refused" {
            let code = value
                .parse::<i32>()
                .ok()
                .filter(|&c| c != 0)
                .unwrap_or_else(|| {
                    panic!(
                        "kept.tsv line {n}: {path} {mode} refused, exit {value:?}: a refusal \
                         exits with a number other than 0"
                    )
                });
            assert!(
                at.is_none(),
                "kept.tsv line {n}: {path} {mode}, a second row"
            );
            kept.push((path.into(), mode.into(), Kept::Refused(code)));
            continue;
        }
        let outputs: &[&str] = if mode == "freeze" {
            &["rs", "trace"]
        } else {
            &["json"]
        };
        assert!(
            outputs.contains(&output),
            "kept.tsv line {n}: {path} {mode} {output:?}: {mode} gives no such output"
        );
        let hex = value.len() == 16
            && value
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        let hash = u64::from_str_radix(value, 16)
            .ok()
            .filter(|_| hex)
            .unwrap_or_else(|| {
                panic!(
                    "kept.tsv line {n}: {path} {mode} {output}: {value:?}, where a hash is 16 \
                     hex digits"
                )
            });
        match at {
            None => kept.push((
                path.into(),
                mode.into(),
                Kept::Converts(vec![(output.into(), hash)]),
            )),
            Some(i) => match &mut kept[i].2 {
                Kept::Converts(have) if !have.iter().any(|(o, _)| o == output) => {
                    have.push((output.into(), hash));
                }
                _ => panic!("kept.tsv line {n}: {path} {mode} {output}, a second row"),
            },
        }
    }
    for (path, mode, k) in &kept {
        if let Kept::Converts(have) = k {
            let names: Vec<&str> = have.iter().map(|(o, _)| o.as_str()).collect();
            let want: &[&str] = if mode == "freeze" {
                &["rs", "trace"]
            } else {
                &["json"]
            };
            assert_eq!(names, want, "kept.tsv: {path} {mode} keeps each output");
        }
    }
    kept
}

/// A `.nir`'s stem, as `stranger.sh` takes it (`basename "$nir" .nir`).
fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".nir").expect("a .nir path")
}

/// The run's folder under the target dir: one per file and mode.
fn folder(path: &str, mode: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("kept")
        .join(mode)
        .join(path)
}

/// One run of the binary on `path` in `mode`, into its own folder: the
/// exit code, `None` for a signal.
fn convert(path: &str, mode: &str) -> Option<i32> {
    let dir = folder(path, mode);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the run's folder");
    let stem = stem(path);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_gresil-nir2json"));
    if mode != "native" {
        cmd.arg("--sim-units");
    }
    if mode == "freeze" {
        cmd.arg("--freeze").arg(dir.join(format!("{stem}.rs")));
    }
    cmd.arg(repo(path))
        .arg(dir.join(format!("{stem}.json")))
        .output()
        .expect("the binary runs")
        .status
        .code()
}

/// An exit as a red line says it.
fn exit(code: Option<i32>) -> String {
    code.map_or("killed by a signal".into(), |c| format!("exit {c}"))
}

/// Every `.nir` under `dir`, whose path from the repository root is
/// `rel`, into `out`.
fn walk(dir: &Path, rel: &str, out: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let entry = entry.expect("an entry");
        let name = entry.file_name().into_string().expect("a UTF-8 name");
        let path = format!("{rel}/{name}");
        if entry.file_type().expect("its type").is_dir() {
            walk(&entry.path(), &path, out);
        } else if name.ends_with(".nir") {
            out.insert(path);
        }
    }
}

#[test]
fn every_kept_conversion_gives_its_kept_output() {
    let mut red = Vec::new();
    let mut moved = false;
    let mut frozen = Vec::new();
    for (path, mode, kept) in manifest() {
        let code = convert(&path, &mode);
        match kept {
            Kept::Refused(want) if code == Some(want) => {}
            Kept::Refused(want) if code == Some(0) => red.push(format!(
                "{path} {mode} refused: kept exit {want}, now converts. A refusal that starts \
                 to convert is not ruled yet: it is decided when it first happens."
            )),
            Kept::Refused(want) => {
                moved = true;
                red.push(format!(
                    "{path} {mode} refused: kept exit {want}, now {}",
                    exit(code)
                ));
            }
            Kept::Converts(outputs) if code == Some(0) => {
                for (output, want) in outputs {
                    let file = folder(&path, &mode).join(format!("{}.{output}", stem(&path)));
                    let got = fs::read(&file).map(|b| fnv1a(&b));
                    if got.as_ref().ok() != Some(&want) {
                        moved = true;
                        let now = got
                            .map_or_else(|e| format!("not written: {e}"), |h| format!("{h:016x}"));
                        red.push(format!(
                            "{path} {mode} {output}: kept {want:016x}, now {now}"
                        ));
                    }
                }
                if mode == "freeze" {
                    frozen.push(path);
                }
            }
            Kept::Converts(outputs) => {
                moved = true;
                for (output, want) in outputs {
                    red.push(format!(
                        "{path} {mode} {output}: kept {want:016x}, now refused, {}",
                        exit(code)
                    ));
                }
            }
        }
    }
    // A --freeze run's JSON is the --sim-units run's, byte for byte.
    for path in &frozen {
        let json = |mode| fs::read(folder(path, mode).join(format!("{}.json", stem(path))));
        if !matches!((json("freeze"), json("sim-units")), (Ok(a), Ok(b)) if a == b) {
            moved = true;
            red.push(format!(
                "{path} freeze json: not the --sim-units run's JSON byte for byte"
            ));
        }
    }
    let rule = if moved { RULE } else { "" };
    assert!(
        red.is_empty(),
        "{} kept conversion(s) red (tests/kept.rs, the module doc):\n  {}\n{rule}",
        red.len(),
        red.join("\n  ")
    );
}

#[test]
fn every_nir_of_the_three_folders_has_its_rows() {
    let mut on_disk = BTreeSet::new();
    for folder in FOLDERS {
        walk(&repo(folder), folder, &mut on_disk);
    }
    let kept = manifest();
    let mut red = Vec::new();
    for path in &on_disk {
        for mode in MODES {
            if !kept.iter().any(|(p, m, _)| p == path && m == mode) {
                red.push(format!("{path}: no {mode} row"));
            }
        }
    }
    for (path, mode, _) in &kept {
        if !on_disk.contains(path) {
            red.push(format!(
                "{path} {mode}: a row for no .nir of the three folders"
            ));
        }
    }
    assert!(
        red.is_empty(),
        "kept.tsv and the three folders part:\n  {}\nA .nir joins with its rows, one per \
         mode and output (tests/kept.rs, the module doc).",
        red.join("\n  ")
    );
}
