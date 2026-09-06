#!/usr/bin/env bash
# Stop hook: run `just ci` when Rust sources changed, and report failures back.
#
# Silent when there is nothing to check. Never blocks on a re-entry (the harness
# sets stop_hook_active when we are already resuming from this hook), so a
# failure Claude cannot fix cannot loop.

set -uo pipefail

payload=$(cat)

# Re-entry guard: do not block twice in a row.
if printf '%s' "$payload" | python3 -c '
import json, sys
try:
    sys.exit(0 if json.load(sys.stdin).get("stop_hook_active") else 1)
except Exception:
    sys.exit(1)
'; then
  exit 0
fi

root="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null)}"
[ -n "$root" ] && cd "$root" || exit 0

# Before T1 there is no workspace to check.
[ -f Cargo.toml ] || exit 0

command -v just >/dev/null 2>&1 || exit 0

# Only spend time on this when Rust or the build config actually changed.
if ! git status --porcelain 2>/dev/null | grep -qE '\.rs$|Cargo\.(toml|lock)$|\.snap$'; then
  exit 0
fi

output=$(just ci 2>&1)
status=$?
[ $status -eq 0 ] && exit 0

# Strip ANSI colour so the report reads cleanly in the transcript.
printf '%s' "$output" | sed -E $'s/\033\[[0-9;]*[a-zA-Z]//g; s/\033\([A-Z]//g' | tail -60 | python3 -c '
import json, sys
print(json.dumps({
    "decision": "block",
    "reason": "`just ci` failed. Fix it before finishing — this is step 6 of the task cycle in AGENTS.md.\n\n"
              + sys.stdin.read(),
}))
'
