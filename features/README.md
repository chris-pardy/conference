# Features

There's one file per feature, named after the capability
(`session-chat.md`, not `add-websockets.md`). Each file is the
human-readable record of the feature. It holds:

- what the feature is and why it exists
- the options considered, and which one was chosen
- the architecture analysis and any design review rounds
- the numbered test cases (`TC-1`, `TC-2`, …)
- the log of adversarial review rounds

The frontmatter tracks progress through the pipeline described in
[`AGENTS.md`](../AGENTS.md):

```yaml
status: implementing     # see AGENTS.md for the list
impact: standalone       # or cross-cutting; set by architecture analysis
depends-on: [attendee-profile]   # features that must be complete first
branch: feature/session-chat     # set when the build starts
tests-commit: 3f0e1e9    # the approved red tests; these files are frozen
```

Specs (up to `status: ready`) live on `main` before any code exists. The PR
that builds a feature also sets its `status` to `complete`, so on `main` the
status changes when the PR merges. `gh pr list --head feature/<slug>` finds
the PR.

```bash
grep -l '^status: ready' features/*.md      # specced, waiting to be built
grep -L '^status: complete' features/*.md   # everything not yet done
```
