#!/usr/bin/env bash
# Fails on identity strings that must never re-enter this public repository, and
# warns on figures shaped like real book data. See the "Public repository" section
# of CLAUDE.md for what belongs in each tier.
#
#   ./scripts/check-no-real-data.sh          # scan the tree (what CI runs)
#   ./scripts/check-no-real-data.sh --staged # scan the staged diff (pre-commit hook)
set -uo pipefail

mode="${1:-tree}"
self='scripts/check-no-real-data.sh'
status=0

# Identity strings. Naming them here leaks nothing — the org and the commit author
# are already public — and the tree is clean, so any hit is a regression.
# nigel.rygn.io is the project's own Pages domain and is deliberately kept.
gate='Raygun|RAYGUN|\bDalton\b|\bRooney\b|(^|[^.a-z])rygn\.io|/Users/[a-z]|P\.O\. Box'
# Fixture and placeholder values are allowed; anything else shaped like them is not.
# Stated as an allowlist so the real values never have to appear in this file.
allow='P\.O\. Box 1234|/Users/(sam|you|<|\$)'

# Real book figures are large and oddly precise; statutory and fixture amounts are
# neither. Prose only — Rust test constants are not where this leaks. Advisory.
warn='\$[0-9]{2,3},[0-9]{3}\.[0-9]{2}|\b[0-9]{2}-[0-9]{7}\b|\b[0-9]{3}-[0-9]{2}-[0-9]{4}\b'

if [ "$mode" = "--staged" ]; then
  gate_subject=$(git diff --cached -U0 -- . ":(exclude)$self" | grep '^+' | grep -v '^+++')
  warn_subject=$(git diff --cached -U0 -- 'backlog/' 'docs/' '*.md' | grep '^+' | grep -v '^+++')
else
  gate_subject=$(git grep -nIE "$gate" -- . ':(exclude)site/' ":(exclude)$self" 2>/dev/null)
  warn_subject=$(git grep -nIE "$warn" -- 'backlog/' 'docs/' 2>/dev/null)
fi

hits=$(printf '%s\n' "$gate_subject" | grep -E "$gate" | grep -vE "$allow" || true)
if [ -n "$hits" ]; then
  echo "FAIL: identity strings that must not be committed"
  printf '%s\n' "$hits" | sed 's/^/  /'
  status=1
fi

soft=$(printf '%s\n' "$warn_subject" | grep -E "$warn" || true)
if [ -n "$soft" ]; then
  echo "WARN: figures shaped like real book data — confirm each is statutory or fixture"
  printf '%s\n' "$soft" | sed 's/^/  /'
fi

[ "$status" = 0 ] && echo "OK: no identity strings found"
exit "$status"
