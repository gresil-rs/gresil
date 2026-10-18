#!/usr/bin/env bash
# tools/units.sh: the current is in pA, and no line spells it in µA.
#
# Round 65 renamed the current from µA to pA in the library's names and
# prose: with the resistance in MΩ, the step's `R·I/1000` is mV only when
# the current is in pA (`lif_neuron`'s module doc), so µA was off by
# 10^6. Every tracked text file is red on a line that writes the current
# in microamps: µA (U+00B5) or μA
# (U+03BC), `uA` standing alone or glued to a number, a number glued to
# `ua`, a name ending in `_ua` or `_UA`, or the word microamp. Outside
# these, which the record keeps as written:
#   - the closed bridge chapter, whose printed labels stay (GUARD 2):
#     crates/neuralos-rt/ and the `ternary_*` examples;
#   - paper/ (the paper track's), evidence/, ISA.md and
#     docs/RESEARCH_LOG.md;
#   - the crate README's version notes, its `## Since alpha.` sections,
#     frozen at publish;
#   - docs/releases/, the release notes, frozen at publish as those
#     version notes are; a draft names a published version's fields as
#     that version spelled them;
#   - three lines of that README, allowed by their exact text until
#     alpha.9's publish round changes them. An allowed line that is gone
#     is red too, so the commit that changes it drops it here;
#   - this file, which names what it looks for.
# These forms and no others: a word spelled apart (micro-amps, micro
# amps), a bare `ua` after a space and an escape (\u00b5A) pass, and a
# name that ends in `_UA` for another reason (USER_UA, a locale such as
# uk_UA) is red; such a line is reworded, or ruled a frozen place.
# Run from anywhere in the repo. Exit 1 on a hit, each printed as
# file:line:text; exit 2 when `git grep` itself fails, so a scan that
# read nothing is never green.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
export LC_ALL=C

mu=$'\xc2\xb5'      # U+00B5 MICRO SIGN
greek=$'\xce\xbc'   # U+03BC GREEK SMALL LETTER MU
pat="${mu}A|${greek}A|(^|[^[:alpha:]])uA([^[:alpha:]]|$)|[0-9]ua([^[:alpha:]]|$)|_ua([^[:alnum:]]|$)|_UA([^[:alnum:]]|$)|[Mm]icroamp"

readme=crates/gresil/README.md
allowed=(
  'microamps, u32 microseconds) — no floating point, no allocator in the'
  "One LIF neuron on the centi-mV grid, driven by a constant 160 ${greek}A at a"
  "~200 ${greek}A dead zone moves the membrane exactly zero — recorded, tested,"
)

set +e
hits=$(git grep --no-color -I -n -E -e "$pat" -- . \
  ':!:crates/neuralos-rt/' ':!:crates/gresil/examples/ternary_*' \
  ':!:paper/' ':!:evidence/' ':!:ISA.md' ':!:docs/RESEARCH_LOG.md' \
  ':!:docs/releases/' ":!:$readme" ':!:tools/units.sh')
rc=$?
set -e
if [ "$rc" -gt 1 ]; then
  echo "git grep failed (exit $rc): the scan read nothing" >&2
  exit 2
fi

declare -A seen=()
n=0
notes=0
while IFS= read -r line || [ -n "$line" ]; do
  n=$((n + 1))
  case $line in
    '## Since alpha.'*) notes=1 ;;
    '## '*) notes=0 ;;
  esac
  [ "$notes" = 1 ] && continue
  for a in "${allowed[@]}"; do
    if [ "$line" = "$a" ]; then
      seen[$a]=1
      continue 2
    fi
  done
  if [[ $line =~ $pat ]]; then
    hits+="${hits:+$'\n'}$readme:$n:$line"
  fi
done < "$readme"

gone=
for a in "${allowed[@]}"; do
  [ -n "${seen[$a]:-}" ] || gone+="${gone:+$'\n'}$readme: allowed line not found: $a"
done

if [ -n "$hits" ] || [ -n "$gone" ]; then
  [ -z "$hits" ] || { echo "the current written in microamps (pA is the unit):"; echo "$hits"; }
  [ -z "$gone" ] || { echo "an allowed line is gone; drop it from tools/units.sh:"; echo "$gone"; }
  exit 1
fi
echo "the current in pA: no other spelling in the tracked files outside the frozen places"
