import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import {
  type Account,
  eventually,
  type Json,
  ownSpaceRecords,
  pdsOf,
  writeSpaceRecord,
  xrpc,
} from '../support/atproto.ts'
import {
  accountsIn,
  allowApp,
  type Deployment,
  deploy,
  disallowApp,
  joinWithCode,
  NSID,
  post,
  publicConference,
  seedConference,
  sharedCode,
} from '../support/conference.ts'
import { OtherApp } from '../support/other-app.ts'

// Inside a conference: members write but don't read, and which apps can read
// the space, with whose delegation.

// Seeding takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 180_000, hookTimeout: 60_000 })

let dep: Deployment
let app: OtherApp
let other: OtherApp
beforeAll(async () => {
  dep = await deploy()
  app = await OtherApp.start(process.env.VIVARIUM_URL as string)
  other = await OtherApp.start(process.env.VIVARIUM_URL as string)
})
afterAll(async () => {
  await other?.stop()
  await app?.stop()
  await dep?.stop()
})

const texts = (records: Json[]) => records.map((r) => r.value?.text)

/** AtmosphereConf with Pim on staff, Ana and Bram joined by code, and a post from each. */
async function withMembers(viv: Parameters<typeof accountsIn>[0]) {
  const seeded = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  await sharedCode(dep, seeded.conference, 'atmosphere27', seeded.olga)
  const ana = await viv.createAccount(viv.handle('ana'))
  const bram = await viv.createAccount(viv.handle('bram'))
  const asAna = await joinWithCode(dep, ana, seeded.conference, 'atmosphere27')
  await joinWithCode(dep, bram, seeded.conference, 'atmosphere27')
  await writeSpaceRecord(viv.url, bram, seeded.conference.space, NSID.post, post('Borrel at Hannekes Boom'))
  return { ...seeded, pim: seeded.admins.pim as Account, ana, bram, asAna }
}

/** Reads a writer's records through an app's access, once they're there. */
async function readThrough(
  access: Awaited<ReturnType<Awaited<ReturnType<OtherApp['signIn']>>['open']>>,
  repo: string,
  text: string,
) {
  return eventually(
    () => access.records(repo, NSID.post),
    (records) => texts(records).includes(text),
    `the app reads “${text}” from ${repo}`,
  )
}

test('TC-17: A member writes to the space but can’t read other members’ records', async ({ viv }) => {
  const { conference, olga, ana, bram, asAna } = await withMembers(viv)

  // Ana writes a record in the space, from her own PDS.
  await writeSpaceRecord(viv.url, ana, conference.space, NSID.post, post('Who’s going to the canal tour?'))
  expect(texts(await ownSpaceRecords(viv.url, ana, conference.space, NSID.post))).toContain(
    'Who’s going to the canal tour?',
  )
  // The space took it: its host lists her as a writer.
  expect((await allowApp(dep, conference, app.clientId, olga)).code).toBe(0)
  const asOrganizer = await (await app.signIn(olga)).open(conference.space)
  await eventually(
    () => asOrganizer.writers(),
    (writers) => writers.includes(ana.did),
    'the host lists Ana as a writer',
  )

  // Directly, with her own session, she can't read Bram's records.
  const direct = await xrpc(await pdsOf(viv.url, bram.did), 'com.atproto.space.listRecords', {
    token: ana.accessJwt,
    params: { space: conference.space, repo: bram.did, collection: NSID.post },
  })
  expect(direct.status).not.toBe(200)
  expect(JSON.stringify(direct.body)).not.toContain('Hannekes Boom')

  // Through another app, with her delegation, eventside (the managing app)
  // gives her no read access at all.
  const throughApp = await (await app.signIn(ana)).requestCredential(conference.space)
  expect(throughApp.status).not.toBe(200)
  expect(throughApp.body.credential).toBeUndefined()

  // Through eventside, nothing she asks for carries Bram's raw records.
  for (const answer of [
    await asAna.get(conference.space),
    await asAna.get(conference.event),
    await asAna.getMembership(conference),
  ]) {
    expect(JSON.stringify(answer.body)).not.toContain('Hannekes Boom')
  }
})

test('TC-18: Non-members can’t see inside', async ({ viv }) => {
  const { conference, olga, ana } = await withMembers(viv)
  expect((await allowApp(dep, conference, app.clientId, olga)).code).toBe(0)
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const nowhere = `${conference.space.slice(0, conference.space.lastIndexOf('/'))}/no-such-conference`

  // Mallory, through an allowed app: refused, as for a conference that
  // doesn't exist. (The PDS words the two refusals itself, so only the
  // outcome is compared: no credential either way.)
  const session = await app.signIn(mallory)
  for (const space of [conference.space, nowhere]) {
    const answer = await session.requestCredential(space)
    expect(answer.status, space).toBeGreaterThanOrEqual(400)
    expect(answer.body.credential, space).toBeUndefined()
  }

  // With her own session, straight at the organization's PDS: nothing.
  const direct = await xrpc(await pdsOf(viv.url, conference.org.did), 'com.atproto.space.listRecords', {
    token: mallory.accessJwt,
    params: { space: conference.space, repo: conference.org.did, collection: NSID.member },
  })
  expect(direct.status).not.toBe(200)

  // A visitor who isn't signed in, through eventside: the public view and nothing from inside.
  const visitor = await publicConference(dep, conference.event)
  expect(visitor.status).toBe(200)
  const shown = JSON.stringify(visitor.body)
  expect(shown).not.toContain('Hannekes Boom')
  expect(shown).not.toContain(ana.did)
})

test('TC-19: An allowed app reads the space with an organizer’s delegation', async ({ viv }) => {
  const { conference, olga, pim, ana, bram } = await withMembers(viv)
  await writeSpaceRecord(viv.url, ana, conference.space, NSID.post, post('Stroopwafels at the booth'))
  expect((await allowApp(dep, conference, app.clientId, olga)).code).toBe(0)

  const access = await (await app.signIn(pim)).open(conference.space)
  await readThrough(access, ana.did, 'Stroopwafels at the booth')
  await readThrough(access, bram.did, 'Borrel at Hannekes Boom')
  expect(await access.writers()).toEqual(expect.arrayContaining([ana.did, bram.did]))
})

test('TC-20: An allowed app can’t read others’ records for an attendee', async ({ viv }) => {
  const { conference, olga, ana, bram } = await withMembers(viv)
  await writeSpaceRecord(viv.url, ana, conference.space, NSID.post, post('Stroopwafels at the booth'))
  expect((await allowApp(dep, conference, app.clientId, olga)).code).toBe(0)
  const session = await app.signIn(ana)

  // For an attendee, eventside gives the app no read access to the space…
  const credential = await session.requestCredential(conference.space)
  expect(credential.status).not.toBe(200)
  expect(credential.body.credential).toBeUndefined()
  // …so Bram's records stay out of reach, through Ana's own token too.
  const bramsRecords = await xrpc(await pdsOf(viv.url, bram.did), 'com.atproto.space.listRecords', {
    token: ana.accessJwt,
    params: { space: conference.space, repo: bram.did, collection: NSID.post },
  })
  expect(bramsRecords.status).not.toBe(200)
  // Ana's own records, it can read.
  const own = await session.ownRecords(conference.space, NSID.post)
  expect(own.status, JSON.stringify(own.body)).toBe(200)
  expect(texts(own.body.records)).toContain('Stroopwafels at the booth')
})

test('TC-21: An app that isn’t allowed can’t read at all', async ({ viv }) => {
  const { conference, olga, pim, ana } = await withMembers(viv)
  // The first app is allowed; the other isn't.
  expect((await allowApp(dep, conference, app.clientId, olga)).code).toBe(0)

  for (const person of [olga, pim, ana]) {
    const answer = await (await other.signIn(person)).requestCredential(conference.space)
    expect(answer.status, `with ${person.handle}’s delegation`).not.toBe(200)
    expect(answer.body.credential).toBeUndefined()
  }
})

test('TC-22: Staff can manage which apps may read', async ({ viv }) => {
  const { conference, olga, pim, ana } = await withMembers(viv)
  await writeSpaceRecord(viv.url, ana, conference.space, NSID.post, post('Stroopwafels at the booth'))

  const allowed = await allowApp(dep, conference, app.clientId, pim)
  expect(allowed.code, allowed.stderr).toBe(0)
  const access = await (await app.signIn(olga)).open(conference.space)
  await readThrough(access, ana.did, 'Stroopwafels at the booth')

  const disallowed = await disallowApp(dep, conference, app.clientId, pim)
  expect(disallowed.code, disallowed.stderr).toBe(0)
  const after = await (await app.signIn(olga)).requestCredential(conference.space)
  expect(after.status).not.toBe(200)
  expect(after.body.credential).toBeUndefined()
})
