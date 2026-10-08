import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import {
  type Account,
  didDocument,
  type Json,
  ownSpaceRecords,
  pdsOf,
  writeSpaceRecord,
  xrpcOk,
} from '../support/atproto.ts'
import { checkSignatures, validKeys } from '../support/attestation.ts'
import {
  Attendee,
  accountsIn,
  addAdmin,
  allowApp,
  allowAppForOrg,
  cli,
  cliJson,
  cliOk,
  cliOkJson,
  type Deployment,
  deploy,
  memberAction,
  NSID,
  type Org,
  seedConference,
  setMethods,
  sharedCode,
  uniqueCode,
} from '../support/conference.ts'
import { isCurrent, memberEntry, OtherApp } from '../support/other-app.ts'

// Seeding an organization takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

// Signed decisions (design review round 5): our server checks each
// permission record and join when it's taken, then signs it with the
// authority's `#eventside_attest*` key. Readers count only what verifies.

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

const MEMBER = 'app.eventside.admin.member'

/** The `#eventside_attest*` key fragments in the authority's DID document now. */
async function attestKeys(org: Org): Promise<string[]> {
  const doc = await didDocument(process.env.VIVARIUM_URL as string, org.did)
  return ((doc.verificationMethod ?? []) as Json[])
    .map((method) => String(method.id).split('#')[1])
    .filter((fragment) => fragment?.startsWith('eventside_attest'))
    .sort()
}

/** An admin's `member` record admitting a person, from the admin's own repo in the admin space. */
async function admissionBy(org: Org, admin: Account, subject: string): Promise<Json> {
  const records = await ownSpaceRecords(process.env.VIVARIUM_URL as string, admin, org.adminSpace, MEMBER)
  const found = records.find((r) => r.value.subject === subject && r.value.via !== 'removed' && !r.value.until)
  if (!found) throw new Error(`no member record by ${admin.handle} for ${subject}: ${JSON.stringify(records)}`)
  return found
}

/** Replaces a record in the account's own repo in a space, from its own PDS. */
async function putSpaceRecord(account: Account, space: string, collection: string, rkey: string, record: Json) {
  const vivariumUrl = process.env.VIVARIUM_URL as string
  await xrpcOk(await pdsOf(vivariumUrl, account.did), 'com.atproto.space.putRecord', {
    token: account.accessJwt,
    body: { space, repo: account.did, collection, rkey, record },
  })
}

test('TC-59: An admin record without our signature doesn’t count', async ({ viv }) => {
  const { org, conference, superAdmin, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code'],
  })
  await allowApp(dep, conference, app.clientId)
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))

  // Pim writes an admission straight to his PDS, as another app would: no signature.
  await writeSpaceRecord(viv.url, admins.pim, org.adminSpace, MEMBER, {
    space: conference.space,
    subject: mallory.did,
    via: 'admin',
    since: new Date().toISOString(),
  })
  // Our host reads it, from Pim's notice and again by crawling.
  await cliOk(dep, ['reindex', '--org', org.did])

  expect(await mallory.isMember(conference.space), 'Mallory isn’t a member').toBe(false)
  const access = await (await app.signIn(superAdmin)).open(conference.space)
  const entry = memberEntry(await access.members(), mallory.did)
  expect(isCurrent(entry), `our host doesn’t list her: ${JSON.stringify(entry)}`).toBe(false)
})

test('TC-60: A decision keeps its rank after its author is demoted', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, kees, 'owner')
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'ban'], bram.person, { as: kees })).code).toBe(0)
  // Kees is made staff.
  await cliOk(dep, ['org', 'admin', 'add', kees.handle, '--org', org.did, '--role', 'staff'])

  const byPim = await memberAction(dep, conference, ['member', 'add'], bram.person, { as: admins.pim })
  expect(byPim.code, 'staff can’t admit someone an owner banned').not.toBe(0)
  expect(await bram.isMember(conference.space), 'Bram is still banned').toBe(false)
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('refused')

  const byKees = await memberAction(dep, conference, ['member', 'add'], bram.person, { as: kees })
  expect(byKees.code, 'Kees, now staff, can’t lift his own ban').not.toBe(0)
  expect(await bram.isMember(conference.space)).toBe(false)
})

test('TC-61: Another app can check a decision for itself', async ({ viv }) => {
  const { org, conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const bram = await viv.createAccount(viv.handle('bram'))
  expect((await memberAction(dep, conference, ['member', 'add'], bram)).code, 'Olga admits Bram').toBe(0)
  await allowAppForOrg(dep, org, app.clientId)

  // Another app reads Olga's admission from the admin space, and checks it itself.
  const access = await (await app.signIn(superAdmin)).open(org.adminSpace)
  const records = await access.records(superAdmin.did, MEMBER)
  const admission = records.find((r) => r.value.subject === bram.did && r.value.space === conference.space)
  expect(admission, 'Olga’s admission of Bram').toBeDefined()
  const authorityDoc = await didDocument(viv.url, org.did)
  const checks = checkSignatures(admission?.value, { repository: superAdmin.did, space: org.adminSpace, authorityDoc })
  expect(
    checks.some((check) => check.valid),
    `a signature verifies against Atmosphere’s DID document: ${JSON.stringify(checks)}`,
  ).toBe(true)

  // The same record, copied into Mallory's repository, doesn't.
  const mallory = await viv.createAccount(viv.handle('mallory'))
  await writeSpaceRecord(viv.url, mallory, org.adminSpace, MEMBER, admission?.value)
  const [copy] = await ownSpaceRecords(viv.url, mallory, org.adminSpace, MEMBER)
  expect(copy.value.signatures).toEqual(admission?.value.signatures)
  const copied = checkSignatures(copy.value, { repository: mallory.did, space: org.adminSpace, authorityDoc })
  expect(
    copied.some((check) => check.valid),
    `the copy doesn’t verify: ${JSON.stringify(copied)}`,
  ).toBe(false)
})

test('TC-63: Rotating the signing key keeps earlier decisions', async ({ viv }) => {
  const { org, conference, superAdmin } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  const dirk = await Attendee.signIn(dep, await viv.createAccount(viv.handle('dirk')))
  const cas = await Attendee.signIn(dep, await viv.createAccount(viv.handle('cas')))
  for (const person of [bram, dirk]) {
    expect((await memberAction(dep, conference, ['member', 'add'], person.person)).code).toBe(0)
  }
  const signedWith = async (did: string) =>
    validKeys((await admissionBy(org, superAdmin, did)).value, {
      repository: superAdmin.did,
      space: org.adminSpace,
      authorityDoc: await didDocument(viv.url, org.did),
    })
  const first = `${org.did}#eventside_attest`
  const second = `${org.did}#eventside_attest_2`
  expect(await attestKeys(org)).toEqual(['eventside_attest'])
  expect(await signedWith(bram.did)).toEqual([first])
  // Dirk's admission as it stands now, signed only with the first key.
  const dirkBefore = await admissionBy(org, superAdmin, dirk.did)

  // The operator adds a new key, and re-signs the standing records.
  const added = await cliOkJson(dep, ['org', 'keys', 'add', '--org', org.did])
  expect(added.key).toBe(second)
  expect(await attestKeys(org)).toEqual(['eventside_attest', 'eventside_attest_2'])
  await cliOk(dep, ['org', 'keys', 'resign', '--org', org.did])

  expect(await bram.isMember(conference.space), 'Bram is still a member').toBe(true)
  expect(await signedWith(bram.did), 'his admission is signed with both keys').toEqual([first, second])
  expect((await memberAction(dep, conference, ['member', 'add'], cas.person)).code).toBe(0)
  expect(await signedWith(cas.did), 'a new admission is signed with the new key').toContain(second)

  // The operator removes the old key.
  await cliOk(dep, ['org', 'keys', 'remove', 'eventside_attest', '--org', org.did])
  expect(await attestKeys(org)).toEqual(['eventside_attest_2'])
  await cliOk(dep, ['reindex', '--org', org.did])
  expect(await bram.isMember(conference.space), 'Bram is still a member').toBe(true)
  expect(await dirk.isMember(conference.space), 'so is Dirk, re-signed').toBe(true)

  // A record signed only with the old key no longer counts: Dirk's admission, put back as it was.
  const rkey = String(dirkBefore.uri).split('/').at(-1) as string
  await putSpaceRecord(superAdmin, org.adminSpace, MEMBER, rkey, dirkBefore.value)
  await cliOk(dep, ['reindex', '--org', org.did])
  expect(await dirk.isMember(conference.space), 'Dirk’s old-key admission doesn’t count').toBe(false)
  expect(await bram.isMember(conference.space)).toBe(true)
})

test('TC-64: A join written by another app is only a request', async ({ viv }) => {
  const { org, conference } = await seedConference(dep, accountsIn(viv), { methods: ['code', 'request'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'), { maxUses: 20 })
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  const pendingRequests = async () =>
    (await cliOkJson(dep, ['requests', 'list', '--conference', conference.space])).requests.map(
      (r: { did: string }) => r.did,
    )

  // Ana writes a join with the code straight to her PDS: no signature.
  await writeSpaceRecord(viv.url, ana.person, conference.intake, NSID.join, { code })
  await cliOk(dep, ['reindex', '--org', org.did])

  const viewer = (await ana.getConference(conference.space)).body.viewer
  expect(viewer, 'her request is pending, and she isn’t a member').toEqual({ member: false, request: 'pending' })
  expect(await pendingRequests()).toContain(ana.did)

  // With requests off, the same join simply doesn't let her in.
  await setMethods(dep, conference, ['code'])
  const lena = await Attendee.signIn(dep, await viv.createAccount(viv.handle('lena')))
  await writeSpaceRecord(viv.url, lena.person, conference.intake, NSID.join, { code })
  await cliOk(dep, ['reindex', '--org', org.did])
  expect((await lena.getConference(conference.space)).body.viewer).toEqual({ member: false })
  expect(await pendingRequests()).not.toContain(lena.did)
})

test('TC-65: Staff can issue codes', async ({ viv }) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })

  const issued = await cli(dep, [
    'codes',
    'issue',
    '--conference',
    conference.space,
    '--shared',
    uniqueCode('atmosphere27'),
    '--as',
    admins.pim.handle,
    '--json',
  ])
  expect(issued.code, issued.stderr).toBe(0)
  const code = cliJson(issued).codes[0]

  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect(await bram.isMember(conference.space)).toBe(true)
})
