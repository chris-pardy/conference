---
name: write-tests
description: Steps 5-6 of the feature pipeline. Use when a feature file has status tests. Writes automated tests for every test case using vitest, Playwright and vivarium, shows the user they fail for the right reasons, and freezes them on approval.
---

# Write the automated tests and show them red

## Write

- Write at least one automated test per test case, named after it:
  `test('TC-3: a member leaves a session chat', …)`. If a test case truly
  can't be automated, say why in the feature file and mark it `(manual)`.
- Use vitest for units and integration, and Playwright for UI flows.
  Anything that touches atproto runs against vivarium through the
  `@vivarium/client/vitest` fixtures (see Testing in `AGENTS.md`). Never use
  the live network.
- Test behavior through the surfaces a user or another feature would use:
  the UI, the API, the records written. Don't test internals, because the
  tests would then dictate an implementation that doesn't exist yet.
- If the repo doesn't have the test tooling yet, set it up as part of this
  step.
- If a test needs a module to exist before it can fail meaningfully, add a
  stub that throws `not implemented` **in a separate commit before** the
  tests commit. Stubs aren't frozen, so the implementation can replace them.

## Show them red

Run the suite. Every new test has to fail, and for the right reason:

- **Right:** an assertion fails, or a `not implemented` stub throws.
- **Wrong:** a syntax error, a bad import path, broken setup, or vivarium
  not running. Fix these and run again.
- A new test that already **passes** is testing nothing, or the behavior
  already exists. Find out which before going on.

All tests that existed before this feature must still pass.

Present the results to the user. This is a human gate.

- The command you ran.
- A table with one row per test case, showing the test, its file and its
  failure reason.
- The relevant part of the real output. Don't paraphrase it.
- The paths of the test files, so they can read the code.

Iterate on their feedback, re-running each time, until they approve.

## Freeze

On approval:

1. Commit the test files and test-only fixtures, and nothing else:
   `test(<slug>): red tests for TC-1..TC-n`.
2. Record that commit's SHA as `tests-commit` in the feature file, set
   `status: implementing`, commit and push.
3. Run `scripts/check-tests-unchanged.sh <sha>` once to confirm the freeze
   works.

Then go straight on to `implement-feature`. From here to the review gate,
don't ask the user anything.
