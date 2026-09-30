---
name: write-test-cases
description: Step 4 of the feature pipeline. Use when a feature file has status test-cases. Writes numbered, human-readable test cases into the feature file and gets the user's approval.
---

# Write test cases

The test cases are the contract for the feature. The automated tests are
written from them, and the code is done when they pass. They're written for a
person to read, so they contain no code and no implementation names.

## Write

Add them under `## Test cases` in the feature file, numbered `TC-1`, `TC-2`,
and so on. Never renumber a test case once the user has seen it. If one is
dropped, strike it through and leave the number in place.

```markdown
### TC-3: A member leaves a session chat

- **Given** Ana is a member of the "Keynote" chat
- **When** she leaves the chat
- **Then** she no longer sees it in her chat list
- **And** the other members see "Ana left"
```

Cover:

- The main experience, from start to finish.
- Each option chosen in brainstorming and design review.
- Edge cases and failures: empty, offline, no permission, bad input,
  concurrent use.
- For cross-cutting features, regression cases showing that each affected
  feature still works.

If the `## Open questions` section isn't empty, resolve those questions with
the user first.

## Approve

Show the user the list, with titles only if it's long, plus anything you had
to interpret. This is a human gate. Iterate until they approve.

On approval, set `status: tests`, commit (`docs(<slug>): test cases`), push,
and continue with `write-tests`.
