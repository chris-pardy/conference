---
name: design-review
description: Step 3 of the feature pipeline, for cross-cutting features only. Use when a feature file has status design-review. Drafts a technical design, has it critiqued, and iterates with the user over one or more review rounds until they approve it.
---

# Design review

A cross-cutting feature changes things other features rely on, so the design
gets agreed before tests or code are written.

## Draft

Write the design under `## Design review` in the feature file:

- **Approach:** how it works, in a few paragraphs.
- **Components:** modules, services and screens, both new and changed.
- **Data:** lexicons, records, spaces, indexes, and who writes and reads each.
- **Interfaces:** APIs and module boundaries other features will use.
- **Impact on existing features:** for each feature affected, what changes
  and how it stays working.
- **Alternatives:** designs you rejected and why.
- **Risks:** what could go wrong and what's still uncertain.

Match the length to the change. A small cross-cutting change gets a small
design.

## Critique before presenting

Spawn a fresh subagent that didn't write the design. Give it the feature file
and the files of the affected features, and ask it to find weaknesses:
gaps against the feature description, breakage in other features,
unnecessary complexity, and simpler alternatives. Fold in what holds up.

## Review rounds

Present the design to the user: a short summary, the decisions that most
need their eyes, and the path to the file. This is a human gate, so wait.

Record each round under `### Round N`, noting the user's feedback and what
changed as a result. Commit each round (`docs(<slug>): design review round
N`) to `main`, `git pull --rebase`, and push. Repeat until the user approves.

On approval, set `status: test-cases`, commit, push, and continue with
`write-test-cases`.
