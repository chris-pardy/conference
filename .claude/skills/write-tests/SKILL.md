---
name: write-tests
description: Steps 5-6 of the feature pipeline, and the start of a build. Use when the user asks to build a feature with status ready. Starts the feature branch, refreshes the spec against what's changed since it was written, writes automated tests for every test case using vitest, Playwright and vivarium, shows the user they fail for the right reasons, and freezes them on approval.
---

# Start the build: refresh, write the tests, show them red

## Start

1. Check that every feature in `depends-on` has `status: complete`. If one
   doesn't, tell the user and stop.
2. From an up-to-date `main`, run `git switch -c feature/<slug>`, set
   `branch:` in the feature file, and push with `-u`.

## Refresh the spec

The spec may have been written long before this build, and other features
may have landed since. Find out what's changed:
`git log --oneline <last commit touching features/<slug>.md>..main`. Then
re-check:

- The architecture analysis. Is the code it describes still there? Did a
  standalone feature become cross-cutting because of what landed?
- The design, if there is one. Does it still fit the code?
- The test cases. Do they contradict how a completed feature now behaves?

Update the feature file with what you find. If the verdict flips to
cross-cutting and there's no design, do a `design-review` now. If a test
case needs to change, keep its number, show the user the change, and get
their approval, since it alters the contract they approved. Commit on the
branch.

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
