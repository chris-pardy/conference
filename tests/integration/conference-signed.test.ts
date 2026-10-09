import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { type Account, eventually, type Json, pdsOf, writeSpaceRecord, xrpc, xrpcOk } from '../support/atproto.ts'
import { attestKeys, didWebDocument, managingAppDid, validKeys } from '../support/attestation.ts'
import { sleep } from '../support/auth.ts'
import {
  Attendee,
  accountsIn,
  allowApp,
  type Conference,
  cliOk,
  connectAdmin,
  countedRecords,
  type Deployment,
  deploy,
  eventsideDoc,
  joinWithCode,
  member,
  memberRecord,
  NSID,
  orgSpaceRecords,
  post,
  type Seeded,
  seedConference,
  sharedCode,
} from '../support/conference.ts'
import { OtherApp } from '../support/other-app.ts'

// Signed membership records, as another app reads and checks them, and the
// outbox that writes them.

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

const pimOf = (seeded: Seeded): Account => {
  if (!seeded.admins.pim) throw new Error('the seed has no Pim')
  return seeded.admins.pim
}

/** AtmosphereConf with Pim on staff, the shared code, and Ana joined with it. */
async function withAna(viv: Parameters<typeof accountsIn>[0], on: Deployment = dep) {
  const seeded = await seedConference(on, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  await sharedCode(on, seeded.conference, 'atmosphere27', seeded.olga)
  const ana = await viv.createAccount(viv.handle('ana'))
  await joinWithCode(on, ana, seeded.conference, 'atmosphere27')
  return { ...seeded, pim: pimOf(seeded), ana }
}

/**
 * The organization's member records, as an allowed app reads them with an
 * organizer's delegation, and the space's policies as it reads them too.
 */
async function membersThroughApp(conference: Conference, organizer: Account, holds: (records: Json[]) => boolean) {
  const access = await (await app.signIn(organizer)).open(conference.space)
  const records = await eventually(
    () => access.records(conference.org.did, NSID.member),
    holds,
    'the member records are there',
  )
  return { records, space: await access.getSpace() }
}

const about = (records: Json[], did: string) => records.find((r) => r.value.subject === did)

test('TC-37: Another app can verify who’s a member and who organizes', async ({ viv }) => {
  const { conference, olga, pim, ana } = await withAna(viv)
  expect((await allowApp(dep, conference, app.clientId, olga)).code).toBe(0)

  const { records, space } = await membersThroughApp(
    conference,
    olga,
    (records) => !!about(records, ana.did) && !!about(records, pim.did),
  )
  expect(about(records, ana.did)?.value.role).toBe('attendee')
  expect(about(records, pim.did)?.value.role).toBe('staff')

  // The app trusts the signer the space names as its managing app, and
  // resolves that DID itself: it's eventside's.
  const signer = managingAppDid(space.writePolicy)
  expect(signer).toBe(managingAppDid(space.readPolicy))
  const signerDoc = await didWebDocument(signer)
  expect(signerDoc.id).toBe((await eventsideDoc(dep)).id)
  const keys = attestKeys(signerDoc)
  for (const did of [ana.did, pim.did]) {
    const record = about(records, did)?.value
    const valid = validKeys(record, { repository: conference.org.did, signerDoc })
    expect(valid.length, `the record about ${did} verifies`).toBeGreaterThanOrEqual(1)
    for (const key of valid) expect(keys).toContain(key)
  }
})

test('TC-38: Records that aren’t properly signed don’t count', async ({ viv }) => {
  const { conference, olga, ana } = await withAna(viv)
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const bram = await viv.createAccount(viv.handle('bram'))
  await joinWithCode(dep, bram, conference, 'atmosphere27')
  const anasRecord = await memberRecord(conference, ana.did)
  const anasValue = anasRecord?.value as Json

  // Mallory's app writes a record claiming she's an owner, signatures copied
  // from a real one, into her own repo in the space.
  const forged = { ...anasValue, subject: mallory.did, role: 'owner' }
  await xrpc(await pdsOf(viv.url, mallory.did), 'com.atproto.space.putRecord', {
    token: mallory.accessJwt,
    body: { space: conference.space, repo: mallory.did, collection: NSID.member, rkey: mallory.did, record: forged },
  })
  // Ana's record in the organization's repo is altered to make her an owner.
  const altered = { ...anasValue, role: 'owner' }
  await xrpcOk(await pdsOf(viv.url, conference.org.did), 'com.atproto.space.putRecord', {
    token: conference.org.account.accessJwt,
    body: {
      space: conference.space,
      repo: conference.org.did,
      collection: NSID.member,
      rkey: anasRecord?.rkey,
      record: altered,
    },
  })

  // Neither verifies, for any reader.
  const signerDoc = await eventsideDoc(dep)
  expect(validKeys(forged, { repository: mallory.did, signerDoc })).toEqual([])
  expect(validKeys(altered, { repository: conference.org.did, signerDoc })).toEqual([])

  // Mallory isn't a member, and can't act as an owner.
  expect(await (await Attendee.signIn(dep, mallory)).isMember(conference)).toBe(false)
  await connectAdmin(dep, mallory)
  expect((await member(dep, conference, 'ban', bram, mallory)).code, 'Mallory bans Bram').not.toBe(0)

  // The altered record is ignored: Ana is still an attendee, with no owner's powers.
  expect(await (await Attendee.signIn(dep, ana)).role(conference)).toBe('attendee')
  await connectAdmin(dep, ana)
  expect((await member(dep, conference, 'ban', bram, ana)).code, 'Ana bans Bram').not.toBe(0)
  expect(await (await Attendee.signIn(dep, bram)).isMember(conference)).toBe(true)
  // Olga still decides as before.
  expect((await member(dep, conference, 'remove', bram, olga)).code).toBe(0)
})

test('TC-39: A new signing key is used, and older records still verify', async ({ viv }) => {
  const { conference, pim, ana } = await withAna(viv)
  const anasRecord = await memberRecord(conference, ana.did)
  const before = attestKeys(await eventsideDoc(dep))
  expect(before.length).toBeGreaterThanOrEqual(1)

  await cliOk(dep, ['keys', 'add'])
  const signerDoc = await eventsideDoc(dep)
  const after = attestKeys(signerDoc)
  const added = after.filter((key) => !before.includes(key))
  expect(added, 'one new #eventside_attest key').toHaveLength(1)
  for (const key of before) expect(after).toContain(key)

  // Pim admits Joost: his record is signed with the new key.
  const joost = await viv.createAccount(viv.handle('joost'))
  expect((await member(dep, conference, 'add', joost, pim)).code).toBe(0)
  const joostsRecord = await memberRecord(conference, joost.did)
  expect(validKeys(joostsRecord?.value, { repository: conference.org.did, signerDoc })).toEqual(added)

  // Ana's earlier record still verifies, with the first key.
  const anaKeys = validKeys(anasRecord?.value, { repository: conference.org.did, signerDoc })
  expect(anaKeys.length).toBeGreaterThanOrEqual(1)
  for (const key of anaKeys) expect(before).toContain(key)
})

test('TC-40: A decision interrupted by a crash is completed on restart', async ({ viv }) => {
  const own = await deploy()
  try {
    const { conference, olga, pim } = await withAna(viv, own)
    expect((await allowApp(own, conference, app.clientId, olga)).code).toBe(0)
    const joost = await viv.createAccount(viv.handle('joost'))

    // The server is down, and the CLI stops right after committing Pim's decision.
    await own.stop()
    const admitted = await member(own, conference, 'add', joost, pim, {
      env: { EVENTSIDE_HALT_AFTER_DECISION: '1' },
    })
    expect([0, 86], admitted.stderr).toContain(admitted.code)
    expect((await orgSpaceRecords(conference, NSID.member)).some((r) => r.value.subject === joost.did)).toBe(false)

    // The server starts again and finishes the decision.
    await own.restart()
    await memberRecord(conference, joost.did)
    await writeSpaceRecord(viv.url, joost, conference.space, NSID.post, post('Made it in after all'))
    const access = await (await app.signIn(olga)).open(conference.space)
    await eventually(
      () => access.writers(),
      (writers) => writers.includes(joost.did),
      'the host takes Joost’s write',
    )
  } finally {
    await own.stop()
  }
})

test('TC-41: A record written while not a member is never counted', async ({ viv }) => {
  const { conference, olga, pim } = await withAna(viv)
  const ruud = await viv.createAccount(viv.handle('ruud'))
  await joinWithCode(dep, ruud, conference, 'atmosphere27')
  expect((await member(dep, conference, 'remove', ruud, pim)).code).toBe(0)

  // While removed, he writes from another client: his own PDS session.
  const whileRemoved = await xrpc(await pdsOf(viv.url, ruud.did), 'com.atproto.space.createRecord', {
    token: ruud.accessJwt,
    body: {
      space: conference.space,
      repo: ruud.did,
      collection: NSID.post,
      record: { $type: NSID.post, ...post('Written while removed') },
    },
  })
  // He's admitted again later, and writes again.
  expect((await member(dep, conference, 'add', ruud, pim)).code).toBe(0)
  await writeSpaceRecord(viv.url, ruud, conference.space, NSID.post, post('Written after rejoining'))

  const texts = (records: Json[]) => records.map((r) => r.value?.text)
  await eventually(
    () => countedRecords(dep, conference, NSID.post, olga),
    (records) => texts(records).includes('Written after rejoining'),
    'the record he wrote after rejoining is counted',
  )
  await sleep(2_000)
  const counted = texts(await countedRecords(dep, conference, NSID.post, olga))
  expect(counted).toContain('Written after rejoining')
  expect(counted, `written while removed (${whileRemoved.status})`).not.toContain('Written while removed')
})
