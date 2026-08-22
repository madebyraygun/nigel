#!/usr/bin/env bash
# Shows what a schema migration does to a set of books, by running the same
# books through two builds and diffing every report.
#
#   ./scripts/verify-migration.sh                       # origin/main vs the current branch
#   ./scripts/verify-migration.sh main feat/my-branch   # any two refs
#   BASELINE_BIN=… CANDIDATE_BIN=… ./scripts/verify-migration.sh   # skip the builds
#
# The baseline build seeds demo books and writes its reports; the candidate
# build then opens those same books, so its migrations run against a database
# the older code created — which is the case a fresh install never exercises
# and the one that goes wrong in the field. Every report is captured before and
# after, and the diff is the answer.
#
# SAFETY. The run happens inside its own HOME and data directory, so the books
# you keep are never opened, never migrated and never read. To try it against
# real books, copy them into the lab the script prints; do not point it at the
# originals.
#
# Portable to macOS and Linux: no GNU-only flags, and the database is located
# through the app's own settings file rather than by searching the filesystem.

set -uo pipefail

BASELINE_REF="${1:-origin/main}"
CANDIDATE_REF="${2:-HEAD}"

command -v git >/dev/null || { echo "git is required" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 is required (it reads the app's settings and database)" >&2; exit 1; }

REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "run this from inside the repository" >&2; exit 1; }

say() { printf '\n\033[1m== %s\033[0m\n' "$1"; }

# One SQL query, one line per row. The app ships no query surface, and every
# platform that runs Nigel has python; a sqlite3 binary is not a given.
q() {
  python3 - "$1" "$2" <<'PY'
import sqlite3, sys
try:
    for row in sqlite3.connect(sys.argv[1]).execute(sys.argv[2]):
        print("  " + "  ".join("" if v is None else str(v) for v in row))
except Exception as exc:
    print("  (query failed: %s)" % exc)
PY
}

LAB="$(mktemp -d)"
OUT="$LAB/reports"
mkdir -p "$OUT"

say "Lab: $LAB  (your own books are untouched)"

# --- the two builds -------------------------------------------------------

build_ref() { # ref, label -> prints a binary path
  local ref="$1" label="$2" wt="$LAB/build-$2"
  git -C "$REPO" worktree add --detach "$wt" "$ref" >/dev/null 2>&1 || {
    echo "could not check out '$ref'" >&2; return 1; }
  ( cd "$wt" && cargo build --quiet ) >&2 || { echo "build failed for '$ref'" >&2; return 1; }
  echo "$wt/target/debug/nigel"
}

if [ -n "${BASELINE_BIN:-}" ] && [ -n "${CANDIDATE_BIN:-}" ]; then
  say "Using the binaries you provided"
else
  say "Building both refs — the first run compiles twice and is slow"
  echo "  baseline:  $BASELINE_REF"
  echo "  candidate: $CANDIDATE_REF"
  BASELINE_BIN="$(build_ref "$BASELINE_REF" baseline)" || exit 1
  CANDIDATE_BIN="$(build_ref "$CANDIDATE_REF" candidate)" || exit 1
fi
for b in "$BASELINE_BIN" "$CANDIDATE_BIN"; do
  [ -x "$b" ] || { echo "not executable: $b" >&2; exit 1; }
done

# Only now: cargo and rustup read their toolchain out of the real HOME, so the
# lab replaces it after the builds and before the app ever runs.
export HOME="$LAB"
export NIGEL_DATA_DIR="$LAB/books"

# --- books the baseline build made ---------------------------------------

say "1. Seed demo books with the BASELINE build"
"$BASELINE_BIN" init --data-dir "$NIGEL_DATA_DIR" --profile business >/dev/null 2>&1
"$BASELINE_BIN" demo >/dev/null 2>&1 || true

# `demo` repoints the data directory at its own books, so ask the settings file
# where they ended up rather than guessing or searching.
DB="$(python3 - "$LAB" <<'PY'
import json, os, sys
cfg = os.path.join(sys.argv[1], ".config", "nigel", "settings.json")
try:
    data_dir = json.load(open(cfg))["data_dir"]
except Exception:
    data_dir = os.path.join(sys.argv[1], "books")
print(os.path.join(os.path.expanduser(data_dir), "nigel.db"))
PY
)"
echo "  database: $DB"
[ -f "$DB" ] || { echo "  no database was created — is 'demo' available in this build?" >&2; exit 1; }
q "$DB" "SELECT 'schema_version before: '||value FROM metadata WHERE key='schema_version'"

REPORTS="pnl balance tax cashflow expenses k1"

say "2. Capture the BASELINE reports"
for r in $REPORTS; do
  "$BASELINE_BIN" report "$r" > "$OUT/before-$r.txt" 2>&1 || true
  # A misspelled report name writes clap's error into the file, and the same
  # error lands in the after-file too — which then diffs clean and reads as a
  # report that did not move. Fail on it instead.
  case "$(head -1 "$OUT/before-$r.txt")" in
    error:*) echo "  '$r' is not a report this build has:" >&2
             sed 's/^/    /' "$OUT/before-$r.txt" >&2
             exit 1 ;;
  esac
done
echo "  captured: $REPORTS"

say "3. Open the same books with the CANDIDATE build — its migrations run"
"$CANDIDATE_BIN" report pnl >/dev/null 2>&1 || true
q "$DB" "SELECT 'schema_version after: '||value FROM metadata WHERE key='schema_version'"

say "4. What the schema gained"
q "$DB" "SELECT 'table: '||name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"

say "5. Every report, before against after"
CHANGED=0
for r in $REPORTS; do
  "$CANDIDATE_BIN" report "$r" > "$OUT/after-$r.txt" 2>&1 || true
  if diff -q "$OUT/before-$r.txt" "$OUT/after-$r.txt" >/dev/null 2>&1; then
    echo "  $r: identical"
  else
    CHANGED=1
    echo "  $r: CHANGED —"
    diff "$OUT/before-$r.txt" "$OUT/after-$r.txt" | sed 's/^/    /'
  fi
done
[ "$CHANGED" -eq 0 ] && echo "
  Nothing moved. For a migration that only adds structure that is the pass;
  for one that corrects a figure, it means the demo books do not exercise it."

say "Done"
echo "  reports:  $OUT"
echo "  clean up: rm -rf $LAB"
echo "  worktrees this made are detached and removable with:  git -C $REPO worktree prune"
