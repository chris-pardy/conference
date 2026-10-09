import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { didDocument, pdsOf, publicRecords, serviceEndpoint, xrpcOk } from '../support/atproto.ts'
import { attestKeys } from '../support/attestation.ts'
import { sleep } from '../support/auth.ts'
import {
  ATMOSPHERECONF,
  Attendee,
  accountsIn,
  addAdmin,
  connectAdmin,
  connectOrg,
  createConference,
  type Deployment,
  deploy,
  eventsideDoc,
  isConferenceSpace,
  member,
  memberRecord,
  NSID,
  orgSpaceRecords,
  seedConference,
  setRole,
  spacePolicies,
} from '../support/conference.ts'

// The organization and its conference: connecting the organization's account,
// creating the conference, and who may act as an admin.

// Seeding takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 180_000, hookTimeout: 60_000 })

let dep: Deployment
beforeAll(async () => {
  dep = await deploy()
})
afterAll(async () => {
  await dep?.stop()
})

const origin = (url: string | undefined) => (url ? new URL(url).origin : url)

test('TC-1: Creating a conference makes eventside its managing app', async ({ viv }) => {
  const account = await viv.createAccount(viv.handle('atmosphere'))
  const docBefore = await didDocument(viv.url, account.did)
  const org = await connectOrg(dep, account)
  const olga = await viv.createAccount(viv.handle('olga'))
  await connectAdmin(dep, olga)

  const conference = await createConference(dep, org, olga)

  // The space is on the organization's own PDS…
  expect(isConferenceSpace(conference.space), conference.space).toBe(true)
  expect(conference.space.startsWith(`at://${org.did}/`)).toBe(true)
  const policies = await spacePolicies(conference)
  expect(policies.uri).toBe(conference.space)
  // …and its policies name eventside as the managing app, for reads and writes.
  const eventside = await eventsideDoc(dep)
  expect(String(eventside.id)).toMatch(/^did:web:/)
  for (const policy of [policies.readPolicy, policies.writePolicy]) {
    expect(policy.$type).toBe('com.atproto.simplespace.defs#managingAppPolicy')
    expect(policy.managingApp).toBe(`${eventside.id}#eventside_access`)
  }
  // Eventside's DID document says where the PDS asks, and lists its signing keys.
  expect(origin(serviceEndpoint(eventside, 'eventside_access'))).toBe(origin(dep.url))
  expect(attestKeys(eventside).length).toBeGreaterThanOrEqual(1)

  // The organization's identity is untouched: same DID document, same PDS.
  expect(await didDocument(viv.url, account.did)).toEqual(docBefore)
  const pds = await pdsOf(viv.url, account.did)
  expect((await xrpcOk(pds, 'com.atproto.repo.describeRepo', { params: { repo: account.did } })).did).toBe(account.did)
})

test('TC-2: A public conference publishes an event other calendar apps can read', async ({ viv }) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv))
  expect(isConferenceSpace(conference.space), conference.space).toBe(true)
  expect(conference.space.startsWith(`at://${org.did}/`)).toBe(true)

  // Any calendar app reads the organization's public repo, with no session.
  const events = await publicRecords(viv.url, org.did, NSID.event)
  const event = events.find((r) => r.uri === conference.event)
  expect(event, `the event ${conference.event} is in ${org.did}'s public repo`).toBeDefined()
  expect(event?.value.name).toBe(ATMOSPHERECONF.name)
  expect(Date.parse(event?.value.startsAt)).toBe(Date.parse(ATMOSPHERECONF.startsAt))
  expect(Date.parse(event?.value.endsAt)).toBe(Date.parse(ATMOSPHERECONF.endsAt))
  expect(JSON.stringify(event?.value.locations ?? [])).toContain(ATMOSPHERECONF.city)

  // The event links to the conference on eventside, which a calendar app shows.
  const links: string[] = (event?.value.uris ?? []).map((u: { uri: string }) => u.uri)
  expect(
    links.some((uri) => uri.startsWith(dep.url)),
    `a link to ${dep.url} among ${links}`,
  ).toBe(true)
  // And eventside's sidecar sits next to it, naming the event and the space.
  const sidecars = await publicRecords(viv.url, org.did, NSID.sidecar)
  expect(sidecars.map((r) => r.value)).toContainEqual(
    expect.objectContaining({ event: conference.event, space: conference.space }),
  )
})

test('TC-3: Creating a conference sets up its main feed', async ({ viv }) => {
  const { conference } = await seedConference(dep, accountsIn(viv))

  const until = Date.now() + 10_000
  let feeds = await orgSpaceRecords(conference, NSID.feed)
  while (feeds.length === 0 && Date.now() < until) {
    await sleep(250)
    feeds = await orgSpaceRecords(conference, NSID.feed)
  }
  expect(feeds.length, 'a feed record in the organization’s repo in the space').toBeGreaterThanOrEqual(1)
})

test('TC-4: Admins act only as themselves', async ({ viv }) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim', 'lotte'],
    unconnected: ['lotte'],
  })
  const pim = admins.pim as NonNullable<typeof admins.pim>
  const lotte = admins.lotte as NonNullable<typeof admins.lotte>
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const joost = await viv.createAccount(viv.handle('joost'))

  // Lotte is staff but hasn't connected; Mallory is nobody.
  for (const as of [lotte, mallory]) {
    const refused = await member(dep, conference, 'add', joost, as)
    expect(refused.code, `acting as ${as.handle} is refused`).not.toBe(0)
  }
  const asJoost = await Attendee.signIn(dep, joost)
  expect(await asJoost.isMember(conference)).toBe(false)
  await sleep(1_000)
  const records = await orgSpaceRecords(conference, NSID.member)
  expect(records.filter((r) => r.value.subject === joost.did)).toEqual([])

  // Pim has connected: acting as Pim works, and the decision records Pim.
  const admitted = await member(dep, conference, 'add', joost, pim)
  expect(admitted.code, admitted.stderr).toBe(0)
  expect(await asJoost.isMember(conference)).toBe(true)
  const record = await memberRecord(conference, joost.did)
  expect(record?.value.decidedBy).toBe(pim.did)
})

test('TC-5: There’s always an owner', async ({ viv }) => {
  const { conference, olga, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'] })
  const pim = admins.pim as NonNullable<typeof admins.pim>
  const asOlga = await Attendee.signIn(dep, olga)
  expect(await asOlga.role(conference)).toBe('owner')

  // Olga is the only owner: nobody can remove her, demote her, or have her leave.
  for (const as of [olga, pim]) {
    expect((await member(dep, conference, 'remove', olga, as)).code, `remove, as ${as.handle}`).not.toBe(0)
    expect((await setRole(dep, conference, olga, 'staff', as)).code, `demote, as ${as.handle}`).not.toBe(0)
  }
  const left = await asOlga.leave(conference)
  expect(left.status, JSON.stringify(left.body)).toBeGreaterThanOrEqual(400)
  expect(left.status).toBeLessThan(500)
  expect(await asOlga.role(conference)).toBe('owner')

  // Once Kees is an owner too, Olga can step down.
  const kees = await viv.createAccount(viv.handle('kees'))
  const added = await addAdmin(dep, conference, kees, 'owner', olga)
  expect(added.code, added.stderr).toBe(0)
  const steppedDown = await setRole(dep, conference, olga, 'staff', olga)
  expect(steppedDown.code, steppedDown.stderr).toBe(0)
  expect(await asOlga.role(conference)).toBe('staff')
})
