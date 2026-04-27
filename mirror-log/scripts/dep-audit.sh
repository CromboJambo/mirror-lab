#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/dep-audit.sh [cargo tree args...]

Audits three dependency surfaces generated live from Cargo state:
  1. declared direct dependencies from cargo metadata
  2. active direct dependencies from cargo tree --depth 1
  3. full transitive dependency closure from cargo tree

Any arguments are forwarded to both cargo tree invocations. Useful examples:
  scripts/dep-audit.sh
  scripts/dep-audit.sh --features embedding
  scripts/dep-audit.sh --target x86_64-unknown-linux-gnu

Set MIRROR_LOG_INDEX=1 to emit a compact single-line summary suitable for
logging into mirror-log.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required to parse cargo metadata JSON" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

declared_file="$tmpdir/declared.txt"
active_file="$tmpdir/active_direct.txt"
full_file="$tmpdir/full_tree.txt"

cargo metadata --format-version 1 --no-deps \
  | python3 -c '
import json
import sys

metadata = json.load(sys.stdin)
workspace = set(metadata.get("workspace_members", []))
root = None
for package in metadata["packages"]:
    if package["id"] in workspace:
        root = package
        break

if root is None:
    raise SystemExit("unable to find workspace root package")

names = sorted({dep["name"] for dep in root.get("dependencies", [])})
for name in names:
    print(name)
' >"$declared_file"

cargo tree --depth 1 --prefix none "$@" \
  | tail -n +2 \
  | awk '{print $1}' \
  | sort -u >"$active_file"

cargo tree --prefix none "$@" \
  | tail -n +2 \
  | awk '{print $1}' \
  | sort -u >"$full_file"

declared_count="$(wc -l <"$declared_file" | tr -d ' ')"
active_count="$(wc -l <"$active_file" | tr -d ' ')"
full_count="$(wc -l <"$full_file" | tr -d ' ')"
transitive_count="$((full_count - active_count))"

echo "Declared direct deps: $declared_count"
echo "Active direct deps:   $active_count"
echo "Full compiled deps:   $full_count"
echo "Transitive deps:      $transitive_count"
echo

echo "Declared but inactive"
comm -23 "$declared_file" "$active_file" || true
echo

echo "Active but undeclared"
comm -13 "$declared_file" "$active_file" || true
echo

echo "Transitive closure only"
comm -13 "$active_file" "$full_file" || true

if [[ "${MIRROR_LOG_INDEX:-0}" == "1" ]]; then
  declared_only="$(comm -23 "$declared_file" "$active_file" | paste -sd, -)"
  active_only="$(comm -13 "$declared_file" "$active_file" | paste -sd, -)"
  transitive_only="$(comm -13 "$active_file" "$full_file" | paste -sd, -)"

  echo
  echo "mirror-log: declared=$declared_count active=$active_count full=$full_count transitive=$transitive_count declared_inactive=[${declared_only}] active_undeclared=[${active_only}] transitive_only=[${transitive_only}]"
fi
