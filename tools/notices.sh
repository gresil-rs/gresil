#!/usr/bin/env bash
# tools/notices.sh <manifest> <package> <target> <out> [--nir] [--rev <tag>]
#
# The third-party notices of one release binary, written from its
# lockfile and its toolchain, never by hand (ISA round 76). The two
# artifact scripts call it beside their binary: firmware/esp32c3/build.sh
# for the ELF, with --nir, and crates/gresil-nir2json/build.sh for the
# converter. <package> is the binary's package in <manifest>'s
# workspace, <target> the target it is built for, <out> the file
# written; the standard library's notices, the toolchain's own file, are
# copied beside <out> as rust-<version>-COPYRIGHT-library.html. A
# notices file is derived and rewritten at each build; a release's copy
# is pinned by its notes' § Files.
#
# THE TREE. cargo and rustc run from <manifest>'s folder, so rustup takes
# that tree's pin: a `git archive` of a past tag gets its own toolchain,
# sysroot and texts. <out>'s header names the tree by its tag or its
# commit, from git when a git work tree tracks <manifest>, else by --rev
# (an archive, inside a work tree or not); never by a folder. COPYRIGHT,
# LICENSE-NIR, the table, the three C texts and the gate's pattern are
# this script's own tree's, whatever tree it describes.
#
# <out>, in parts, in this order, each numbered as present: a file
# without --nir has no part 3, one for a target other than *-musl no
# part 5, and the parts after a missing one move up. Every text, in
# parts 2, 3 and 5 of the list below, is headed
# `Text <i> of <T>, sha256 <sha>`, <i> counted through the file.
#   1. COPYRIGHT, this repository's notice.
#   2. The crates linked into the binary: `cargo tree -e
#      normal,no-proc-macro` on <target>. Path packages are left to part
#      1, each under AGPL-3.0-or-later, the license COPYRIGHT gives, or
#      the script refuses. A crate's text, in this order: its package's
#      own license files (LICEN[CS]E*, COPYING*, NOTICE*, COPYRIGHT*,
#      UNLICENSE*, at the package's top), all of them; with none, an
#      expression that offers Apache-2.0 on its own takes the toolchain's
#      licenses/Apache-2.0.txt; else its row in tools/notices/table.
#      Each distinct text once, by sha256, with the crates that carry it.
#   3. With --nir, NIR's BSD 3-Clause text, LICENSE-NIR, for the
#      two_lif_neurons graph the binary compiles from
#      crates/gresil/tests/traces/frozen.rs. The premise is checked: that
#      file of this tree holds `pub mod two_lif_neurons {`, the module
#      COPYRIGHT names, or the script refuses, so a rename or a removal
#      stops the build until COPYRIGHT, this check and the flag follow.
#   4. The standard library: `rustc --version`, and the toolchain's file.
#   5. For a *-musl target, the C runtime the toolchain links in
#      statically: musl's COPYRIGHT, with the version libc.a carries,
#      then LLVM's LICENSE.TXT for libunwind and for compiler-rt's
#      crtbegin and crtend objects, with the LLVM version `rustc -vV`
#      prints.
#   6. The crates the build runs and does not link, what `-e
#      normal,build` reaches less part 2: name, version, expression.
# Then one line, `notices: <L> linked, <B> build-time, <T> texts, sha256
# <sha>  <out>`: L and B count the third-party crates of the list's
# parts 2 and 6, T the texts parts 2, 3 and 5 carry; <out> as given,
# $HOME shown as ~.
#
# THE C TEXTS. Part 5's three texts are committed in tools/notices/
# without a version in their names: each was fetched at the tags of
# both pins' versions (1.92.0: musl 1.2.3, LLVM 21.1.3; 1.98.1: musl
# 1.2.5, LLVM 22.1.8) and is byte-equal at both:
#   musl-COPYRIGHT           musl v1.2.3 and v1.2.5,
#     https://git.musl-libc.org/cgit/musl/plain/COPYRIGHT?h=<tag>
#   libunwind-LICENSE.TXT    llvmorg-21.1.3 and llvmorg-22.1.8,
#     https://raw.githubusercontent.com/llvm/llvm-project/<tag>/libunwind/LICENSE.TXT
#   compiler-rt-LICENSE.TXT  the same two tags,
#     https://raw.githubusercontent.com/llvm/llvm-project/<tag>/compiler-rt/LICENSE.TXT
# Their sha256 and the musl and LLVM versions the guards accept are set
# below, one home. A libc.a whose musl version is not listed refuses,
# and so does a rustc whose LLVM version is not, until the new version's
# texts are compared and the list grows.
#
# THE GATE. <out> is a release asset, and no hook reads one (the build
# is the guard, tools/remap.sh). Before its line, the script refuses its
# own <out> on a hit of this tree's .githooks/personal-pattern, or on
# any word that starts with / or ~/ (a URL passes): it writes names,
# versions, license expressions and file names, never a path, neither
# the registry's, the sysroot's, the manifest's nor an archive's.
#
# Exit: 0 written. 1 refused, naming the cause: a linked crate nothing
# covers, a crate with no source folder, a row or a committed text whose
# sha differs, a musl or LLVM version not listed, --nir without its
# module, a path package under another license, a source other than
# crates.io and path packages, a gate hit. 2 usage (--rev where git
# names the tree, none where it does not), or cargo tree failing.
set -euo pipefail
export LC_ALL=C

musl_versions="1.2.3 1.2.5"
llvm_versions="21.1.3 22.1.8"
musl_sha=f9bc4423732350eb0b3f7ed7e91d530298476f8fec0c6c427a1c04ade22655af
libunwind_sha=b5efebcaca80879234098e52d1725e6d9eb8fb96a19fce625d39184b705f7b6d
compiler_rt_sha=1a8f1058753f1ba890de984e48f0242a3a5c29a6a8f2ed9fd813f36985387e8d

usage() {
  echo "usage: $0 <manifest> <package> <target> <out> [--nir] [--rev <tag>]" >&2
  exit 2
}
die() {
  echo "notices.sh: $*" >&2
  exit 1
}

[ $# -ge 4 ] || usage
manifest=$1 package=$2 target=$3 out=$4
shift 4
nir=0 rev=
while [ $# -gt 0 ]; do
  case $1 in
    --nir) nir=1; shift ;;
    --rev) [ $# -ge 2 ] || usage; rev=$2; shift 2 ;;
    *) usage ;;
  esac
done
if [ ! -f "$manifest" ] || [ ! -d "$(dirname "$out")" ]; then
  echo "notices.sh: <manifest> must be a file and <out>'s folder must exist" >&2
  usage
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
mdir=$(cd "$(dirname "$manifest")" && pwd)
mfile=$mdir/$(basename "$manifest")
odir=$(cd "$(dirname "$out")" && pwd)
ofile=$odir/$(basename "$out")
shown=$out
if [ -n "${HOME:-}" ]; then
  case $out in "$HOME"/*) shown="~/${out#"$HOME"/}" ;; esac
fi
reg=${CARGO_HOME:-${HOME:-}/.cargo}/registry/src
table=$root/tools/notices/table
frozen=$root/crates/gresil/tests/traces/frozen.rs
license_nir=$root/crates/gresil-nir2json/tests/fixtures/community/LICENSE-NIR

# A refused run leaves no file under <out>'s name: a stale notices file
# beside a new binary would read as its notices.
rm -f "$ofile"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

sha() { sha256sum "$1" | cut -d' ' -f1; }

if [ "$nir" = 1 ]; then
  grep -q -x 'pub mod two_lif_neurons {' "$frozen" \
    || die "--nir: crates/gresil/tests/traces/frozen.rs holds no line 'pub mod two_lif_neurons {', the module COPYRIGHT names as made from NIR's file; COPYRIGHT, this check and the flag follow a rename together"
  [ -f "$license_nir" ] || die "--nir: crates/gresil-nir2json/tests/fixtures/community/LICENSE-NIR is missing"
fi

cd "$mdir"

# The tree's name: a tag or a commit, never a folder. git names it only
# when it tracks <manifest>: an archive extracted inside a work tree,
# under its target/ say, is no commit of that work tree.
if git rev-parse --is-inside-work-tree >/dev/null 2>&1 \
  && git ls-files --error-unmatch -- "${mfile##*/}" >/dev/null 2>&1; then
  if [ -n "$rev" ]; then
    echo "notices.sh: --rev names an archive's tree; here git names it" >&2
    exit 2
  fi
  if tag=$(git describe --tags --exact-match HEAD 2>/dev/null); then
    tree=$tag
  else
    tree="commit $(git rev-parse --short=7 HEAD)"
  fi
  [ -z "$(git status --porcelain --untracked-files=no)" ] || tree="$tree, with changes not committed"
elif [ -n "$rev" ]; then
  tree=$rev
else
  echo "notices.sh: no git work tree tracks <manifest>: name its tag or commit with --rev" >&2
  exit 2
fi

vv=$(rustc -vV)
release=$(sed -n 's/^release: //p' <<<"$vv")
llvm=$(sed -n 's/^LLVM version: //p' <<<"$vv")
rustc_version=$(rustc --version)
sysroot=$(rustc --print sysroot)
docs=$sysroot/share/doc/rust
html=rust-$release-COPYRIGHT-library.html
[ -f "$docs/COPYRIGHT-library.html" ] || die "rustc $release ships no share/doc/rust/COPYRIGHT-library.html"

# cargo tree, twice; RUSTC_WRAPPER unset, so a listing starts no
# compiler server, and no color: CARGO_TERM_COLOR=always (CI sets
# it) wraps the `(*)` mark in escapes that split() does not strip.
listing() {
  env -u RUSTC_WRAPPER cargo tree --manifest-path "$mfile" -p "$package" \
    --target "$target" --locked --offline -e "$1" --prefix none \
    --color never --format '{p}|{l}' >"$2" 2>"$tmp/tree.err" || {
    echo "notices.sh: cargo tree -e $1 failed:" >&2
    cat "$tmp/tree.err" >&2
    exit 2
  }
}
listing normal,no-proc-macro "$tmp/linked.raw"
listing normal,build "$tmp/all.raw"

# One line a package: name, version, expression, tab-separated, registry
# crates only. Path packages are this repository's; anything else
# refuses.
self_version=
split() {
  local line p l name rest ver src
  sed 's/ (\*)$//' "$1" | sort -u >"$tmp/sorted"
  : >"$2"
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    p=${line%%|*} l=${line#*|}
    p=${p% (proc-macro)}
    name=${p%% *} rest=${p#* }
    ver=${rest%% *} ver=${ver#v}
    src=
    [ "$rest" = "${rest#* }" ] || src=${rest#* }
    case $src in
      '')
        printf '%s\t%s\t%s\n' "$name" "$ver" "$l" >>"$2" ;;
      '(/'*')')
        [ "$l" = AGPL-3.0-or-later ] \
          || die "$name $ver: a path package under '$l'; part 1 covers this repository's own, AGPL-3.0-or-later"
        [ "$name" != "$package" ] || self_version=$ver ;;
      *)
        die "$name $ver: a source the script does not read; crates.io's and this repository's path packages only" ;;
    esac
  done <"$tmp/sorted"
  sort -o "$2" "$2"
}
split "$tmp/linked.raw" "$tmp/linked"
split "$tmp/all.raw" "$tmp/all"
comm -13 "$tmp/linked" "$tmp/all" >"$tmp/build"
[ -n "$self_version" ] || die "$package is not a path package of <manifest>'s workspace"
n_linked=$(grep -c . "$tmp/linked" || true)
n_build=$(grep -c . "$tmp/build" || true)

# The table: name, version, source, sha256, holder (optional).
declare -A row_src row_sha row_holder
n=0
while IFS= read -r line || [ -n "$line" ]; do
  n=$((n + 1))
  case $line in '' | '#'*) continue ;; esac
  IFS=$'\t' read -r t_name t_ver t_src t_sha t_holder t_more <<<"$line"
  if [ -z "$t_sha" ] || [ -n "$t_more" ] || [ ${#t_sha} -ne 64 ]; then
    die "tools/notices/table:$n: not a row (name, version, source, sha256, holder, tab-separated)"
  fi
  k=$t_name@$t_ver
  row_src[$k]=$t_src
  row_sha[$k]=$t_sha
  row_holder[$k]=$t_holder
done <"$table"

offers_apache() {
  local e
  case $1 in *'('* | *')'* | *' AND '* | *' WITH '*) return 1 ;; esac
  e=$(printf '%s' "$1" | sed 's#/# OR #g' | tr -s ' ')
  [[ " OR $e OR " == *" OR Apache-2.0 OR "* ]]
}

# Part 2's texts: one file each under $tmp/text/, keyed in the order of
# their first crate, with the license text's sha and the carriers.
mkdir -p "$tmp/text"
: >"$tmp/keys"
carry() {  # key, the text's file, the carrier line, the license text's sha
  if [ ! -f "$tmp/text/$1" ]; then
    cp "$2" "$tmp/text/$1"
    printf '%s\n' "$4" >"$tmp/text/$1.sha"
    printf '%s\n' "$1" >>"$tmp/keys"
  fi
  printf '  %s\n' "$3" >>"$tmp/text/$1.by"
}
while IFS=$'\t' read -r name ver expr; do
  dir=
  for d in "$reg"/*/"$name-$ver"; do
    if [ -d "$d" ]; then dir=$d; break; fi
  done
  [ -n "$dir" ] || die "$name $ver: no source folder in the cargo registry; the build extracts it, so build first"
  files=()
  for f in "$dir"/*; do
    base=${f##*/}
    case ${base,,} in
      licen[cs]e* | copying* | notice* | copyright* | unlicense*)
        [ -f "$f" ] || die "$name $ver: $base is no file; the script reads license files, not folders"
        files+=("$base") ;;
    esac
  done
  k=$name@$ver
  if [ ${#files[@]} -gt 0 ]; then
    for base in "${files[@]}"; do
      got=$(sha "$dir/$base")
      carry "$got" "$dir/$base" "$name $ver, $base" "$got"
    done
  elif offers_apache "$expr"; then
    [ -f "$docs/licenses/Apache-2.0.txt" ] || die "rustc $release ships no licenses/Apache-2.0.txt"
    got=$(sha "$docs/licenses/Apache-2.0.txt")
    carry "$got" "$docs/licenses/Apache-2.0.txt" "$name $ver, Apache-2.0, no license file" "$got"
  elif [ -n "${row_src[$k]:-}" ]; then
    src=${row_src[$k]} want=${row_sha[$k]} holder=${row_holder[$k]}
    case $src in
      toolchain:*)
        file=${src#toolchain:} what="the toolchain's ${src#toolchain:}"
        [ -f "$docs/licenses/$file" ] || die "tools/notices/table: $name $ver: rustc $release ships no licenses/$file"
        cp "$docs/licenses/$file" "$tmp/cut" ;;
      *:*)
        file=${src%%:*} lines=${src#*:}
        [[ $lines =~ ^[0-9]+-[0-9]+$ ]] || die "tools/notices/table: $name $ver: '$lines' is no <first>-<last> line range"
        [ -f "$dir/$file" ] || die "tools/notices/table: $name $ver: its package has no $file"
        what="$file lines ${lines%-*} to ${lines#*-}"
        sed -n "${lines%-*},${lines#*-}p" "$dir/$file" >"$tmp/cut" ;;
      *) die "tools/notices/table: $name $ver: a source neither <file>:<first>-<last> nor toolchain:<file>" ;;
    esac
    got=$(sha "$tmp/cut")
    [ "$got" = "$want" ] || die "tools/notices/table: $name $ver: $what has sha256 $got, the row says $want"
    if [ -n "$holder" ]; then
      {
        printf '%s\n\n' "$holder"
        cat "$tmp/cut"
        printf '\n%s %s carries no copyright notice of its own; the holder\n' "$name" "$ver"
        printf 'named above is the author its manifest gives.\n'
      } >"$tmp/composed"
      carry "row-$name-$ver" "$tmp/composed" "$name $ver, $what under its author's name, no license file" "$got"
    else
      carry "$got" "$tmp/cut" "$name $ver, $what, no license file" "$got"
    fi
  else
    die "$name $ver ($expr): no license file in its package, no Apache-2.0 in its expression, no row in tools/notices/table"
  fi
done <"$tmp/linked"
n_texts=$(grep -c . "$tmp/keys" || true)

musl=
case $target in *-musl)
  libc=$sysroot/lib/rustlib/$target/lib/self-contained/libc.a
  [ -f "$libc" ] || die "rustc $release has no self-contained libc.a for $target"
  # musl's version is the one string in libc.a that is a bare x.y.z.
  musl=$(tr '\0' '\n' <"$libc" | { grep -a -x -E '[0-9]+\.[0-9]+\.[0-9]+' || true; } | sort -u)
  [ "$(printf '%s\n' "$musl" | grep -c .)" = 1 ] \
    || die "libc.a of rustc $release carries $(printf '%s\n' "$musl" | grep -c .) version strings, not one"
  case " $musl_versions " in *" $musl "*) ;; *)
    die "libc.a of rustc $release is musl $musl, which this script's header does not list ($musl_versions): compare its COPYRIGHT with tools/notices/musl-COPYRIGHT, then list it" ;;
  esac
  case " $llvm_versions " in *" $llvm "*) ;; *)
    die "rustc $release is LLVM $llvm, which this script's header does not list ($llvm_versions): compare libunwind's and compiler-rt's LICENSE.TXT at llvmorg-$llvm with tools/notices/, then list it" ;;
  esac
  for pair in "musl-COPYRIGHT $musl_sha" "libunwind-LICENSE.TXT $libunwind_sha" \
    "compiler-rt-LICENSE.TXT $compiler_rt_sha"; do
    got=$(sha "$root/tools/notices/${pair% *}")
    [ "$got" = "${pair#* }" ] || die "tools/notices/${pair% *} has sha256 $got, the header says ${pair#* }"
  done
  n_texts=$((n_texts + 3))
  ;;
esac
[ "$nir" = 0 ] || n_texts=$((n_texts + 1))

rule() { printf '%s\n' '========================================================================'; }
thin() { printf '%s\n' '------------------------------------------------------------------------'; }
para() {  # one paragraph of prose, wrapped at 72
  printf '%s\n' "$*" | fold -s -w 72 | sed 's/ *$//'
}
text() {  # a text, with a newline at its end
  cat "$1"
  [ -z "$(tail -c 1 "$1")" ] || printf '\n'
}
part=0
heading() {
  part=$((part + 1))
  printf '\n'
  rule
  printf '%s. %s\n' "$part" "$1"
  rule
  printf '\n'
}
list() {  # the crates of a list, with their expressions
  while IFS=$'\t' read -r name ver expr; do
    printf '  %s %s: %s\n' "$name" "$ver" "${expr:-(no license expression)}"
  done <"$1"
}

{
  printf 'Third-party notices: %s %s, %s\n\n' "$package" "$self_version" "$target"
  para "The binary $package $self_version, built for $target with rustc" \
    "$release, from the gresil repository at $tree. tools/notices.sh" \
    "writes this file from that tree's Cargo.lock and toolchain, never by" \
    "hand: $n_linked third-party crates linked into the binary, $n_build" \
    "that its build runs without linking them, $n_texts license texts."

  heading "This repository's notice, COPYRIGHT"
  text "$root/COPYRIGHT"

  heading "The crates linked into the binary"
  para "$n_linked crates, by name. A crate's text is its package's license" \
    "files, all of them. A package with none whose expression offers" \
    "Apache-2.0 carries the Apache License 2.0 as rustc $release ships it" \
    "(licenses/Apache-2.0.txt); the others name their text in their line" \
    "below. Each text once, with the crates that carry it."
  printf '\n'
  list "$tmp/linked"
  i=0
  while IFS= read -r key; do
    i=$((i + 1))
    printf '\n'
    thin
    printf 'Text %s of %s, sha256 %s\n' "$i" "$n_texts" "$(cat "$tmp/text/$key.sha")"
    printf 'Carried by:\n'
    cat "$tmp/text/$key.by"
    thin
    printf '\n'
    text "$tmp/text/$key"
  done <"$tmp/keys"

  if [ "$nir" = 1 ]; then
    heading "NIR's license, for the graph the binary compiles"
    para "The binary compiles the two_lif_neurons module of" \
      "crates/gresil/tests/traces/frozen.rs, frozen from a conversion of a" \
      "file of the NIR repository (https://github.com/neuromorphs/NIR)," \
      "under that repository's BSD 3-Clause license, LICENSE-NIR."
    i=$((i + 1))
    printf '\n'
    thin
    printf 'Text %s of %s, sha256 %s\n' "$i" "$n_texts" "$(sha "$license_nir")"
    printf 'NIR: LICENSE-NIR, BSD 3-Clause\n'
    thin
    printf '\n'
    text "$license_nir"
  fi

  heading "The Rust standard library"
  printf '%s\n\n' "$rustc_version"
  para "The binary links crates of the Rust standard library, as rustc" \
    "$release ships them for $target. Their notices are the toolchain's" \
    "own file, $html, which ships beside this one as the toolchain" \
    "installs it, byte for byte."

  if [ -n "$musl" ]; then
    heading "The C runtime the toolchain links in"
    para "For $target, rustc $release links into the static binary code of" \
      "three C runtime libraries it carries itself: musl $musl, the C" \
      "library and its start files; LLVM $llvm's libunwind, the unwinder;" \
      "and LLVM $llvm's compiler-rt, its crtbegin and crtend objects."
    for t in "musl $musl: COPYRIGHT|musl-COPYRIGHT" \
      "libunwind, LLVM $llvm: LICENSE.TXT|libunwind-LICENSE.TXT" \
      "compiler-rt, LLVM $llvm: LICENSE.TXT, for crtbegin and crtend|compiler-rt-LICENSE.TXT"; do
      i=$((i + 1))
      printf '\n'
      thin
      printf 'Text %s of %s, sha256 %s\n' "$i" "$n_texts" "$(sha "$root/tools/notices/${t#*|}")"
      printf '%s\n' "${t%|*}"
      thin
      printf '\n'
      text "$root/tools/notices/${t#*|}"
    done
  fi

  heading "The crates the build runs and does not link"
  if [ "$n_build" = 0 ]; then
    para "None: every crate the build reaches is linked into the binary."
  else
    para "$n_build crates run while the binary is built, as build scripts," \
      "procedural macros and their dependencies; the binary does not link" \
      "them, so their texts are not reproduced. Name, version and license" \
      "expression:"
    printf '\n'
    list "$tmp/build"
  fi
} >"$tmp/out"

# The gate (the header): the personal pattern, then any word that is a
# path. A scan that did not run is not a pass.
pattern=$(cat "$root/.githooks/personal-pattern" 2>/dev/null || true)
[ -n "$pattern" ] || die ".githooks/personal-pattern is missing or empty; refusing rather than scanning with an empty pattern"
for test in pattern path; do
  rc=0
  if [ "$test" = pattern ]; then
    hits=$(grep -a -n -E -i -e "$pattern" "$tmp/out") || rc=$?
  else
    hits=$(grep -a -n -E '(^|[[:space:](<"=,;:])(/|~/)[A-Za-z0-9._-]' "$tmp/out") || rc=$?
  fi
  [ "$rc" -le 1 ] || die "the gate's $test grep failed (exit $rc); a scan that did not run is not a pass"
  if [ -n "$hits" ]; then
    echo "notices.sh: REFUSED, $(printf '%s\n' "$hits" | grep -c .) line(s) of $shown hit the $test test:" >&2
    if [ -n "${HOME:-}" ]; then
      printf '%s\n' "$hits" | sed "s#$HOME#<HOME>#g" >&2
    else
      printf '%s\n' "$hits" >&2
    fi
    exit 1
  fi
done

cp "$docs/COPYRIGHT-library.html" "$odir/$html"
mv "$tmp/out" "$ofile"
echo "notices: $n_linked linked, $n_build build-time, $n_texts texts, sha256 $(sha "$ofile")  $shown"
