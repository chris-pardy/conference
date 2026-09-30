#!/usr/bin/env bash
# Fails if any file from the approved red-tests commit has changed since.
# Compares against the working tree, so uncommitted edits are caught too.
#
# Usage: scripts/check-tests-unchanged.sh <tests-commit>
set -euo pipefail

sha=${1:?usage: $0 <tests-commit>}
files=$(git show --name-only --no-renames --diff-filter=AM --format= "$sha")

if [ -z "$files" ]; then
  echo "commit $sha adds or modifies no files" >&2
  exit 1
fi

# shellcheck disable=SC2086 # file list is newline-separated paths
changed=$(git diff --name-only "$sha" -- $files)

if [ -n "$changed" ]; then
  echo "frozen test files changed since $sha:" >&2
  echo "$changed" >&2
  exit 1
fi

echo "frozen test files unchanged since $sha"
