import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { eventually, ownSpaceRev, pdsOf, xrpc } from '../support/atproto.ts'
import { sleep } from '../support/auth.ts'
import {
  Attendee,
  accountsIn,
  allowApp,
  type Deployment,
  deploy,
  importList,
  joinWithCode,
  member,
  NSID,
  orgSpaceRecords,
  post,
  seedConference,
  setMethods,
  sharedCode,
} from '../support/conference.ts'
import { OtherApp } from '../support/other-app.ts'

// Leaving, removal and bans.

// Seeding takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 180_000, hookTimeout: 60_000 })

let dep: Deployment
let app: OtherApp
beforeAll(async () => {
  dep = await deploy()
  app = await OtherApp.start(process.env.VIVARIUM_URL as string)
})
afterAll(async () => {
  await app?.stop()
  await dep?.stop()
})

test('TC-23: Ana leaves, and can come back', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const ana = await joinWithCode(dep, await viv.createAccount(viv.handle('ana')), conference, 'atmosphere27')

  const left = await ana.leave(conference)
  expect(left.status, JSON.stringify(left.body)).toBe(200)
  expect(await ana.isMember(conference)).toBe(false)
  const inside = await ana.get(conference.space)
  expect(inside.body.viewer?.member ?? false).toBe(false)

  const back = await ana.join(conference, 'atmosphere27')
  expect(back.body.status).toBe('joined')
  expect(await ana.isMember(conference)).toBe(true)
})

test('TC-24: A removed member loses access at once', async ({ viv }) => {
  const { conference, olga, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code'],
  })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const ruud = await viv.createAccount(viv.handle('ruud'))
  const asRuud = await joinWithCode(dep, ruud, conference, 'atmosphere27')
  expect(await asRuud.isMember(conference)).toBe(true)
  expect((await allowApp(dep, conference, app.clientId, olga)).code).toBe(0)
  const host = await (await app.signIn(olga)).open(conference.space)

  const removed = await member(dep, conference, 'remove', ruud, admins.pim as NonNullable<typeof admins.pim>)
  expect(removed.code, removed.stderr).toBe(0)

  // His very next request is refused, with no wait for any cache.
  expect(await asRuud.isMember(conference)).toBe(false)
  expect((await asRuud.get(conference.space)).body.viewer?.member ?? false).toBe(false)

  // And the space doesn't take his writes any more: either his PDS refuses,
  // or the space's host never takes the new revision.
  const write = await xrpc(await pdsOf(viv.url, ruud.did), 'com.atproto.space.createRecord', {
    token: ruud.accessJwt,
    body: {
      space: conference.space,
      repo: ruud.did,
      collection: NSID.post,
      record: { $type: NSID.post, ...post('Written after removal') },
    },
  })
  if (write.status === 200) {
    const rev = await ownSpaceRev(viv.url, ruud, conference.space)
    await sleep(3_000)
    const listed = (await host.listRepos()).find((repo) => repo.did === ruud.did)
    expect(listed?.repoRev, 'the host never takes his revision').not.toBe(rev)
  }
})

test('TC-25: A banned person can’t get back in by any method', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code', 'list'] })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const bram = await viv.createAccount(viv.handle('bram'))
  const asBram = await joinWithCode(dep, bram, conference, 'atmosphere27')
  expect((await importList(dep, conference, [bram.handle], olga)).code).toBe(0)

  const banned = await member(dep, conference, 'ban', bram, olga)
  expect(banned.code, banned.stderr).toBe(0)
  // He's removed.
  expect(await asBram.isMember(conference)).toBe(false)
  await eventually(
    () => orgSpaceRecords(conference, NSID.ban),
    (records) => records.some((r) => r.value.subject === bram.did),
    'a ban record for Bram is in the space',
  )

  // The code and the list refuse him…
  expect((await asBram.join(conference, 'atmosphere27')).body.status).toBe('refused')
  expect((await asBram.join(conference)).body.status).toBe('refused')
  // …and so does open joining.
  await setMethods(dep, conference, ['code', 'list', 'open'], olga)
  expect((await asBram.join(conference)).body.status).toBe('refused')
  expect(await asBram.isMember(conference)).toBe(false)
})

test('TC-26: Someone can be banned before they join', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code', 'list', 'open'] })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const mallory = await viv.createAccount(viv.handle('mallory'))
  expect((await importList(dep, conference, [mallory.handle], olga)).code).toBe(0)

  const banned = await member(dep, conference, 'ban', mallory, olga)
  expect(banned.code, banned.stderr).toBe(0)

  const asMallory = await Attendee.signIn(dep, mallory)
  expect((await asMallory.join(conference, 'atmosphere27')).body.status).toBe('refused')
  expect((await asMallory.join(conference)).body.status).toBe('refused')
  expect(await asMallory.isMember(conference)).toBe(false)
})

test('TC-27: Lifting a ban lets someone join again, without joining them', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const bram = await viv.createAccount(viv.handle('bram'))
  const asBram = await joinWithCode(dep, bram, conference, 'atmosphere27')
  expect((await member(dep, conference, 'ban', bram, olga)).code).toBe(0)
  expect((await asBram.join(conference, 'atmosphere27')).body.status).toBe('refused')

  const lifted = await member(dep, conference, 'unban', bram, olga)
  expect(lifted.code, lifted.stderr).toBe(0)
  expect(await asBram.isMember(conference)).toBe(false)

  expect((await asBram.join(conference, 'atmosphere27')).body.status).toBe('joined')
  expect(await asBram.isMember(conference)).toBe(true)
})
