---
name: brainstorm-feature
description: Step 1 of the feature pipeline. Use whenever the user wants to add, change or explore a feature of the app. Runs a Q&A with the user until the feature is concrete, then writes features/<slug>.md with the description and the options considered.
---

# Brainstorm a feature

The goal is a concrete, shared understanding of one feature, written down so
that someone who wasn't in the conversation knows what's being built and why.

## Before asking anything

- Read `features/` to see which features exist and how they relate to this
  one.
- Skim the code this feature would touch. Don't ask the user anything the
  repo can answer.

## The Q&A

Ask questions one at a time, or in small groups of closely related ones. When
the answer is a choice, offer the options and put your recommendation first.
Work through:

- **Who and when:** who uses this, and at what moment (before, during or
  after the conference; organizer or attendee).
- **The problem:** what's painful or impossible today.
- **The experience:** what the user sees and does, step by step.
- **The data:** what gets stored, who owns it and who can see it. Name the
  atproto records or spaces if you can.
- **Edge cases and failures:** empty states, offline, permissions,
  concurrent edits, bad input.
- **Scope:** what this feature deliberately doesn't do.
- **Done:** how we'll know it works.

When a question has real alternatives, write them down along with their
trade-offs, which one the user picked and why. Keep the rejected options too,
because they're part of the record.

Keep asking until you could write the test cases without guessing. If the
feature turns out to be several features, suggest splitting it and brainstorm
one at a time.

## Finish

1. Summarize the feature back to the user in a few sentences and ask them to
   confirm or correct it. This is a human gate.
2. Pick a slug that names the capability, not the implementation
   (`session-chat`, not `add-websocket`).
3. Write `features/<slug>.md` from [template.md](template.md) and set
   `status: analysis`. Fill in the summary, experience, options considered,
   out-of-scope and open-questions sections. If the feature needs other
   features to exist first, list their slugs in `depends-on`. Leave the
   later sections as headings.
4. On `main`, commit (`docs(<slug>): feature brainstorm`), run
   `git pull --rebase`, and push.
5. Go straight on to `analyze-architecture`, which doesn't need the user.
