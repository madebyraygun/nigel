#!/usr/bin/env bash
# Fails on identity strings that must never re-enter this public repository, and
# warns on figures shaped like real book data. See the "Public repository" section
# of CLAUDE.md for what belongs in each tier.
#
#   ./scripts/check-no-real-data.sh          # scan the tree (what CI runs)
#   ./scripts/check-no-real-data.sh --staged # scan the staged diff (pre-commit hook,
#                                            # installed via core.hooksPath=.githooks)
#
# SCOPE. This gate is about **content committed into the tree** — fixtures, docs,
# templates, test data, task notes, commit messages — where an operator's name,
# address, contact details or real figures have no business being.
#
# Commit authorship is NOT in scope and is not a violation. Every commit in this
# repository is authored by its real author, that is correct, and the history
# rewrite scrubbed content and never authorship. The same goes for the
# organisation's own package and repository metadata: the crate name, the GitHub
# slug, the Pages domain and the maintainer address in `Cargo.toml` are how a
# published project identifies itself. Neither is scanned here, and neither
# should be "fixed".
#
# The check is this script's **exit status**. Do not read its output to decide
# whether it passed: grepping for a word matches that word inside a failure
# report too, which is how a refused commit once got through.
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
