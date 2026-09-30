#!/usr/bin/env bash
# Enforces a feature branch's frozen tests (see AGENTS.md), in CI and locally.
#
# The branch comes from the pull request in CI (GITHUB_HEAD_REF) and from git
# otherwise. On feature/<slug>, the freeze is the first tests-commit ever
# recorded in features/<slug>.md, so a branch can't erase or re-point it.
#
# Usage: scripts/frozen-tests.sh
set -euo pipefail

say() { echo "frozen-tests: $*"; }
fail() { echo "frozen-tests: $*" >&2; exit 1; }

branch=${GITHUB_HEAD_REF:-$(git branch --show-current)}
if [[ ! $branch =~ ^feature/(.+)$ ]]; then
  say "skipped: not a feature branch (${branch:-detached HEAD})"
  exit 0
fi
slug=${BASH_REMATCH[1]}
file=features/$slug.md

tests_commit_in() { sed -n 's/^tests-commit:[[:space:]]*//p' | head -n 1 | tr -d '[:space:]'; }

# The branch's own history of the feature file, oldest first. Anything
# already on main counts as the starting point.
base=
for ref in origin/main main; do
  if git rev-parse --verify --quiet "$ref^{commit}" >/dev/null; then base=$ref; break; fi
done

frozen=
if [[ -n $base ]]; then
  frozen=$(git show "$base:$file" 2>/dev/null | tests_commit_in || true)
  range=$base..HEAD
else
  range=HEAD
fi
if [[ -z $frozen ]]; then
  while read -r commit; do
    value=$(git show "$commit:$file" 2>/dev/null | tests_commit_in || true)
    if [[ -n $value ]]; then frozen=$value; break; fi
  done < <(git rev-list --reverse "$range" -- "$file")
fi

current=
if [[ -f $file ]]; then current=$(tests_commit_in < "$file"); fi

if [[ -z $frozen && -z $current ]]; then
  say "skipped: no tests-commit yet in $file"
  exit 0
fi
# Recorded in the working tree but not committed yet: that's the freeze.
if [[ -z $frozen ]]; then frozen=$current; fi

if [[ -z $current ]]; then
  fail "the freeze was removed: $file had tests-commit $frozen, now it's blank"
fi

full() { git rev-parse --verify --quiet "$1^{commit}" 2>/dev/null || echo "$1"; }
if [[ $(full "$current") != $(full "$frozen") ]]; then
  fail "tests-commit was re-pointed from $frozen to $current in $file; the freeze can't be moved"
fi

if ! git merge-base --is-ancestor "$frozen" HEAD 2>/dev/null; then
  fail "tests-commit $frozen is not an ancestor of HEAD (not in this branch's history)"
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
"$script_dir/check-tests-unchanged.sh" "$frozen"
