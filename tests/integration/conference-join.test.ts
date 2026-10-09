import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { changeHandle, parseAtUri } from '../support/atproto.ts'
import { sleep } from '../support/auth.ts'
import {
  ATMOSPHERECONF,
  Attendee,
  accountsIn,
  cliOk,
  type Deployment,
  deploy,
  importList,
  publicConference,
  seedConference,
  setMethods,
  sharedCode,
  uniqueCode,
} from '../support/conference.ts'

// Finding a conference through the app's API, and joining it by each method.
// The public page itself is in e2e/conference.spec.ts.

// Seeding takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 180_000, hookTimeout: 60_000 })

let dep: Deployment
beforeAll(async () => {
  dep = await deploy()
})
afterAll(async () => {
  await dep?.stop()
})

test('TC-7: The public page works by DID or by handle', async ({ viv }) => {
  const { conference, org } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const { rkey } = parseAtUri(conference.event)

  const byDid = await publicConference(dep, `at://${org.did}/community.lexicon.calendar.event/${rkey}`)
  const byHandle = await publicConference(dep, `at://${org.handle}/community.lexicon.calendar.event/${rkey}`)
  expect(byDid.status, JSON.stringify(byDid.body)).toBe(200)
  expect(byDid.body.name).toBe(ATMOSPHERECONF.name)
  expect(byHandle.status, JSON.stringify(byHandle.body)).toBe(200)
  expect(byHandle.body).toEqual(byDid.body)
})

test('TC-8: Ana joins with a shared invite code', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))

  const answer = await ana.join(conference, 'atmosphere27')
  expect(answer.status, JSON.stringify(answer.body)).toBe(200)
  expect(answer.body.status).toBe('joined')
  expect(await ana.membership(conference)).toEqual({ member: true, role: 'attendee' })
  // Inside: the conference as she now sees it says she's a member.
  const inside = await ana.get(conference.space)
  expect(inside.status).toBe(200)
  expect(inside.body.viewer).toMatchObject({ member: true })
})

test('TC-9: A wrong code doesn’t admit anyone', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await sharedCode(dep, conference, 'atmosphere27', olga)
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))

  const answer = await mallory.join(conference, 'atmosphere26')
  expect(answer.status, JSON.stringify(answer.body)).toBe(200)
  expect(answer.body.status).toBe('refused')
  expect(await mallory.isMember(conference)).toBe(false)
})

test('TC-10: Expired and used-up shared codes stop working', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const expiring = uniqueCode('early-bird')
  await sharedCode(dep, conference, expiring, olga, { expires: new Date(Date.now() + 3_000).toISOString() })
  const twoUses = uniqueCode('crew')
  await sharedCode(dep, conference, twoUses, olga, { maxUses: 2 })
  for (const name of ['first', 'second']) {
    const someone = await Attendee.signIn(dep, await viv.createAccount(viv.handle(name)))
    expect((await someone.join(conference, twoUses)).body.status, name).toBe('joined')
  }
  await sleep(4_000) // past the first code's expiry

  const joost = await Attendee.signIn(dep, await viv.createAccount(viv.handle('joost')))
  for (const code of [expiring, twoUses]) {
    const answer = await joost.join(conference, code)
    expect(answer.status, code).toBe(200)
    expect(answer.body.status, code).toBe('refused')
  }
  expect(await joost.isMember(conference)).toBe(false)
})

test('TC-11: Being on the attendee list by handle admits you on sign-in', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const ana = await viv.createAccount(viv.handle('ana'))
  const imported = await importList(dep, conference, [ana.handle], olga)
  expect(imported.code, imported.stderr).toBe(0)

  const asAna = await Attendee.signIn(dep, ana)
  const answer = await asAna.join(conference)
  expect(answer.status, JSON.stringify(answer.body)).toBe(200)
  expect(answer.body.status).toBe('joined')
  expect(await asAna.isMember(conference)).toBe(true)
})

test('TC-12: A handle on the list stays with the account it first named', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const bram = await viv.createAccount(viv.handle('bram'))
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const listed = bram.handle
  expect((await importList(dep, conference, [listed], olga)).code).toBe(0)

  // Bram moves to a new handle, and Mallory takes over the old one.
  const renamed = viv.handle('bram-new')
  await changeHandle(viv.url, bram, renamed)
  await changeHandle(viv.url, mallory, listed)

  const asMallory = await Attendee.signIn(dep, { ...mallory, handle: listed })
  expect((await asMallory.join(conference)).body.status).toBe('refused')
  expect(await asMallory.isMember(conference)).toBe(false)

  // The place on the list is still Bram's.
  const asBram = await Attendee.signIn(dep, { ...bram, handle: renamed })
  expect((await asBram.join(conference)).body.status).toBe('joined')
  expect(await asBram.isMember(conference)).toBe(true)
})

test('TC-13: A handle that doesn’t resolve is refused at import', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const ana = await viv.createAccount(viv.handle('ana'))
  const nobody = viv.handle('nobody-here')

  const imported = await importList(dep, conference, [nobody, ana.handle], olga)
  // The import reports the row that didn't resolve…
  expect(`${imported.stdout}\n${imported.stderr}`).toContain(nobody)
  // …and imports the rest.
  const asAna = await Attendee.signIn(dep, ana)
  expect((await asAna.join(conference)).body.status).toBe('joined')
})

test('TC-14: An open conference admits anyone signed in', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await setMethods(dep, conference, ['open'], olga)
  const joost = await Attendee.signIn(dep, await viv.createAccount(viv.handle('joost')))

  const answer = await joost.join(conference)
  expect(answer.status, JSON.stringify(answer.body)).toBe(200)
  expect(answer.body.status).toBe('joined')
  expect(await joost.isMember(conference)).toBe(true)
})

test('TC-15: A conference with no way in for you refuses', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['list'] })
  const ana = await viv.createAccount(viv.handle('ana'))
  expect((await importList(dep, conference, [ana.handle], olga)).code).toBe(0)
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))

  for (const attempt of [undefined, 'atmosphere27']) {
    const answer = await mallory.join(conference, attempt)
    expect(answer.status, String(attempt)).toBe(200)
    expect(answer.body.status, String(attempt)).toBe('refused')
  }
  expect(await mallory.isMember(conference)).toBe(false)
})

test('TC-16: Too many join attempts are slowed down', async ({ viv }) => {
  const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  await cliOk(dep, ['code', 'create', 'atmosphere27', '--conference', conference.space, '--as', olga.handle])
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))

  // One wrong code after another, until she's told to wait (within reason).
  let slowedAfter: number | undefined
  for (let attempt = 1; attempt <= 25 && slowedAfter === undefined; attempt++) {
    const answer = await mallory.join(conference, uniqueCode(`atmosphere${attempt}`))
    if (answer.status === 429) slowedAfter = attempt
    else expect(answer.body.status, `attempt ${attempt}`).toBe('refused')
  }
  expect(slowedAfter, 'slowed down within 25 attempts').toBeDefined()
  expect(slowedAfter).toBeGreaterThan(1)

  // Now even the right code waits.
  const right = await mallory.join(conference, 'atmosphere27')
  expect(right.status).toBe(429)
  expect(await mallory.isMember(conference)).toBe(false)
})
