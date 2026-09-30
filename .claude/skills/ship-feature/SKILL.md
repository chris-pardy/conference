---
name: ship-feature
description: Steps 10-12 of the feature pipeline. Use when a feature's adversarial review has come back clean. Marks the feature complete within its branch, opens the PR with a demo video, and applies the review gate (auto-merge minor changes, wait for a human on architectural ones).
---

# Ship a feature

## 10. Mark complete, then open the PR with a demo

**Mark complete.** In `features/<slug>.md` on the feature branch, set
`status: complete`. Commit (`docs(<slug>): mark complete`) and push. Because
the PR carries this change, `main` shows the feature as complete exactly
when the PR merges. Nothing needs to be committed to `main` afterwards.

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

## 11. Review gate

Wait for CI with `gh pr checks <n> --watch`.

- **Checks fail:** go back to `implement-feature` to fix them, pushing to
  the same branch. Frozen tests stay frozen, and `status` stays `complete`.
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
- **Architectural:** tell the user the PR is waiting for their review, and
  stop. Merging it completes the feature, and nothing more is needed.

## 12. Clean up

After a merge, run `git switch main && git pull --rebase` and delete the
local feature branch. The feature file on `main` now says `complete`.
