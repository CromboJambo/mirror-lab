#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/log-dep-audit-matrix.sh

Logs dependency audit entries for a small matrix of Cargo feature surfaces.
Each surface is deduplicated independently through scripts/log-dep-audit.sh.

Environment:
  MIRROR_LOG_DB       Database path to use. Defaults to ./mirror.db
  MIRROR_LOG_PREFIX   Source prefix. Defaults to dep-audit
  DEP_AUDIT_SURFACES  Newline-delimited matrix entries. Defaults to:
                      default|
                      embedding|--features embedding

Each DEP_AUDIT_SURFACES entry is:
  label|cargo tree args

Examples:
  scripts/log-dep-audit-matrix.sh

  DEP_AUDIT_SURFACES=$'default|\ninference|--features inference' \
    scripts/log-dep-audit-matrix.sh
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
db_path="${MIRROR_LOG_DB:-$repo_root/mirror.db}"
source_prefix="${MIRROR_LOG_PREFIX:-dep-audit}"

surfaces="${DEP_AUDIT_SURFACES:-$'default|\nembedding|--features embedding'}"

logged=0
unchanged=0

while IFS= read -r entry; do
  [[ -z "$entry" ]] && continue

  label="${entry%%|*}"
  args_string="${entry#*|}"
  source_name="${source_prefix}:${label}"

  if [[ -n "$args_string" ]]; then
    read -r -a args <<<"$args_string"
  else
    args=()
  fi

  echo "Surface: $label"
  if output="$(MIRROR_LOG_DB="$db_path" MIRROR_LOG_SOURCE="$source_name" "$repo_root/scripts/log-dep-audit.sh" "${args[@]}")"; then
    echo "$output"
    if grep -q "Logged dependency surface change" <<<"$output"; then
      logged=$((logged + 1))
    else
      unchanged=$((unchanged + 1))
    fi
  else
    echo "$output" >&2
    exit 1
  fi
  echo
done <<<"$surfaces"

echo "Matrix summary: logged=$logged unchanged=$unchanged"
