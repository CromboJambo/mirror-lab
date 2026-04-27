#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/log-dep-audit.sh [dep-audit args...]

Runs the live dependency audit and appends a new mirror-log event only when the
compact audit summary differs from the most recent dep-audit entry already
stored in the database.

Environment:
  MIRROR_LOG_DB      Database path to use. Defaults to ./mirror.db
  MIRROR_LOG_SOURCE  Event source label. Defaults to dep-audit

Any arguments are forwarded to scripts/dep-audit.sh.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 is required to read the latest indexed audit entry" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
db_path="${MIRROR_LOG_DB:-$repo_root/mirror.db}"
source_name="${MIRROR_LOG_SOURCE:-dep-audit}"

current_line="$(
  cd "$repo_root"
  MIRROR_LOG_INDEX=1 scripts/dep-audit.sh "$@" | awk -F'mirror-log: ' '/^mirror-log: /{print $2}'
)"

if [[ -z "$current_line" ]]; then
  echo "dep-audit did not emit a compact summary" >&2
  exit 1
fi

last_line=""
if [[ -f "$db_path" ]]; then
  order_column="rowid"
  if sqlite3 "$db_path" "PRAGMA table_info(events);" | awk -F'|' '{print $2}' | grep -qx 'ingested_at'; then
    order_column="ingested_at"
  elif sqlite3 "$db_path" "PRAGMA table_info(events);" | awk -F'|' '{print $2}' | grep -qx 'timestamp'; then
    order_column="timestamp"
  fi

  last_line="$(
    sqlite3 "$db_path" "SELECT content FROM events WHERE source = '$source_name' ORDER BY $order_column DESC, rowid DESC LIMIT 1;"
  )"
fi

if [[ "$current_line" == "$last_line" ]]; then
  echo "No dependency surface change detected."
  exit 0
fi

sql_escape() {
  printf "%s" "$1" | sed "s/'/''/g"
}

columns="$(sqlite3 "$db_path" "PRAGMA table_info(events);" | awk -F'|' '{print $2}')"
has_column() {
  grep -qx "$1" <<<"$columns"
}

timestamp="$(date +%s)"
event_id="$(python3 -c 'import uuid; print(uuid.uuid4())')"

insert_columns="id,timestamp,source,content"
insert_values="'$(sql_escape "$event_id")',$timestamp,'$(sql_escape "$source_name")','$(sql_escape "$current_line")'"

if has_column "meta"; then
  insert_columns+=",meta"
  insert_values+=",NULL"
fi

if has_column "ingested_at"; then
  insert_columns+=",ingested_at"
  insert_values+=",$timestamp"
fi

if has_column "content_hash"; then
  content_hash="$(printf "%s" "$current_line" | sha256sum | awk '{print $1}')"
  insert_columns+=",content_hash"
  insert_values+=",'$content_hash'"
fi

sqlite3 "$db_path" "INSERT INTO events ($insert_columns) VALUES ($insert_values);"
echo "Logged dependency surface change to $db_path"
