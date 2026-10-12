#!/usr/bin/env bash
# tools/evidence-sums.sh: every checksum manifest under evidence/ verifies.
#
# evidence/ holds sha-pinned machine outputs, immutable in content
# (AGENTS.md § Session discipline), and the pin is a manifest beside the
# files: SHA256SUMS, nulls.SHA256SUMS or PINNED.sha256. Every tracked
# manifest runs `sha256sum -c --strict` from its own folder, so a pinned
# file edited, truncated or gone is red at the commit that does it.
# `--strict` makes a line `sha256sum` cannot parse red; the forms it
# reads pass, a comment and a blank line among them. The two manifests
# read by hand below take only `<64 hex>  <path>` rows.
#
# Four manifests are not written for their own folder. They stay as
# written, since no evidence file is edited for this check (ruled
# 2026-10-12), and the table below reads each one:
#   - nir-hdf5-gate/SHA256SUMS lists repo-root paths: run from the root;
#   - step5-calibration/generator/PINNED.sha256 names a commit on its
#     first line and pins two files at that commit, read with
#     `git show <commit>:<path>` (ruled 2026-09-24), so the clone must
#     hold that commit (CI's per-commit job fetches the whole history);
#   - nir-assembly-gate/SHA256SUMS lists bare names of fixtures that live
#     in the two crates' tests/nir_fixtures/: each row is read where its
#     file is, and a name found in neither folder or in both is red;
#   - step5-calibration/nulls.SHA256SUMS pins the 105 calibration nulls
#     tools/step5_calibration_burn.sh wrote to models/, which is
#     gitignored: no clone holds them, so it is skipped, and said.
# A table entry that names no tracked manifest is red, so a move cannot
# leave the table behind. A file no manifest lists is not read, and
# neither are shas kept as records in .txt files (r4's before and after
# lists of models/ files), which are not manifests. Run from anywhere in
# the repo; exit 1 on any failure.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
export LC_ALL=C

root_relative=evidence/nir-hdf5-gate/SHA256SUMS
at_commit=evidence/step5-calibration/generator/PINNED.sha256
fixtures=evidence/nir-assembly-gate/SHA256SUMS
fixture_dirs=(crates/neuralos-snn/tests/nir_fixtures crates/neuralos-rt/tests/nir_fixtures)
skipped=evidence/step5-calibration/nulls.SHA256SUMS

mapfile -t manifests < <(git ls-files -- evidence | grep -E '/(SHA256SUMS|[^/]+\.SHA256SUMS|[^/]+\.sha256)$')
fail=0
red() {
  echo "RED $*"
  fail=1
}

declare -A tracked=()
for m in "${manifests[@]}"; do
  tracked[$m]=1
done
for t in "$root_relative" "$at_commit" "$fixtures" "$skipped"; do
  [ -n "${tracked[$t]:-}" ] || red "$t: the table names it, git tracks no such manifest"
done

# One row of a hand-read manifest: "<64 hex>  <path>".
row() {
  [[ $1 =~ ^([0-9a-f]{64})\ \ ([^[:space:]].*)$ ]]
}

verified=0
for m in "${manifests[@]}"; do
  case $m in
    "$skipped")
      echo "skip $m: $(grep -c . "$m") rows, files in models/, never committed"
      continue
      ;;
    "$root_relative")
      sha256sum -c --strict --quiet "$m" || red "$m (from the repo root)"
      ;;
    "$at_commit")
      commit=$(sed -n '1s/^commit \([0-9a-f]\{40\}\)$/\1/p' "$m")
      if [ -z "$commit" ]; then
        red "$m: line 1 is not 'commit <40 hex>'"
      elif ! git cat-file -e "$commit^{commit}" 2>/dev/null; then
        red "$m: commit $commit is not in this clone (a shallow clone?)"
      else
        while IFS= read -r line || [ -n "$line" ]; do
          if ! row "$line"; then
            red "$m: not a row: $line"
          elif [ "$(git show "$commit:${BASH_REMATCH[2]}" | sha256sum | cut -d' ' -f1)" != "${BASH_REMATCH[1]}" ]; then
            red "$m: ${BASH_REMATCH[2]} at ${commit:0:7}"
          fi
        done < <(tail -n +2 "$m")
      fi
      ;;
    "$fixtures")
      while IFS= read -r line || [ -n "$line" ]; do
        if ! row "$line"; then
          red "$m: not a row: $line"
          continue
        fi
        want=${BASH_REMATCH[1]}
        name=${BASH_REMATCH[2]}
        found=()
        for d in "${fixture_dirs[@]}"; do
          [ ! -f "$d/$name" ] || found+=("$d/$name")
        done
        if [ "${#found[@]}" -ne 1 ]; then
          red "$m: $name is in ${#found[@]} of the two fixture folders"
        elif [ "$(sha256sum "${found[0]}" | cut -d' ' -f1)" != "$want" ]; then
          red "$m: ${found[0]}"
        fi
      done < "$m"
      ;;
    *)
      (cd "$(dirname "$m")" && sha256sum -c --strict --quiet "$(basename "$m")") || red "$m"
      ;;
  esac
  verified=$((verified + 1))
done

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "evidence: $verified of ${#manifests[@]} manifests verified, 1 skipped"
