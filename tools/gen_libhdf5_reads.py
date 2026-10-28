#!/usr/bin/env python3
"""Write the libhdf5 reads of record (deterministic): what libhdf5, the C
library, reads in every .nir of the converter's three fixture folders,
hashed, for tests/libhdf5_reads.rs to compare with hdf5-pure's reads.

Inputs : every .nir under crates/gresil-nir2json/tests/fixtures/,
         crates/neuralos-rt/tests/nir_fixtures/ and evidence/nir-hdf5-gate/
         (tests/kept.rs's three folders), read through h5py.
Output : crates/gresil-nir2json/tests/libhdf5_reads.tsv, beside kept.tsv.
         Its first line names the h5py and libhdf5 versions that wrote it,
         its second the columns; then one row a group and a dataset of
         each file, depth first, children in name order:
         - a group: `group`;
         - a dataset of float32, float64, int64 or variable-length strings,
           the converter's four reads: its kind (`f32`, `f64`, `i64`,
           `vlenstr`), its dims (`scalar` for none), its value count and
           the FNV-1a 64 of its values' text, one value a line, "\\n" after
           each: a float as the hex of its bits, an integer in decimal, a
           string as the hex of its bytes;
         - a dataset past the converter's size count, MAX_VALUES values, or
           stored in chunks of more than MAX_VALUES values: `bound`, its
           dims and value count, and nothing read, as the converter
           refuses it unread;
         - any other dataset: `unread`; a link that is not a hard link:
           `link`.
         Hashes only: no value of a file is copied.
Run    : .nirenv/bin/python3 tools/gen_libhdf5_reads.py

A new fixture joins with its rows: run this again, and the test refuses
a .nir of the three folders without rows, and a row without its file.
"""
import pathlib

import h5py
import numpy as np

ROOT = pathlib.Path(__file__).resolve().parent.parent
FOLDERS = (
    "crates/gresil-nir2json/tests/fixtures",
    "crates/neuralos-rt/tests/nir_fixtures",
    "evidence/nir-hdf5-gate",
)
OUT = ROOT / "crates/gresil-nir2json/tests/libhdf5_reads.tsv"
BOUND = 1 << 22  # the converter's MAX_VALUES, its size count
OFFSET, PRIME, MASK = 0xCBF29CE484222325, 0x100000001B3, (1 << 64) - 1


def fnv1a(data: bytes) -> int:
    h = OFFSET
    for b in data:
        h = ((h ^ b) * PRIME) & MASK
    return h


def kind_of(dt) -> str | None:
    """The converter's read for a dtype: `f32`, `f64`, `i64`, `vlenstr`."""
    info = h5py.check_string_dtype(dt)
    if info is not None:
        return "vlenstr" if info.length is None else None
    if dt.kind == "f" and dt.itemsize in (4, 8):
        return f"f{8 * dt.itemsize}"
    if dt.kind == "i" and dt.itemsize == 8:
        return "i64"
    return None


def cells(raw, kind: str) -> list:
    """Each value's text, in the dataset's order."""
    if kind == "vlenstr":
        return [(x if isinstance(x, bytes) else str(x).encode()).hex()
                for x in np.asarray(raw, dtype=object).reshape(-1)]
    flat = np.asarray(raw).reshape(-1)
    if kind == "f32":
        return [f"{b:08x}" for b in flat.astype("<f4").view("<u4").tolist()]
    if kind == "f64":
        return [f"{b:016x}" for b in flat.astype("<f8").view("<u8").tolist()]
    return [str(int(x)) for x in flat.tolist()]


def rows_of(group: h5py.Group, base: str, rows: list) -> None:
    for name in sorted(group):
        path = f"{base}/{name}" if base else name
        if not isinstance(group.get(name, getlink=True), h5py.HardLink):
            rows.append((path, "link", "-", "-", "-"))
            continue
        obj = group[name]
        if isinstance(obj, h5py.Group):
            rows.append((path, "group", "-", "-", "-"))
            rows_of(obj, path, rows)
            continue
        kind = kind_of(obj.dtype) if isinstance(obj, h5py.Dataset) else None
        if kind is None or obj.shape is None:
            rows.append((path, "unread", "-", "-", "-"))
            continue
        dims = ",".join(str(d) for d in obj.shape) or "scalar"
        # the size count before any read: past it, nothing is read
        n = int(np.prod(obj.shape, dtype=object)) if obj.shape else 1
        chunk = int(np.prod(obj.chunks, dtype=object)) if obj.chunks else 0
        if n > BOUND or chunk > BOUND:
            rows.append((path, "bound", dims, str(n) if n <= MASK else "past-u64", "-"))
            continue
        vals = cells(obj[()], kind)
        text = "".join(v + "\n" for v in vals).encode()
        rows.append((path, kind, dims, str(len(vals)), f"{fnv1a(text):016x}"))


def main() -> None:
    files = sorted(p.relative_to(ROOT).as_posix() for d in FOLDERS for p in (ROOT / d).rglob("*.nir"))
    lines = [
        f"# written by tools/gen_libhdf5_reads.py, h5py {h5py.version.version}, "
        f"libhdf5 {h5py.version.hdf5_version}",
        "file\tpath\tkind\tdims\tvalues\tfnv1a64",
    ]
    for f in files:
        rows = []
        with h5py.File(ROOT / f, "r") as h:
            rows_of(h, "", rows)
        lines += ["\t".join((f,) + r) for r in rows]
    OUT.write_text("".join(line + "\n" for line in lines))
    print(f"reads  : {OUT.name} ({len(files)} files, {len(lines) - 2} rows)")


if __name__ == "__main__":
    main()
