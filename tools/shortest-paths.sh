#!/usr/bin/env bash
# tools/shortest-paths.sh: no doc or example takes a root name, or a
# variant of one, through its module.
#
# The crate README says, § Modules, that every example in the crate's
# docs imports each name by its shortest public path. The root
# re-exports a few names, read here from the `pub use` statements of
# crates/gresil/src/lib.rs, so a new re-export extends the check.
# Each root name has two public paths, and so has what a `use` reaches
# through one, a root enum's variant. The check assumes no other name
# has two, which holds while the crate's other `pub use` statements
# re-export from private modules. So the sentence breaks where a path
# runs through a root name's module: `gresil::nir::NirImport` for
# `gresil::NirImport`, or
# `gresil::lif_neuron::NeuronType::Excitatory` for
# `gresil::NeuronType::Excitatory`.
# Read: each `use gresil::…;` statement, its lines joined and its
# groups flattened, in
#   - the doc lines of crates/gresil/src (`///` and `//!`, the
#     hidden `# ` lines too), where the crate doc's examples are;
#   - the crate README, which is the crate doc;
#   - crates/gresil/examples, the programs the package ships, their
#     doc lines too.
# Not read: tests/, even a file an example compiles from it (`trace`
# takes tests/traces/cases.rs by `#[path]`), the other crates, a path
# outside a `use` statement, and a glob import (`use …::*;`), which
# names nothing. Not seen: a `use` after another statement or an
# attribute on its line (`#[allow(…)] use …;`), a crate alias
# (`use gresil as snn;`), and a block doc comment (`/** … */`).
# Run from anywhere in the repo. Exit 1 on a hit, each printed as
# file:line:text with the root path; exit 2 when `git grep` fails,
# lib.rs gives no root name, re-exports a glob or names a path that is
# not identifiers, a statement has no `;`, or a place holds no
# statement, so a scan that read nothing is never green.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
export LC_ALL=C

crate=crates/gresil
lib=$crate/src/lib.rs

# trim TEXT: TEXT without its leading and trailing blanks, in $trimmed.
trim() {
  trimmed=$1
  trimmed=${trimmed#"${trimmed%%[![:space:]]*}"}
  trimmed=${trimmed%"${trimmed##*[![:space:]]}"}
}

# flatten PREFIX TREE: each path the use tree TREE names under PREFIX,
# appended to the array `paths`. `self` names PREFIX, `X as Y` names X,
# and a glob names nothing.
flatten() {
  local prefix=$1 tree inner part ch depth i
  trim "$2"
  tree=$trimmed
  if [[ $tree == '{'*'}' ]]; then
    inner=${tree:1:${#tree}-2}
    part=
    depth=0
    for ((i = 0; i < ${#inner}; i++)); do
      ch=${inner:i:1}
      case $ch in
        '{') depth=$((depth + 1)) ;;
        '}') depth=$((depth - 1)) ;;
        ,)
          if [ "$depth" -eq 0 ]; then
            flatten "$prefix" "$part"
            part=
            continue
          fi
          ;;
      esac
      part+=$ch
    done
    flatten "$prefix" "$part"
  elif [[ $tree == *::* ]]; then
    trim "${tree%%::*}"
    flatten "$prefix::$trimmed" "${tree#*::}"
  else
    trim "${tree%% as *}"
    case $trimmed in
      '' | '*') ;;
      self) paths+=("$prefix") ;;
      *) paths+=("$prefix::$trimmed") ;;
    esac
  fi
}

# The root's names: each path a `pub use` statement of lib.rs names,
# keyed by its path through the module, valued by its root path. Each
# line loses its `//` comment first, as a statement in the scan below
# does. A path that is not identifiers (a block comment in a group, say)
# or a glob names nothing the script can know, so either ends the check.
declare -A root=()
stmt=
while IFS= read -r line || [ -n "$line" ]; do
  line=${line%%//*}
  if [ -z "$stmt" ]; then
    [[ $line =~ ^pub[[:space:]]+use[[:space:]]+(.*)$ ]] || continue
    stmt=${BASH_REMATCH[1]}
  else
    stmt+=" $line"
  fi
  [[ $stmt == *';'* ]] || continue
  body=${stmt%%;*}
  stmt=
  body=${body#crate::}
  body=${body#self::}
  paths=()
  flatten gresil "$body"
  for p in "${paths[@]}"; do
    if ! [[ $p =~ ^gresil(::[A-Za-z_][A-Za-z0-9_]*)+$ ]]; then
      echo "$lib: \`pub use $body;\` names \`${p#gresil::}\`, which is not a path of identifiers" >&2
      exit 2
    fi
    root[$p]=gresil::${p##*::}
  done
  if [[ $body == *'*'* ]]; then
    echo "$lib: \`pub use $body;\` re-exports a glob, whose names the check cannot read" >&2
    exit 2
  fi
done < "$lib"
if [ "${#root[@]}" -eq 0 ]; then
  echo "$lib: no \`pub use\` statement names a root name: the check read nothing" >&2
  exit 2
fi

# grep_lines PLACE ARGS...: `git grep -n` with ARGS into $out, exit 2 on
# a failure of git grep itself.
grep_lines() {
  local place=$1 rc
  shift
  set +e
  out=$(git grep --no-color -I -n "$@")
  rc=$?
  set -e
  if [ "$rc" -gt 1 ]; then
    echo "git grep failed on $place (exit $rc): the scan read nothing" >&2
    exit 2
  fi
}

hits=
declare -A statements=() imported=()
start='^[[:space:]]*(pub([[:space:]]*\([^)]*\))?[[:space:]]+)?use[[:space:]]+(::)?gresil::'
docline='^[[:space:]]*//(/([^/]|$)|!)'

# scan KEY PLACE: reads the `file:line:text` records in $out and counts
# PLACE's statements and names under KEY. A doc line is read as its code
# (its `///` or `//!` and one space dropped); a statement must end before
# a record that is not the next line of its file. A path is a hit when
# it, or its start, is a root name's path through its module.
scan() {
  local key=$1 place=$2 rec file n text code body name at p q rest hit
  local open= sfile= sprev=0 stext=
  local -a slines=()
  statements[$key]=0
  imported[$key]=0
  while IFS= read -r rec; do
    [ -n "$rec" ] || continue
    file=${rec%%:*}
    rec=${rec#*:}
    n=${rec%%:*}
    text=${rec#*:}
    code=$text
    if [[ $code =~ $docline ]]; then
      trim "$code"
      code=${trimmed:3}
      code=${code# }
    fi
    # a doc test's hidden line
    case $code in
      '#') code= ;;
      '# '*) code=${code:2} ;;
    esac
    if [ -n "$open" ] && { [ "$file" != "$sfile" ] || [ "$n" -ne $((sprev + 1)) ]; }; then
      echo "$sfile:${slines[0]%%:*}: a \`use\` statement with no \`;\`" >&2
      exit 2
    fi
    if [ -z "$open" ]; then
      [[ $code =~ $start ]] || continue
      open=1
      sfile=$file
      stext=${code#*gresil::}
      slines=()
    else
      stext+=" $code"
    fi
    sprev=$n
    slines+=("$n:$text")
    stext=${stext%%//*}
    [[ $stext == *';'* ]] || continue
    open=
    body=${stext%%;*}
    statements[$key]=$((statements[$key] + 1))
    paths=()
    flatten gresil "$body"
    imported[$key]=$((imported[$key] + ${#paths[@]}))
    for p in "${paths[@]}"; do
      # p, or its start: a root enum's variant reached through the
      # enum's module is a longer path too
      q=$p
      rest=
      while [ -z "${root[$q]:-}" ] && [[ $q == *::*::* ]]; do
        rest=::${q##*::}$rest
        q=${q%::*}
      done
      [ -n "${root[$q]:-}" ] && [ "${root[$q]}" != "$q" ] || continue
      name=${q##*::}
      hit=${slines[0]}
      for at in "${slines[@]}"; do
        if [[ ${at#*:} =~ (^|[^[:alnum:]_])$name([^[:alnum:]_]|$) ]]; then
          hit=$at
          break
        fi
      done
      hits+="${hits:+$'\n'}$sfile:$hit"$'\n'"  ${p##*::}: $p, from the root ${root[$q]}$rest"
    done
  done <<< "$out"
  if [ -n "$open" ]; then
    echo "$sfile:${slines[0]%%:*}: a \`use\` statement with no \`;\`" >&2
    exit 2
  fi
  if [ "${statements[$key]}" -eq 0 ]; then
    echo "no \`use gresil::…;\` statement in $place: the scan read nothing" >&2
    exit 2
  fi
}

grep_lines "the doc lines of $crate/src" -E "$docline" -- "$crate/src"
scan docs "the doc lines of $crate/src"
grep_lines "$crate/README.md" -e '' -- "$crate/README.md"
scan readme "$crate/README.md"
grep_lines "$crate/examples" -e '' -- "$crate/examples"
scan examples "$crate/examples"

if [ -n "$hits" ]; then
  echo "a root name, or a variant of one, imported through its module (the root path is its shortest):"
  echo "$hits"
  exit 1
fi
echo "${#root[@]} root names, each imported by its root path: the doc lines" \
  "${imported[docs]} names in ${statements[docs]} statements, the README" \
  "${imported[readme]} in ${statements[readme]}, the examples" \
  "${imported[examples]} in ${statements[examples]}"
