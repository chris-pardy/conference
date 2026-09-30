#!/usr/bin/env bash
# Creates or updates the `main` ruleset from .github/rulesets/main.json.
# Needs a gh login with admin rights on the repository. Safe to re-run: it
# updates the ruleset with the same name instead of adding another.
#
# Usage: scripts/apply-ruleset.sh [owner/repo]
set -euo pipefail

repo=${1:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}
file=$(dirname "${BASH_SOURCE[0]}")/../.github/rulesets/main.json
name=$(jq -r .name "$file")

id=$(gh api "repos/$repo/rulesets" --jq ".[] | select(.name == \"$name\") | .id" | head -n 1)
if [[ -n $id ]]; then
  gh api --method PUT "repos/$repo/rulesets/$id" --input "$file" >/dev/null
  echo "updated ruleset \"$name\" ($id) on $repo"
else
  id=$(gh api --method POST "repos/$repo/rulesets" --input "$file" --jq .id)
  echo "created ruleset \"$name\" ($id) on $repo"
fi
