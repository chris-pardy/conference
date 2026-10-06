import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import {
  createPublicRecord,
  didDocument,
  ownSpaceRecords,
  parseAtUri,
  plcData,
  publicRecords,
  serviceEndpoint,
} from '../support/atproto.ts'
import { getSession, sessionCookie, sleep } from '../support/auth.ts'
import {
  ATMOSPHERECONF,
  Attendee,
  addAdmin,
  allowAppForOrg,
  cli,
  cliOk,
  cliOkJson,
  connect,
  connectOk,
  createConference,
  createOrg,
  type Deployment,
  deploy,
  memberAction,
  publicConference,
  setMethods,
  WEDDING,
} from '../support/conference.ts'
import { P256Key } from '../support/crypto.ts'
import { OtherApp } from '../support/other-app.ts'

// Seeding an organization takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

// Organizations, admins and conferences, set up through the admin CLI.

let dep: Deployment
beforeAll(async () => {
  dep = await deploy()
})
afterAll(async () => {
  await dep?.stop()
})

const EVENT = 'community.lexicon.calendar.event'

test('TC-4: Creating a public conference publishes an event other calendar apps can read', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const org = await createOrg(dep, olga)
  const conference = await createConference(dep, org, ATMOSPHERECONF)

  expect(conference.event, 'a public conference has a public event').toMatch(/^at:\/\//)
  const event = parseAtUri(conference.event as string)
  expect(event.repo).toBe(olga.did)
  expect(event.collection).toBe(EVENT)

  // Read the way any calendar app reads it: Olga's public repo.
  const record = (await publicRecords(viv.url, olga.did, EVENT)).find((r) => r.uri === conference.event)
  expect(record, 'the event should be in Olga’s repository').toBeDefined()
  expect(record?.value.name).toBe('AtmosphereConf')
  expect(new Date(record?.value.startsAt).toISOString()).toMatch(/^2027-04-29/)
  expect(new Date(record?.value.endsAt).toISOString()).toMatch(/^2027-05-02/)
  expect(JSON.stringify(record?.value.locations)).toContain('Amsterdam')

  const settings = (await publicRecords(viv.url, olga.did, 'app.eventside.conference')).find(
    (r) => r.value.space === conference.space,
  )
  expect(settings, 'a settings record should name the conference’s space').toBeDefined()
  expect(settings?.value.superAdmin).toBe(olga.did)

  // Anyone, signed out, opens it by the event's link.
  const page = await publicConference(dep, conference.event as string)
  expect(page.status).toBe(200)
  expect(page.body.name).toBe('AtmosphereConf')
})

test('TC-5: A conference can adopt an event the organization already published', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const org = await createOrg(dep, olga)
  // Published through another calendar app, before eventside was involved.
  const published = await createPublicRecord(viv.url, olga, EVENT, {
    name: 'AtmosphereConf 2027',
    startsAt: '2027-04-29T09:00:00+02:00',
    endsAt: '2027-05-02T18:00:00+02:00',
    createdAt: new Date().toISOString(),
  })

  const conference = await createConference(dep, org, { ...ATMOSPHERECONF, event: published.uri })
  expect(conference.event).toBe(published.uri)
  expect(await publicRecords(viv.url, olga.did, EVENT), 'no second event').toHaveLength(1)

  const page = await publicConference(dep, published.uri)
  expect(page.status).toBe(200)
  expect(page.body.name).toBe('AtmosphereConf 2027')
  expect(page.body.space).toBe(conference.space)
})

test('TC-6: A conference can’t adopt someone else’s event', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const org = await createOrg(dep, olga)
  const theirs = await createPublicRecord(viv.url, mallory, EVENT, {
    name: 'AtmosphereConf (not really)',
    startsAt: '2027-04-29T09:00:00+02:00',
    createdAt: new Date().toISOString(),
  })

  const result = await cli(dep, ['conference', 'create', '--org', org.did, '--event', theirs.uri])
  expect(result.code, 'the CLI should refuse').not.toBe(0)
  expect(result.stderr).toMatch(/super admin.*own repo/i)
  expect(await publicRecords(viv.url, olga.did, 'app.eventside.conference')).toHaveLength(0)
  expect(await publicRecords(viv.url, mallory.did, 'app.eventside.conference')).toHaveLength(0)
})

test('TC-7: An invite-only conference publishes nothing', async ({ viv }) => {
  const sanne = await viv.createAccount(viv.handle('sanne'))
  const mallory = await viv.createAccount(viv.handle('mallory'))
  const bruiloft = await createOrg(dep, sanne)
  const wedding = await createConference(dep, bruiloft, WEDDING)

  expect(wedding.event).toBeUndefined()
  expect(await publicRecords(viv.url, sanne.did, EVENT)).toHaveLength(0)
  expect(await publicRecords(viv.url, sanne.did, 'app.eventside.conference')).toHaveLength(0)

  // By its address, signed out or as a non-member: nothing about it.
  const signedOut = await publicConference(dep, wedding.space)
  expect(signedOut.status).toBe(404)
  expect(JSON.stringify(signedOut.body)).not.toContain('wedding')
  const asMallory = await (await Attendee.signIn(dep, mallory)).getConference(wedding.space)
  expect(asMallory.status).toBe(404)
  expect(JSON.stringify(asMallory.body)).not.toContain('wedding')
})

test('TC-8: The last owner can’t be removed', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const org = await createOrg(dep, olga)

  const refused = await cli(dep, ['org', 'admin', 'remove', olga.handle, '--org', org.did])
  expect(refused.code, 'the CLI should refuse to remove the last owner').not.toBe(0)
  expect(refused.stderr).toMatch(/last owner/i)
  let shown = await cliOkJson(dep, ['org', 'show', '--org', org.did])
  expect(shown.admins).toContainEqual(expect.objectContaining({ did: olga.did, role: 'owner' }))

  await addAdmin(dep, org, pim, 'owner')
  await cliOk(dep, ['org', 'admin', 'remove', olga.handle, '--org', org.did])
  shown = await cliOkJson(dep, ['org', 'show', '--org', org.did])
  expect(shown.admins).not.toContainEqual(expect.objectContaining({ did: olga.did, role: 'owner' }))
  expect(shown.admins).toContainEqual(expect.objectContaining({ did: pim.did, role: 'owner' }))
})

test('TC-43: Connecting as an admin leaves no way to act as an admin in the browser', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const recoveryKey = P256Key.generate().didKey()
  const { did } = await cliOkJson(dep, ['org', 'create', '--super-admin', olga.handle, '--recovery-key', recoveryKey])

  const connected = await connect(dep, olga)
  expect(connected.cli.code, connected.cli.stderr).toBe(0)
  expect(connected.page.res.status).toBe(200)
  expect(connected.page.text).toMatch(/connected, you can close this tab/i)

  // The browser that completed sign-in isn't signed in to the app as Olga.
  expect(sessionCookie(dep.url, connected.page.jar)).toBeUndefined()
  const session = await getSession(dep.url, connected.page.jar)
  expect(session.status).toBe(401)
  expect(session.body.did).toBeUndefined()

  const shown = await cliOkJson(dep, ['org', 'show', '--org', did])
  expect(shown.admins).toContainEqual(expect.objectContaining({ did: olga.did, role: 'owner', connected: true }))
})

test('TC-44: A connected admin stays connected while idle', async ({ viv }) => {
  // An attendee idle timeout short enough to wait out, with the renewer running often.
  const idle = await deploy({ SESSION_IDLE_TIMEOUT: '2s', TOKEN_RENEW_INTERVAL: '1s' })
  try {
    const olga = await viv.createAccount(viv.handle('olga'))
    const org = await createOrg(idle, olga)
    await sleep(4_000)
    const conference = await createConference(idle, org, ATMOSPHERECONF)
    expect(conference.event).toMatch(/^at:\/\//)
    expect((await publicRecords(viv.url, olga.did, EVENT)).map((r) => r.uri)).toContain(conference.event)
  } finally {
    await idle.stop()
  }
})

// What admins were asked for before the admin permissions grew: their own
// admin-space records, and nothing else.
const OLD_ADMIN_SCOPES =
  'atproto space:app.eventside.admin?authority=*&action=read&action=create&action=update&action=delete'

test('TC-45: An admin missing newly required permissions is told to reconnect', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const pim = await viv.createAccount(viv.handle('pim'))
  const bram = await viv.createAccount(viv.handle('bram'))
  const kees = await viv.createAccount(viv.handle('kees'))
  const org = await createOrg(dep, olga)
  await cliOk(dep, ['org', 'admin', 'add', pim.handle, '--org', org.did, '--role', 'staff'])
  const conference = await createConference(dep, org, ATMOSPHERECONF)
  await setMethods(dep, conference, ['request'])

  // Pim connected, and approved Bram, before the admin permissions grew.
  const old = { ADMIN_OAUTH_SCOPES: OLD_ADMIN_SCOPES }
  await connectOk(dep, pim, { env: old })
  const asBram = await Attendee.signIn(dep, bram)
  expect((await asBram.join({ conference: conference.space })).body.status).toBe('pending')
  const approved = await memberAction(dep, conference, ['requests', 'approve'], bram, { as: pim, env: old })
  expect(approved.code, approved.stderr).toBe(0)
  expect(await asBram.isMember(conference.space)).toBe(true)

  // Now they have grown.
  const asKees = await Attendee.signIn(dep, kees)
  expect((await asKees.join({ conference: conference.space })).body.status).toBe('pending')
  const refused = await memberAction(dep, conference, ['requests', 'approve'], kees, { as: pim })
  expect(refused.code).not.toBe(0)
  const said = refused.stderr + refused.stdout
  expect(said).toMatch(/reconnect/i)
  expect(said).toContain(`connect ${pim.handle}`)
  expect(await asKees.isMember(conference.space)).toBe(false)

  // What Pim already wrote still counts.
  expect(await asBram.isMember(conference.space)).toBe(true)
})

test('TC-46: Creating an organization gives it an identity that points at our server', async ({ viv }) => {
  const olga = await viv.createAccount(viv.handle('olga'))
  const recoveryKey = P256Key.generate().didKey()
  const { did } = await cliOkJson(dep, ['org', 'create', '--super-admin', olga.handle, '--recovery-key', recoveryKey])
  expect(did).toMatch(/^did:plc:/)

  const doc = await didDocument(viv.url, did)
  expect(serviceEndpoint(doc, 'atproto_space_host')).toBe(dep.url)
  const host = (doc.service ?? []).find((s: { id: string }) => s.id.endsWith('#atproto_space_host'))
  expect(host?.type).toBe('AtprotoSpaceHost')
  const spaceKey = (doc.verificationMethod ?? []).find((m: { id: string }) => m.id.endsWith('#atproto_space'))
  expect(spaceKey?.publicKeyMultibase, 'a space key').toBeTypeOf('string')

  const data = await plcData(viv.url, did)
  expect(data.rotationKeys.length).toBeGreaterThanOrEqual(2)
  expect(data.rotationKeys[0], 'the operator’s recovery key comes first').toBe(recoveryKey)

  // Olga connects; the admin space exists, with her as owner, written by her.
  await connectOk(dep, olga)
  const adminSpace = `at://${did}/space/app.eventside.admin/self`
  const admins = await ownSpaceRecords(viv.url, olga, adminSpace, 'app.eventside.admin.admin')
  expect(admins.map((r) => r.value)).toContainEqual(expect.objectContaining({ subject: olga.did, role: 'owner' }))

  // Our server hosts it: an app Olga uses can get into it.
  const app = await OtherApp.start(viv.url)
  try {
    await allowAppForOrg(dep, { did, superAdmin: olga, adminSpace }, app.clientId)
    const access = await (await app.signIn(olga)).open(adminSpace)
    expect(await access.writers()).toContain(olga.did)
  } finally {
    await app.stop()
  }
})
