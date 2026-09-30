---
status: analysis
impact:
depends-on: [block-actions]
branch:
tests-commit:
---

# Block sandbox

## Summary

This covers the two uses of sandboxed code in cards beyond middleware:

- **Custom blocks** have their own logic and UI: toys, games, anything the
  vocabulary can't express.
- **Computed bindings** derive a value from bound sources, e.g. a
  formatted label or a ratio.

Both run as **wasm modules** in the same runtime that
[`block-actions`](block-actions.md) uses for middleware. Custom blocks draw
by returning standard blocks, plus a **canvas** block for free-form
drawing. They animate either declaratively or with frame ticks.

Both are in v1, as the user chose in the ui-blocks brainstorm. Split out of
[`ui-blocks`](ui-blocks.md) in design review round 1, and reshaped around
wasm in round 2.

## Experience

**Custom blocks.** An attendee sees a custom block inline in a card. It
looks native, because it's made of themed vocabulary blocks, and a toy's
drawing surface is a canvas block inside it. It reacts to taps and inputs,
and it can take part in the card through the normal action path. For
example, a toy's "throw confetti" sends an action, and everyone's view
updates.

**Animation:**

- Simple motion runs smoothly with no per-frame code: a wheel spins to a
  result, a badge pulses, confetti bursts. The block declares the
  animation, and the app plays it.
- Game-like toys get frame ticks at up to 30fps while they're on screen.

**When things go wrong:**

- A custom block that crashes, runs out of budget or keeps overrunning its
  frame budget is replaced by the quiet "unavailable" placeholder.
- The rest of the card, and the app, keep working. A runaway module can't
  freeze the page.

**Computed bindings.** A computed value shows like any other bound value.
If its code fails or runs over budget, the block shows "unavailable".

**Gallery.** The gallery gets a demo toy (a spinning wheel, and a canvas
confetti toy) and a computed binding, plus misbehaving versions: a trap, an
infinite loop, a memory hog, and an over-budget tick.

## Data

- **Modules** are wasm, referenced by `#moduleRef` (a wasm blob CID, the
  uploader's DID, an optional script, and exports). JS authors ship a
  script that runs on the shared QuickJS-wasm runtime.
- **The custom block ABI** is Extism-style, JSON in and out:
  - `render(state, inputs)` → `{blocks, state}`, where `blocks` is a tree
    of vocabulary blocks (including `canvas`) using `#animate`
  - `event(state, event)` → `{state, actions?}`, where `event` is a tap
    (with canvas coordinates), an input change, or a submit
  - `tick(state, dt)` → `{state}`, optional, and only called when the block
    asks for frames
- **State** is JSON the module owns. It's held by the host between calls,
  per viewer and per mounted block, and it's never persisted or shared.
- **The `canvas` block** holds a size, a list of draw commands (rect,
  circle, path, text, image from a declared source) with `#animate`
  allowed on their properties, and a tap handler id. Colors are theme
  token names or explicit values.
- **`#animate`** holds a target property, `from`, `to`, `duration`,
  `easing` and `repeat`. It's played by the host, on either blocks or draw
  commands.
- **Inputs:** only sources the card explicitly passes to the custom block,
  and **never viewer-scoped data** (`mine` and the like). This was the
  user's choice in design review round 1.
- **Computed bindings** (`#computed`: module, export, inputs) call
  `compute(inputs)` and get back a JSON value. They run on the client.
- **Budgets:**
  - fuel for each call
  - a per-frame fuel budget for `tick`
  - a memory cap
  - repeated overruns turn the block into "unavailable"

  Modules run in a Worker that the host can terminate.

## Options considered

### Sandboxed code scope

- **Custom blocks and computed bindings, in v1** (chosen by the user).

### What custom blocks see

- **Declared, non-viewer-scoped sources only** (chosen in round 1).
- Anything, gated by a trust flag on the card's author.

### Isolation technology

- **wasm modules on the shared runtime** (chosen in round 2): no network or
  DOM unless imported, so no WebRTC leak; fuel-bounded; runs in a killable
  Worker; any language; and the same runtime as middleware.
- A sandboxed `srcdoc` iframe running JS (the round 1 draft): a real DOM,
  but it leaks through WebRTC, and an infinite loop freezes the page
  because the frame shares its process.
- JS in QuickJS-wasm only (the step in between): JS only. It's subsumed,
  because JS is now one wasm module among many.

### How custom blocks draw

- **Return standard blocks, plus a `canvas` block with draw commands**
  (chosen): themed and reusable, and the only new UI language is the draw
  commands.
- Standard blocks only: no free-form toys.
- Canvas only: ignores the theme, and can't reuse inputs and text.

### Animation

- **Both: declarative `animate` played by the host, and `tick(dt)` for
  per-frame code under a budget** (chosen): common motion costs no module
  calls, and games still work.
- Host-driven transitions only: no games.

## Out of scope

- Custom blocks on the server, or in notifications. Custom block rendering
  is client-only, while the same modules' `handle` still runs on the
  server as middleware.
- Network, storage, audio or device APIs for modules.
- Installing and trusting third-party modules (`feed-apps`).
- Multiplayer toy state beyond actions and views, like real-time shared
  cursors.

## Open questions

- The per-module size limit, and whether to precompile the QuickJS runtime
  module ahead of time to cut phone startup cost.
- Which draw commands the canvas block has in v1: images from sources,
  gradients, text with theme fonts.
- The fuel budgets for each call and each frame, to be set by measuring on
  a mid-range phone.

## Architecture analysis

## Design review

## Test cases

## Review log
