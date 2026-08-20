#!/bin/bash
# Claude PreToolUse hook: runs cargo fmt --check and cargo test before git commits and PR merges.
# Blocks the action if formatting or tests fail.

INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')
CWD=$(echo "$INPUT" | jq -r '.cwd // empty')

# Only intercept git commit and gh pr merge commands
if ! echo "$COMMAND" | grep -qE '(git commit|gh pr merge)'; then
  exit 0
fi

# Run checks from the working directory
cd "$CWD" || exit 0

# A commit that stages no Rust build inputs cannot change cargo's verdict, so
# backlog/docs/web-only commits skip the suite. `gh pr merge` has no staging
# area to inspect and keeps the full run.
if echo "$COMMAND" | grep -q 'git commit'; then
  STAGED_ALL=$(git diff --cached --name-only)
  case "$COMMAND" in
    *" -a"*|*"--all"*) STAGED_ALL=$(printf '%s\n%s' "$STAGED_ALL" "$(git diff --name-only)") ;;
  esac
  if ! echo "$STAGED_ALL" | grep -qE '^(crates/|Cargo\.toml|Cargo\.lock|build\.rs)'; then
    exit 0
  fi
fi

# Check formatting on staged .rs files only (avoids blocking on pre-existing issues)
STAGED_RS=$(git diff --cached --name-only --diff-filter=ACM -- '*.rs' 2>/dev/null)
if [ -n "$STAGED_RS" ]; then
  FMT_OUTPUT=$(cargo fmt --check 2>&1)
  if [ $? -ne 0 ]; then
    # Only fail if the fmt diff involves staged files
    STAGED_FMT_FAIL=false
    for f in $STAGED_RS; do
      if echo "$FMT_OUTPUT" | grep -q "$f"; then
        STAGED_FMT_FAIL=true
        break
      fi
    done
    if [ "$STAGED_FMT_FAIL" = true ]; then
      echo "Blocked: cargo fmt --check failed on staged files. Run 'cargo fmt' first." >&2
      exit 2
    fi
  fi
fi

# Run tests (no-default-features to match CI)
if ! cargo test --no-default-features -- --test-threads=1 2>/dev/null; then
  echo "Blocked: cargo test failed. Fix failing tests before committing." >&2
  exit 2
fi

exit 0
