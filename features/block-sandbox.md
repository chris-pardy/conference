---
status: analysis
impact:
depends-on: [block-actions]
branch:
tests-commit:
---

# Block sandbox

## Summary

This covers the two uses of sandboxed JS in cards beyond middleware:

- **Custom blocks** render their own UI (toys, games, anything the
  vocabulary can't express) in an isolated frame.
- **Computed bindings** derive a value from bound sources, e.g. a
  formatted label or a ratio.

Both are in v1, as the user chose in the ui-blocks brainstorm. Split out of
[`ui-blocks`](ui-blocks.md) in design review round 1.

## Experience

**Custom blocks.** An attendee sees a custom block inline in a card, sized
to its content. It reacts to taps, and can take part in the card through
the normal action path. For example, a toy's "throw confetti" sends an
action, and everyone's view updates.

If the custom block crashes or stops responding, it's replaced by the
quiet "unavailable" placeholder. The rest of the card keeps working.

**Computed bindings.** A computed value shows like any other bound value.
If its code throws or runs over budget, the block shows "unavailable".

**Gallery.** The gallery gets a demo toy and a computed binding, plus
misbehaving versions of each: a throw, an infinite loop, a network attempt.

## Data

- **Custom blocks** are the `custom` block type, referencing a
  `#moduleRef` (see [`ui-blocks`](ui-blocks.md)).
  - They run in an `<iframe sandbox="allow-scripts">`, built with `srcdoc`
    so a `<meta>` CSP of `default-src 'none'` applies.
  - They talk only over `postMessage`:
    - in: the declared sources' values
    - out: action intents and a size hint
- **What a custom block can see:** only sources the card explicitly passes
  to it, and **never viewer-scoped data** (`mine` and the like). This was
  the user's choice in design review round 1.
- **Computed bindings** (`#computed`: module, export, inputs) run in the
  same pinned QuickJS runtime that [`block-actions`](block-actions.md) uses
  for middleware, with the same budgets. They run on the client only. They
  take only the resolved inputs, and return a JSON value.

## Options considered

### Sandboxed JS scope

- **Custom blocks and computed bindings, in v1** (chosen by the user).

### What custom blocks see

- **Declared, non-viewer-scoped sources only** (chosen).
- Anything, gated by a trust flag on the card's author.

### Custom block isolation

- **A sandboxed `srcdoc` iframe with a meta CSP** (chosen): a real DOM for
  toys, and no network through fetch, XHR, images or fonts.
- QuickJS drawing a restricted virtual UI tree: stronger containment, and
  it would survive a `while(true)`, but it means inventing a UI protocol,
  and toys lose canvas and animation.

## Out of scope

- Custom blocks rendering on the server, or in notifications.
- Network, storage or device APIs for sandboxed code.
- Installing and trusting third-party modules (`feed-apps`).

## Open questions

- **Accepted risks to confirm in design review:**
  - WebRTC isn't blocked by CSP, so a custom block can still leak its
    (non-viewer-scoped) inputs to the network.
  - Sandboxed opaque-origin frames usually share the page's process in
    Chrome and Safari, so an infinite loop freezes the whole app until the
    frame is killed, and a watchdog on the main thread can't fire during
    that freeze.

  Possible mitigations: limit who can author custom blocks, or require
  toys to yield (a cooperative frame budget).
- How big a custom block's module can be.

## Architecture analysis

## Design review

## Test cases

## Review log
