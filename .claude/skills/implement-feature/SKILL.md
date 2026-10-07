---
name: implement-feature
description: Steps 7-9 of the feature pipeline. Use when a feature file has status implementing. Implements the feature unsupervised until the frozen tests pass, then repeats adversarial review and rework until a review comes back clean.
---

# Implement, review, rework

This runs without supervision, so don't ask the user anything. If you're
truly blocked (a frozen test looks wrong, the requirements contradict each
other, or a decision needs a human), set `status: blocked`, explain why in
the feature file, push, tell the user, and stop.

Work in the feature's worktree, `.claude/worktrees/<slug>` (see
`write-tests`). If you're resuming and it's gone, recreate it with
`git worktree add .claude/worktrees/<slug> feature/<slug>` and run
`pnpm install` there.

The frozen files are the ones in `tests-commit`
(`git show --name-only --format= <tests-commit>`). Never edit, move, delete,
skip or `.only` them, and don't weaken their setup through config. The one
exception is a change the user approved and the feature file records: commit
it on its own, as `AGENTS.md` describes.

## 7. Implement

Write the code to make the frozen tests pass, following the architecture
analysis and the approved design. Match the style of the surrounding code.

Quality gates, all of which must pass before you commit:

- The full test suite is green, not only this feature's tests.
- `scripts/check-tests-unchanged.sh <tests-commit>` passes, or lists only
  files whose changes the feature file records as approved by the user.
- The linter and typechecker, if the repo has them, are clean.

Commit (`feat(<slug>): …`) and push.

## 8. Adversarial review

Spawn a **fresh** subagent to do the review. It must not have written this
code, so don't pass it your reasoning. Prompt it to follow
`.claude/skills/adversarial-review/SKILL.md` for this feature, and give it
the worktree's absolute path (it must run every command there), the feature
file path, the `tests-commit` SHA and the base branch (`origin/main`).

Append its findings to `## Review log` in the feature file as `### Round N`.

## 9. Rework

For every finding, either fix it or decline it with a reason:

- **blocking**, **major** and **minor**: fix it, or decline it with a
  concrete reason. "Disagree" isn't enough; say why the finding is wrong.
- **nit**: fix it if it's cheap, otherwise leave it.

Note what you did with each finding in the round's log entry. Re-run every
quality gate, then commit (`fix(<slug>): review round N`) and push.

## Repeat

Go back to step 8. The review is **clean** when a round finds nothing above
nit. The next reviewer sees the log, so it won't re-raise findings you
declined unless it can show the reason is wrong.

If round 10 still isn't clean, set `status: blocked`, summarize what keeps
coming back, and stop.

When the review is clean, go on to `ship-feature`.
