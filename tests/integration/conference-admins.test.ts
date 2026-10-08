import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import { createAccountWithEmail, type Json } from '../support/atproto.ts'
import {
  Attendee,
  accountsIn,
  addAdmin,
  allowApp,
  cli,
  cliOk,
  cliOkJson,
  type Deployment,
  deploy,
  importList,
  memberAction,
  seedConference,
  setMethods,
  sharedCode,
  uniqueCode,
} from '../support/conference.ts'
import { OtherApp } from '../support/other-app.ts'

// Seeding an organization takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

// Permissions as records in the admin space: rebuilt by crawling from the
// super admin, and weighed by who wrote them.

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

/** Members sorted by DID, so two listings compare equal whatever their order. */
const sorted = (members: Json[]) => [...members].sort((a, b) => a.did.localeCompare(b.did))

test('TC-50: Every permission can be rebuilt from the super admin', async ({ viv }) => {
  const { org, conference, superAdmin, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim'],
    methods: ['code', 'list', 'request'],
  })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'), { maxUses: 20 })
  await allowApp(dep, conference, app.clientId)
  const person = (name: string) => viv.createAccount(viv.handle(name))
  const ana = await person('ana')
  const femke = await createAccountWithEmail(viv.url, viv.handle('femke'), `${viv.handle('femke')}@example.com`)
  viv.track(femke.did)
  await importList(dep, conference, [{ handle: ana.handle }, { email: `${femke.handle}@example.com` }])

  // Members who joined each way.
  const asAna = await Attendee.signIn(dep, ana)
  expect((await asAna.join({ conference: conference.space })).body.status, 'by the list').toBe('joined')
  const lena = await Attendee.signIn(dep, await person('lena'))
  expect((await lena.join({ conference: conference.space, code })).body.status, 'by a code').toBe('joined')
  const kees = await Attendee.signIn(dep, await person('kees'))
  expect((await kees.join({ conference: conference.space, request: true })).body.status).toBe('pending')
  expect((await memberAction(dep, conference, ['requests', 'approve'], kees.person, { as: admins.pim })).code).toBe(0)
  const asFemke = await Attendee.signIn(dep, femke)
  const emailNeeded = await asFemke.join({ conference: conference.space })
  expect(emailNeeded.body.status).toBe('emailNeeded')
  await asFemke.verifyEmail(emailNeeded.body.verifyUrl)
  expect(await asFemke.isMember(conference.space), 'by a verified email').toBe(true)
  await setMethods(dep, conference, ['code', 'list', 'request', 'open'])
  const joost = await Attendee.signIn(dep, await person('joost'))
  expect((await joost.join({ conference: conference.space })).body.status, 'while open').toBe('joined')
  await setMethods(dep, conference, ['code', 'list', 'request'])
  // A removal, and a ban.
  const ruud = await Attendee.signIn(dep, await person('ruud'))
  expect((await ruud.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'remove'], ruud.person)).code).toBe(0)
  const bram = await Attendee.signIn(dep, await person('bram'))
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'ban'], bram.person)).code).toBe(0)

  const snapshot = async () => {
    const access = await (await app.signIn(superAdmin)).open(conference.space)
    return {
      members: sorted(await access.members()),
      space: await access.getSpace(),
      org: await cliOkJson(dep, ['org', 'show', '--org', org.did]),
      requests: await cliOkJson(dep, ['requests', 'list', '--conference', conference.space]),
    }
  }
  const before = await snapshot()

  await cliOk(dep, ['reindex', '--org', org.did])

  expect(await snapshot()).toEqual(before)
  // The ban and the code survived too.
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('refused')
  const nina = await Attendee.signIn(dep, await person('nina'))
  expect((await nina.join({ conference: conference.space, code })).body.status).toBe('joined')
  // Joining is closed again, as it was.
  const latecomer = await Attendee.signIn(dep, await person('latecomer'))
  expect((await latecomer.join({ conference: conference.space })).body.status).not.toBe('joined')
})

test('TC-51: Staff can’t override the super admin', async ({ viv }) => {
  const { conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'ban'], bram.person)).code, 'Olga bans Bram').toBe(0)

  // Pim admits him, and tries to open the conference: refused or ignored.
  await memberAction(dep, conference, ['member', 'add'], bram.person, { as: admins.pim })
  await cli(dep, ['join', 'set', '--conference', conference.space, '--methods', 'code,open', '--as', admins.pim.handle])

  expect(await bram.isMember(conference.space)).toBe(false)
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('refused')
  const mallory = await Attendee.signIn(dep, await viv.createAccount(viv.handle('mallory')))
  expect((await mallory.join({ conference: conference.space })).body.status, 'not open').not.toBe('joined')
  expect(await mallory.isMember(conference.space)).toBe(false)
})

test('TC-52: Staff can’t override an owner’s decision', async ({ viv }) => {
  // Pim and Lotte are staff, Kees an owner.
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), {
    staff: ['pim', 'lotte'],
    methods: ['code'],
  })
  const kees = await viv.createAccount(viv.handle('kees'))
  await addAdmin(dep, org, kees, 'owner')
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  const joost = await Attendee.signIn(dep, await viv.createAccount(viv.handle('joost')))
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')

  expect((await memberAction(dep, conference, ['member', 'add'], bram.person, { as: admins.pim })).code).toBe(0)
  expect(await bram.isMember(conference.space)).toBe(true)
  expect((await memberAction(dep, conference, ['member', 'ban'], bram.person, { as: kees })).code).toBe(0)
  expect(await bram.isMember(conference.space), 'the owner’s ban stands').toBe(false)
  expect((await bram.join({ conference: conference.space, code })).body.status).toBe('refused')

  expect((await memberAction(dep, conference, ['member', 'remove'], ana.person, { as: kees })).code).toBe(0)
  expect(await ana.isMember(conference.space)).toBe(false)
  await memberAction(dep, conference, ['member', 'add'], ana.person, { as: admins.pim })
  expect(await ana.isMember(conference.space), 'staff can’t override the owner’s removal').toBe(false)
  expect((await memberAction(dep, conference, ['member', 'add'], ana.person, { as: kees })).code).toBe(0)
  expect(await ana.isMember(conference.space), 'the owner re-admits her').toBe(true)

  // Joost got in with a code: no admin decided about him.
  expect((await joost.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'remove'], joost.person, { as: admins.lotte })).code).toBe(0)
  expect(await joost.isMember(conference.space)).toBe(false)
  expect((await memberAction(dep, conference, ['member', 'add'], joost.person, { as: admins.pim })).code).toBe(0)
  expect(await joost.isMember(conference.space), 'between staff, the latest decision stands').toBe(true)
})

test('TC-53: A former admin’s decisions keep standing', async ({ viv }) => {
  const { org, conference, admins } = await seedConference(dep, accountsIn(viv), { staff: ['pim'], methods: ['code'] })
  const code = await sharedCode(dep, conference, uniqueCode('atmosphere27'))
  const bram = await Attendee.signIn(dep, await viv.createAccount(viv.handle('bram')))
  const ana = await Attendee.signIn(dep, await viv.createAccount(viv.handle('ana')))
  const cas = await Attendee.signIn(dep, await viv.createAccount(viv.handle('cas')))
  // Pim admits Bram; Ana has her own way in as well as Pim's say-so.
  expect((await memberAction(dep, conference, ['member', 'add'], bram.person, { as: admins.pim })).code).toBe(0)
  expect((await ana.join({ conference: conference.space, code })).body.status).toBe('joined')
  expect((await memberAction(dep, conference, ['member', 'add'], ana.person, { as: admins.pim })).code).toBe(0)
  expect(await bram.isMember(conference.space)).toBe(true)

  await cliOk(dep, ['org', 'admin', 'remove', admins.pim.handle, '--org', org.did])

  expect(await bram.isMember(conference.space), 'Pim’s admission of Bram still stands').toBe(true)
  expect(await ana.isMember(conference.space), 'Ana is still a member').toBe(true)
  // Pim is no longer an admin: the CLI won't sign anything more for him.
  const refused = await memberAction(dep, conference, ['member', 'add'], cas.person, { as: admins.pim })
  expect(refused.code, 'a decision as a former admin is refused').not.toBe(0)
  expect(await cas.isMember(conference.space)).toBe(false)
  const removal = await memberAction(dep, conference, ['member', 'remove'], bram.person, { as: admins.pim })
  expect(removal.code, 'so is a removal').not.toBe(0)
  expect(await bram.isMember(conference.space)).toBe(true)
})
