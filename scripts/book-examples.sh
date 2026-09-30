#!/usr/bin/env bash
# Writes the book's Examples part: a page per Rust example under
# examples/rust/{apps,widgets,features,tools}, a page per kind listing them,
# an overview, and the part's entries at the end of docs/book/src/SUMMARY.md
# (everything from its `# Examples` line down is this script's).
#
# A page holds no source of its own - it `{{#include}}`s the example's file,
# so the book shows what the file says the day it is built. What can drift is
# the list: an example added, renamed or removed without a rerun. The book is
# published from a private repository (.forgejo/workflows/book.yml), so these
# pages are the only place a reader of the site sees an example at all.
#
#   scripts/book-examples.sh          rewrite the pages and the SUMMARY part
#   scripts/book-examples.sh --check  fail, with the diff, if a rerun would
#                                     change anything (ci.yml's `check` runs it)
set -euo pipefail
cd "$(dirname "$0")/.."

kinds=(apps widgets features tools)
# One line each, from the kinds table in examples/README.md.
blurb() {
  case "$1" in
    apps) echo "How it composes: an app owning its state, keymap or pane tree, touching whatever it needs." ;;
    widgets) echo "One element or one stock widget, in every state it has." ;;
    features) echo "One cross-cutting behaviour, with exactly the widgets it touches." ;;
    tools) echo "Registered as an example for want of a better slot, and not one: a corpus dump." ;;
  esac
}

generate() {
  local src="$1" kind name
  rm -rf "$src/examples"
  mkdir -p "$src/examples"
  {
    echo "# Examples"
    echo
    echo "Every Rust example in the kui repository, one page each, with its whole"
    echo "source. An example has one subject and shows it in every state it has."
    echo "Each runs inside the repository's devtools harness (\`kui-devtools\`,"
    echo "which is not published): in your own project, take the view and the"
    echo "\`on_event\` and launch them with \`kui_native::app\` as the book does."
    echo "The comment at the top of each file says what it shows and how the"
    echo "repository runs it."
    echo
    for kind in "${kinds[@]}"; do
      echo "- [\`$kind/\`]($kind.md): $(blurb "$kind")"
    done
  } > "$src/examples/index.md"
  for kind in "${kinds[@]}"; do
    mkdir -p "$src/examples/$kind"
    {
      echo "# \`$kind/\`"
      echo
      blurb "$kind"
      echo
      for f in $(find "examples/rust/$kind" -maxdepth 1 -name '*.rs' | sort); do
        name=$(basename "$f" .rs)
        echo "- [\`$name\`]($kind/$name.md)"
      done
    } > "$src/examples/$kind.md"
    for f in $(find "examples/rust/$kind" -maxdepth 1 -name '*.rs' | sort); do
      name=$(basename "$f" .rs)
      {
        echo "# \`$kind/$name.rs\`"
        echo
        echo "<!-- Written by scripts/book-examples.sh; the source is included when the book is built. -->"
        echo
        echo '```rust,noplayground'
        echo "{{#include ../../../../../$f}}"
        echo '```'
      } > "$src/examples/$kind/$name.md"
    done
  done
  # Command substitution drops the trailing blank lines, so a rerun is
  # byte-identical.
  local summary="$src/SUMMARY.md" head
  head=$(awk '/^# Examples$/ { exit } { print }' "$summary")
  {
    printf '%s\n' "$head"
    echo
    echo "# Examples"
    echo
    echo "- [Overview](examples/index.md)"
    for kind in "${kinds[@]}"; do
      echo "- [$kind](examples/$kind.md)"
      for f in $(find "examples/rust/$kind" -maxdepth 1 -name '*.rs' | sort); do
        name=$(basename "$f" .rs)
        echo "  - [$name](examples/$kind/$name.md)"
      done
    done
  } > "$summary.new"
  mv "$summary.new" "$summary"
}

if [ "${1:-}" = --check ]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  cp -R docs/book/src "$tmp/src"
  generate "$tmp/src"
  if ! diff -ru docs/book/src "$tmp/src"; then
    echo "the book's Examples part is stale: run scripts/book-examples.sh" >&2
    exit 1
  fi
else
  generate docs/book/src
fi
