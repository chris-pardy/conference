---
name: analyze-architecture
description: Step 2 of the feature pipeline. Use when a feature file has status analysis. Analyzes the existing code and features and decides whether the feature is standalone or cross-cutting, which determines whether it needs a design review.
---

# Architecture analysis

Decide whether this feature stands on its own or reaches into things other
features depend on. That verdict decides whether it needs a design review
(step 3) and, later, whether its PR can merge without a human (step 11).

## Look

- Read the feature file and every other file in `features/`.
- Find the code the feature would touch: modules, data models, lexicons,
  APIs, UI screens, shared components, build and CI setup. On a large
  codebase, hand the search to an Explore subagent and keep the conclusions.
- For each existing feature, ask whether its behavior or its tests could
  change.

## Decide

The feature is **cross-cutting** if it does any of these:

- Introduces or changes shared foundations: the stack or framework, the
  project layout, auth, build or CI, or shared UI components or theme.
- Adds or changes lexicons, record shapes or database schema.
- Adds or changes an API or module that other features call.
- Changes the behavior of an existing feature.

Otherwise it's **standalone**: new code paths that nothing else depends on.
If you're unsure, call it cross-cutting.

The first features in an empty repo are cross-cutting, because they set the
foundations everything else builds on.

## Write it up

Fill in the `## Architecture analysis` section of the feature file:

- **Existing code touched:** paths, or "none: no code yet".
- **Features affected:** links to their files and how each one is affected.
- **New shared surfaces:** anything this feature adds that later features
  will build on.
- **Verdict:** standalone or cross-cutting, with a one-paragraph reason.

Set `impact:` to match. Set `status: design-review` if the feature is
cross-cutting, or `status: test-cases` if it's standalone. Commit
(`docs(<slug>): architecture analysis`) and push.

Tell the user the verdict in a line or two, then continue with
`design-review` or `write-test-cases`.
