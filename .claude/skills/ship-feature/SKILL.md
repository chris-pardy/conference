---
name: ship-feature
description: Steps 10-12 of the feature pipeline. Use when a feature's adversarial review has come back clean. Marks the feature complete within its branch, opens the PR with a demo video, and applies the review gate (auto-merge minor changes, wait for a human on architectural ones).
---

# Ship a feature

Steps 10 and 11 run in the feature's worktree, `.claude/worktrees/<slug>`.

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

Wait for CI with `gh pr checks <n> --watch`. Right after `gh pr create`,
the workflow may not have registered yet: if `gh pr checks` reports no
checks, retry every 15 seconds for up to 2 minutes before concluding there
are none.

- **Checks fail:** go back to `implement-feature` to fix them, pushing to
  the same branch. Frozen tests stay frozen, and `status` stays `complete`.
- **`frozen-tests` warns that frozen tests changed:** list each changed test
  and where the feature file records the user's approval in the PR body, and
  treat the PR as architectural.
- **No checks are configured** (still none after 2 minutes): this PR can't
  auto-merge. Treat it as architectural and say why in the comment below.

"Green" means every required check (`lint`, `build`, `test`,
`frozen-tests`) has passed on the PR's head commit. The agent merges as a
repository admin, and admins bypass the `main` ruleset, so GitHub won't stop
a red merge: this step is the gate. Never merge with `--admin`.

When CI is green, classify the PR. It's **architectural** if any of these
apply:

- The feature's `impact` is `cross-cutting`.
- The diff adds or upgrades dependencies.
- The diff touches lexicons, database schema, build or CI config, or
  `AGENTS.md`/`.claude/`. Build and CI config includes anything that decides
  which tests run or how: `.github/`, `scripts/`, `vitest.config.ts`,
  `playwright.config.ts`, `biome.jsonc`, `rustfmt.toml`,
  `rust-toolchain.toml`, `Cargo.toml` files, and the `scripts` in
  `package.json`.
- The diff changes a module, API or component that another feature uses.

Otherwise it's **minor**.

Post the classification and its reason as a PR comment. Then:

- **Minor:** confirm once more that every required check passed on the head
  commit, then `gh pr merge <n> --squash --delete-branch`, and go to step 12.
- **Architectural:** tell the user the PR is waiting for their review, and
  stop. Merging it completes the feature, and nothing more is needed.

## 12. Clean up

After a merge, leave the worktree (in Claude Code, `ExitWorktree` with
`action: "keep"`, since the session entered it by path), then from the main
checkout:

```bash
git worktree remove .claude/worktrees/<slug>
git branch -D feature/<slug>   # -D: a squash merge isn't an ancestor of main
git pull --rebase
```

The feature file on `main` now says `complete`. An architectural PR that's
waiting for a human keeps its worktree until whoever merges it runs this.
