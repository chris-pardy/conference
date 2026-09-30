import { VivariumClient } from '@vivarium-dev/client'
import { test, expect } from '@vivarium-dev/client/vitest'

// bsky.app's DID: it exists on the real network and never in a sealed box.
const REAL_NETWORK_DID = 'did:plc:z72i7hdynmk6r22z27h6tvur'

test('TC-9: tests run against a sealed vivarium', async ({ viv }) => {
  expect(process.env.VIVARIUM_URL, 'the check should run tests against its one shared box').toBeTypeOf('string')
  expect(viv.url).toBe(process.env.VIVARIUM_URL)

  const doc = await fetch(`${viv.url}/${REAL_NETWORK_DID}`)
  expect(doc.status).toBe(404)

  const handle = await fetch(`${viv.url}/xrpc/com.atproto.identity.resolveHandle?handle=bsky.app`)
  expect(handle.ok).toBe(false)
})

test('TC-13: a test can ask for a pristine box', async ({ vivFresh }) => {
  expect(process.env.VIVARIUM_URL, 'the check should run tests against its one shared box').toBeTypeOf('string')
  expect(vivFresh.url).not.toBe(process.env.VIVARIUM_URL)
  expect(await vivFresh.isUp()).toBe(true)

  // Separate from the shared box: an account made here doesn't exist there.
  const session = await vivFresh.createAccount('tc13-fresh.vivarium.test')
  const shared = new VivariumClient(process.env.VIVARIUM_URL as string)
  const sharedDids = (await shared.listAccounts()).map((a) => a.did)
  expect(sharedDids).not.toContain(session.did)

  const doc = await fetch(`${vivFresh.url}/${REAL_NETWORK_DID}`)
  expect(doc.status).toBe(404)
})
