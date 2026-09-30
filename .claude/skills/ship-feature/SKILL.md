---
name: ship-feature
description: Steps 10-12 of the feature pipeline. Use when a feature file has status pr, or has status awaiting-human-review and its PR has merged. Opens the PR with a demo video, applies the review gate (auto-merge minor changes, wait for a human on architectural ones), and marks the feature complete once merged.
---

# Ship a feature

## 10. PR and demo

**Demo video.** If the feature has anything a user can see, write a
Playwright script that walks through its main test cases against a local
vivarium, with video recording on (`video: 'on'`, at a phone-sized
viewport). Keep the script outside the test suite so it isn't frozen and
doesn't run in CI. Upload the video:

```bash
gh release view demos >/dev/null 2>&1 || \
  gh release create demos --prerelease --title "Demo videos" --notes "Feature demo recordings"
gh release upload demos <slug>.webm --clobber
```

If the feature has nothing visible to demo, say why in the PR instead.

**PR.** Create it with `gh pr create --base main`. The title is the feature
name. The body contains:

- The summary from the feature file, with a link to the file.
- The test cases, as TC numbers and titles.
- The architecture verdict (standalone or cross-cutting).
- The review: how many rounds, what was found and fixed, and anything
  declined, with the reason.
- The demo link, or the reason there isn't one.
- The output of the final test run.

Record the PR URL as `pr:` in the feature file, commit and push.

## 11. Review gate

Wait for CI with `gh pr checks <n> --watch`.

- **Checks fail:** go back to `implement-feature` to fix them. Frozen tests
  stay frozen.
- **No checks are configured:** this PR can't auto-merge. Treat it as
  architectural and say why in the comment below.

When CI is green, classify the PR. It's **architectural** if any of these
apply:

- The feature's `impact` is `cross-cutting`.
- The diff adds or upgrades dependencies.
- The diff touches lexicons, database schema, build or CI config, or
  `AGENTS.md`/`.claude/`.
- The diff changes a module, API or component that another feature uses.

Otherwise it's **minor**.

Post the classification and its reason as a PR comment. Then:

- **Minor:** `gh pr merge <n> --squash --delete-branch`, then go to step 12.
- **Architectural:** set `status: awaiting-human-review`, commit and push.
  Tell the user the PR is waiting for them, then stop.

## 12. Mark complete

This step runs after the agent merges the PR, or in a later session once a
human has merged it (`gh pr view <n> --json state` shows `MERGED`):

```bash
git switch main && git pull --rebase
```

In `features/<slug>.md`, set `status: complete` and `merged: <YYYY-MM-DD>`.
Commit (`docs(<slug>): mark complete`) directly to `main` and push. Delete
the local feature branch.
