// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! The value check against the C library: every group and dataset of
//! every `.nir` in the three folders `tests/kept.rs` names, as hdf5-pure,
//! the converter's reader, reads it, against what libhdf5, the C library,
//! read in the same file. `tests/libhdf5_reads.tsv` holds one row a group
//! and a dataset of each file, depth first, children in name order: the
//! kind, the dims, the value count and the FNV-1a 64 of the values' text,
//! one value a line, a float as the hex of its bits, an integer in
//! decimal, a string as the hex of its bytes, so no value of a file is
//! copied. `tools/gen_libhdf5_reads.py` wrote it through h5py, its first
//! line naming the h5py and libhdf5 versions; no test writes it, so the C
//! library runs when it is written, not at each run.
//!
//! Each dataset is read here by the call the converter makes for its
//! dtype: `read_f32`, `read_f64`, `read_i64`, and `read_vlen_strings`
//! with the default options. Any other dtype is `unread`, a dataset the
//! converter never reads. Before any read, a dataset past the converter's
//! size count, `MAX_VALUES` values or stored in chunks of more than that
//! many, is a `bound` row and is not read, as the converter refuses it
//! unread: two fixtures hold a weight of 2^20 by 2^20 values, never
//! written, which a whole read would ask 8 TiB for.
//!
//! A file hdf5-pure reads only in part is named in `REFUSED_READS` with
//! the text its failed reads give: each of its reads equals libhdf5's or
//! fails with that text, and one fails at least. A reader that reads it
//! whole is red here, as the file's refused rows are red in
//! `tests/kept.rs`, and the file leaves the list in the commit that
//! rewrites those rows.
//!
//! A red names the file, the path and both reads. And the three folders
//! hold no `.nir` without its rows, and the manifest no row without its
//! file, so a new fixture joins with its rows: run the script again.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use gresil_nir2json::MAX_VALUES;
use hdf5_pure::{DType, Dataset, File, Group, VlenStringReadOptions};

/// The folders that hold the tree's `.nir` files, from the repository
/// root: `tests/kept.rs`'s three.
const FOLDERS: [&str; 3] = [
    "crates/gresil-nir2json/tests/fixtures",
    "crates/neuralos-rt/tests/nir_fixtures",
    "evidence/nir-hdf5-gate",
];

/// The manifest's column header, its second line.
const HEADER: &str = "file\tpath\tkind\tdims\tvalues\tfnv1a64";

/// The files hdf5-pure reads only in part, each with the text its failed
/// reads give (the module doc): in HDF5 2.0's format the twin's chunked
/// datasets carry a data layout message of version 5.
const REFUSED_READS: [(&str, &str); 1] = [(
    "crates/gresil-nir2json/tests/fixtures/snnTorch_two_layer_hdf5_2_0.nir",
    "invalid data layout version: 5",
)];

/// One object's read: its kind, dims, value count and hash, `-` where the
/// kind has none.
type Read = [String; 4];

/// A path from the repository root.
fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

/// FNV-1a 64 (offset `0xcbf2_9ce4_8422_2325`, prime `0x0100_0000_01b3`),
/// as in `tests/kept.rs`.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A read of `kind` with no dims, count or hash: a group, or a dataset
/// the converter never reads.
fn bare(kind: &str) -> Read {
    [kind.into(), "-".into(), "-".into(), "-".into()]
}

/// What hdf5-pure fails on, as a read no manifest row equals.
fn failed(what: String) -> Read {
    ["error".into(), what, "-".into(), "-".into()]
}

/// `tests/libhdf5_reads.tsv`, read: each file's rows by path. A row out
/// of form panics, naming its line.
fn manifest() -> BTreeMap<String, BTreeMap<String, Read>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/libhdf5_reads.tsv");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines();
    let versions = lines.next().unwrap_or_default();
    assert!(
        versions.starts_with("# ") && versions.contains("h5py ") && versions.contains("libhdf5 "),
        "libhdf5_reads.tsv line 1: the h5py and libhdf5 versions that wrote it: {versions:?}"
    );
    assert_eq!(lines.next(), Some(HEADER), "libhdf5_reads.tsv's header row");
    let mut rows: BTreeMap<String, BTreeMap<String, Read>> = BTreeMap::new();
    for (n, line) in (3..).zip(lines) {
        let [file, path, kind, dims, values, hash] = line.split('\t').collect::<Vec<_>>()[..]
        else {
            panic!("libhdf5_reads.tsv line {n}: six tab-separated fields: {line:?}");
        };
        let read = [kind, dims, values, hash].map(String::from);
        let second = rows
            .entry(file.into())
            .or_default()
            .insert(path.into(), read);
        assert!(
            second.is_none(),
            "libhdf5_reads.tsv line {n}: {file} {path}, a second row"
        );
    }
    rows
}

/// The hex of `bytes`, two digits a byte.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// One dataset as hdf5-pure reads it with the converter's call for its
/// dtype; past the size count, a `bound` row and no read.
fn read(ds: &Dataset) -> Read {
    let kind = match ds.dtype() {
        Ok(DType::F32) => "f32",
        Ok(DType::F64) => "f64",
        Ok(DType::I64) => "i64",
        Ok(DType::VariableLengthString) => "vlenstr",
        Ok(_) => return bare("unread"),
        Err(e) => return failed(format!("dtype: {e}")),
    };
    let dims = match ds.shape() {
        Ok(dims) => dims,
        Err(e) => return failed(format!("shape: {e}")),
    };
    let shown = if dims.is_empty() {
        "scalar".to_string()
    } else {
        dims.iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    // the size count before any read: past it, nothing is read
    let values = |d: &[u64]| d.iter().try_fold(1u64, |n, &x| n.checked_mul(x));
    let past = |n: Option<u64>| n.is_none_or(|n| n > MAX_VALUES);
    let n = values(&dims);
    let chunk = ds.chunk_shape().ok().flatten();
    if past(n) || chunk.as_deref().is_some_and(|c| past(values(c))) {
        let n = n.map_or("past-u64".into(), |n| n.to_string());
        return ["bound".into(), shown, n, "-".into()];
    }
    let got: Result<Vec<String>, _> = match kind {
        "f32" => ds
            .read_f32()
            .map(|v| v.iter().map(|x| format!("{:08x}", x.to_bits())).collect()),
        "f64" => ds
            .read_f64()
            .map(|v| v.iter().map(|x| format!("{:016x}", x.to_bits())).collect()),
        "i64" => ds
            .read_i64()
            .map(|v| v.iter().map(i64::to_string).collect()),
        _ => ds
            .read_vlen_strings(VlenStringReadOptions::default())
            .map(|v| v.iter().map(|s| hex(s.as_bytes())).collect()),
    };
    match got {
        Ok(vals) => {
            let text: String = vals.iter().map(|v| format!("{v}\n")).collect();
            [
                kind.into(),
                shown,
                vals.len().to_string(),
                format!("{:016x}", fnv1a(text.as_bytes())),
            ]
        }
        Err(e) => failed(format!("{kind} read: {e}")),
    }
}

/// Every group and dataset under `g`, whose path is `base`, into `out`,
/// depth first, children in name order. A listing or a group that fails
/// is a read at `<path>/`, which no manifest row names.
fn reads(g: &Group, base: &str, out: &mut BTreeMap<String, Read>) {
    let join = |name: &str| {
        if base.is_empty() {
            name.to_string()
        } else {
            format!("{base}/{name}")
        }
    };
    let mut names: Vec<(String, bool)> = Vec::new();
    match g.datasets() {
        Ok(v) => names.extend(v.into_iter().map(|n| (n, false))),
        Err(e) => {
            out.insert(format!("{base}/"), failed(format!("datasets: {e}")));
        }
    }
    match g.groups() {
        Ok(v) => names.extend(v.into_iter().map(|n| (n, true))),
        Err(e) => {
            out.insert(format!("{base}/"), failed(format!("groups: {e}")));
        }
    }
    names.sort();
    for (name, group) in names {
        let path = join(&name);
        if !group {
            let read = g
                .dataset(&name)
                .map_or_else(|e| failed(format!("open: {e}")), |ds| read(&ds));
            out.insert(path, read);
            continue;
        }
        out.insert(path.clone(), bare("group"));
        match g.group(&name) {
            Ok(sub) => reads(&sub, &path, out),
            Err(e) => {
                out.insert(format!("{path}/"), failed(format!("open: {e}")));
            }
        }
    }
}

/// A read as a red line shows it.
fn show(r: &Read) -> String {
    r.join(" ")
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
fn hdf5_pure_reads_every_object_as_libhdf5_did() {
    let rows = manifest();
    let mut red = Vec::new();
    for (file, _) in REFUSED_READS {
        if !rows.contains_key(file) {
            red.push(format!("{file}: in REFUSED_READS, with no rows"));
        }
    }
    for (file, want) in rows {
        let path = repo(&file);
        if !path.exists() {
            continue; // the folders' test names it
        }
        let root = match File::open(&path) {
            Ok(f) => f.root(),
            Err(e) => {
                red.push(format!("{file}: hdf5-pure does not open it: {e}"));
                continue;
            }
        };
        let mut got = BTreeMap::new();
        reads(&root, "", &mut got);
        let why = REFUSED_READS
            .iter()
            .find(|(f, _)| *f == file)
            .map(|&(_, why)| why);
        for (at, w) in &want {
            match got.get(at) {
                Some(g) if g == w => {}
                Some(g) if why.is_some_and(|why| g[0] == "error" && g[1].contains(why)) => {}
                Some(g) => red.push(format!(
                    "{file} {at}: libhdf5 read {}, hdf5-pure reads {}",
                    show(w),
                    show(g)
                )),
                None => red.push(format!(
                    "{file} {at}: libhdf5 read {}, hdf5-pure lists no such object",
                    show(w)
                )),
            }
        }
        for (at, g) in &got {
            if !want.contains_key(at) {
                red.push(format!("{file} {at}: hdf5-pure reads {}, no row", show(g)));
            }
        }
        if why.is_some() && got.values().all(|g| g[0] != "error") {
            red.push(format!(
                "{file}: hdf5-pure reads it whole now; it leaves REFUSED_READS in the \
                 commit that rewrites its refused rows of kept.tsv"
            ));
        }
    }
    assert!(
        red.is_empty(),
        "{} read(s) red (tests/libhdf5_reads.rs, the module doc):\n  {}",
        red.len(),
        red.join("\n  ")
    );
}

#[test]
fn every_nir_of_the_three_folders_has_its_reads() {
    let mut on_disk = BTreeSet::new();
    for folder in FOLDERS {
        walk(&repo(folder), folder, &mut on_disk);
    }
    let rows = manifest();
    let mut red = Vec::new();
    for path in &on_disk {
        if !rows.contains_key(path) {
            red.push(format!("{path}: no rows"));
        }
    }
    for path in rows.keys() {
        if !on_disk.contains(path) {
            red.push(format!("{path}: rows for no .nir of the three folders"));
        }
    }
    assert!(
        red.is_empty(),
        "libhdf5_reads.tsv and the three folders part:\n  {}\nA .nir joins with its rows: run \
         tools/gen_libhdf5_reads.py again (tests/libhdf5_reads.rs, the module doc).",
        red.join("\n  ")
    );
}
