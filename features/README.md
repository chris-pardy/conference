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
branch: feature/session-chat
tests-commit: 3f0e1e9    # the approved red tests; these files are frozen
pr: https://github.com/chris-pardy/conference/pull/4
merged: 2026-10-02
```

A feature is done when its `status` is `complete`.
