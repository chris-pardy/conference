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

# The tests-commit value from a feature file's frontmatter (only between the
# opening and closing ---), without quotes or a trailing # comment.
tests_commit_in() {
  awk '
    NR == 1 { if ($0 != "---") exit; next }
    $0 == "---" { exit }
    /^tests-commit:/ {
      sub(/^tests-commit:[[:space:]]*/, "")
      sub(/[[:space:]]*#.*$/, "")
      gsub(/["\047]/, "")
      split($0, words, /[[:space:]]+/)
      print words[1]
      exit
    }
  '
}

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

for value in "$frozen" "$current"; do
  if [[ -n $value && ! $value =~ ^[0-9a-f]{7,40}$ ]]; then
    fail "tests-commit \"$value\" in $file isn't a commit SHA"
  fi
done

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
  fail "tests-commit $frozen is not an ancestor of HEAD (not in this branch's history). Was the branch rebased? Once tests are frozen, bring in main with \`git merge origin/main\`, never a rebase."
fi

# Changed frozen tests are a warning, not a failure: the user can approve a
# change, recorded in the feature file. Everything above still fails.
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
if ! changed=$("$script_dir/check-tests-unchanged.sh" "$frozen" 2>&1); then
  message="frozen tests changed since $frozen; each change needs the user's approval recorded in $file"
  if [[ -n ${GITHUB_ACTIONS:-} ]]; then echo "::warning title=Frozen tests changed::$message"; fi
  say "warning: $message"
  echo "$changed" | sed 1d >&2
  exit 0
fi
say "$changed"
