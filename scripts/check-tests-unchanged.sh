#!/usr/bin/env bash
# Fails if any file from the approved red-tests commit has changed since.
# Compares against the working tree, so uncommitted edits are caught too.
#
# Usage: scripts/check-tests-unchanged.sh <tests-commit>
set -euo pipefail

sha=${1:?usage: $0 <tests-commit>}
# NUL-separated, so paths with spaces or glob characters stay whole.
files=()
while IFS= read -r -d '' file; do
  [ -n "$file" ] && files+=(":(literal)$file")
done < <(git show -z --name-only --no-renames --diff-filter=AM --format= "$sha")

if [ ${#files[@]} -eq 0 ]; then
  echo "commit $sha adds or modifies no files" >&2
  exit 1
fi

changed=$(git diff --name-only "$sha" -- "${files[@]}")

if [ -n "$changed" ]; then
  echo "frozen test files changed since $sha:" >&2
  echo "$changed" >&2
  exit 1
fi

echo "frozen test files unchanged since $sha"
