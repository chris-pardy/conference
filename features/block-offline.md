---
status: analysis
impact:
depends-on: [block-actions]
branch:
tests-commit:
---

# Block offline

## Summary

This keeps cards usable when the conference wifi isn't. Interactions made
while offline run through the same middleware on the device, show their
result straight away, and are queued. When the connection returns, they're
sent in order. The server's verdict then wins: any optimistic change it
disagrees with is reverted, and its message is shown.

Split out of [`ui-blocks`](ui-blocks.md) in design review round 1. The user
chose queue-and-send, with middleware isomorphic so it can run on the PWA.

## Experience

**Going offline:**

1. An attendee has the app open and the connection drops.
2. Cards keep showing their last known data, with a quiet "offline" marker.
3. They vote on a poll. The local middleware accepts it, the tally bar
   moves by one, and the vote shows as pending.

**Coming back online:**

1. The queue sends pending actions in order.
2. Each confirmed action loses its pending marker.
3. If the server rejects one (say the poll closed while they were offline),
   the card reverts and shows the server's message ("Voting closed at
   14:30").
4. Actions queued after a rejected one are re-checked against the reverted
   state before they're sent.

**Local rejections.** When the local middleware rejects an action ("One
vote each"), the card shows that immediately, and nothing is queued.

**Limits:**

- Offline support covers **going offline with the app already open**,
  including reloading the page while offline once it has been loaded.
- A first visit while offline isn't supported.
- If the session expires while actions are queued, the attendee is asked to
  sign in again before the queue sends. Queued actions are kept until then.

## Data

- **The queue** lives on the device, in IndexedDB. Each entry holds the
  action, its `clientId`, the local middleware result, and its status
  (pending, sent, confirmed, or rejected along with the server's message).
  It survives reloads.
- **The cache** also lives in IndexedDB: the last resolved sources for each
  card, plus the card records and their middleware module code, so both
  can run offline.
- **The optimistic overlay:** pending actions are applied on top of cached
  sources using each view's optimistic reducer (see
  [`block-actions`](block-actions.md)). Record sources get no optimistic
  change.
- **The server clock is final:** an action queued before a deadline but
  sent after it may be rejected, and the spec says so.

## Options considered

### Offline behavior

- **Queue and send, with local middleware** (chosen by the user in the
  ui-blocks brainstorm).
- Fail visibly with retry, nothing queued: rejected.

### Where the queue runs

- **In the page (IndexedDB), without a service worker** (chosen): testable
  with Playwright `context.setOffline` under `ci-pipeline`'s
  `serviceWorkers: 'block'` setting, and it works on Safari.
- Service worker Background Sync: sends even with the app closed, but it
  clashes with the test setup, and Safari doesn't support it. It can be
  added later.

### After a rejection

- **Re-check later queued actions against the reverted state** (chosen):
  e.g. a second vote that was locally rejected as a duplicate becomes
  valid again once the first vote is rejected.
- Revert everything after a rejection: loses valid work.

## Out of scope

- Sending while the app is closed (Background Sync).
- Offline first visits, and precaching the whole app beyond the PWA
  default.
- Optimistic updates for record sources, or for plugin-defined views
  without reducers.

## Open questions

- Whether a rejection that happens while the card is off screen needs a
  notification or badge. It was deferred from the ui-blocks brainstorm.
- The maximum queue age before actions are dropped as stale.

## Architecture analysis

## Design review

## Test cases

## Review log
