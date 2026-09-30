---
name: adversarial-review
description: Step 8 of the feature pipeline. Use when asked to adversarially review a feature branch. Reviews the branch's diff as a skeptical outsider and reports at least three concrete, verified findings with severities. Read-only.
---

# Adversarial review

You didn't write this code. Assume it has bugs and find them. Report **at
least three** findings; more is better. Every finding must be real and
checked. The severity says how much it matters, so if you can only find
nits, report nits rather than inflating them.

Don't edit anything. Your output is the review.

## Inputs

Run everything in the feature's worktree (`.claude/worktrees/<slug>`, or the
path you were given), not the main checkout.

- The feature file, `features/<slug>.md`: what was promised, the test cases,
  the design, and earlier review rounds.
- The diff: `git fetch origin`, then `git diff origin/main...HEAD`.
- The `tests-commit` SHA.

## Check

1. Run `scripts/check-tests-unchanged.sh <tests-commit>`. If it fails, that's
   a blocking finding.
2. Run the full suite and the linter and typechecker. Any failure is
   blocking.
3. Then read the code, looking for:
   - Behavior that differs from the feature file, or test cases whose tests
     pass without really checking the behavior (vacuous assertions,
     over-mocking, and so on).
   - Edge cases: empty, huge, concurrent, offline, retried, partial failure.
   - Error handling: swallowed errors, wrong status codes, and leaks of
     internals.
   - Security: authorization checks, input validation, injection, secrets,
     and whose data can be read or written.
   - atproto correctness: record shapes against lexicons, identity and
     handle handling, cursors, and backfill.
   - Performance: N+1 queries, unbounded work, and missing pagination.
   - Design: duplicated code, dead code, over-engineering, and names or
     structure out of step with the rest of the codebase.
4. Verify each finding before you report it. Trace the code path, or run a
   quick script or test in a scratch location without committing it. Drop
   anything you can't make concrete.
5. Read the review log. Don't re-raise a declined finding unless you can show
   the reason for declining it is wrong.

## Severity

- **blocking:** wrong behavior on the main path, data loss, a security hole,
  a changed frozen test, or a red suite.
- **major:** wrong behavior in an edge case, a missing requirement, or a
  design flaw that will hurt other features.
- **minor:** maintainability, clarity, or small inefficiencies likely to
  cause trouble later.
- **nit:** style or taste.

## Report

Report each finding in this form:

```
N. [severity] path/to/file.ts:42: <one-sentence issue>
   Failure: <concrete input or state → what goes wrong>
   Fix: <suggested change>
```

End with a verdict: **clean** if nothing is above nit, otherwise **not
clean**, with counts by severity.
