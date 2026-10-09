import { afterAll, beforeAll, expect, test, vi } from '@vivarium-dev/client/vitest'
import type { Account } from '../support/atproto.ts'
import {
  Attendee,
  accountsIn,
  type Deployment,
  deploy,
  joinWithCode,
  member,
  memberRecord,
  removeAdmin,
  type Seeded,
  seedConference,
  setRole,
  sharedCode,
} from '../support/conference.ts'

// Who can undo whom: the precedence table of design review round 1, through
// the CLI acting as each admin.

// Seeding takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 180_000, hookTimeout: 60_000 })

let dep: Deployment
beforeAll(async () => {
  dep = await deploy()
})
afterAll(async () => {
  await dep?.stop()
})

/** An admin the seed added. */
const admin = (seeded: Seeded, name: 'kees' | 'pim' | 'lotte'): Account => {
  const person = seeded.admins[name]
  if (!person) throw new Error(`the seed has no ${name}`)
  return person
}

/** Seeds AtmosphereConf with the shared code "atmosphere27". */
async function seed(viv: Parameters<typeof accountsIn>[0], opts: Parameters<typeof seedConference>[2] = {}) {
  const seeded = await seedConference(dep, accountsIn(viv), { methods: ['code'], ...opts })
  await sharedCode(dep, seeded.conference, 'atmosphere27', seeded.olga)
  return seeded
}

test('TC-28: Staff can tighten an owner’s admission', async ({ viv }) => {
  const seeded = await seed(viv, { staff: ['pim'] })
  const { conference, olga } = seeded
  const pim = admin(seeded, 'pim')
  const joost = await viv.createAccount(viv.handle('joost'))
  const asJoost = await Attendee.signIn(dep, joost)

  // Olga admits Joost, and Pim removes him.
  expect((await member(dep, conference, 'add', joost, olga)).code).toBe(0)
  expect(await asJoost.isMember(conference)).toBe(true)
  const removed = await member(dep, conference, 'remove', joost, pim)
  expect(removed.code, removed.stderr).toBe(0)
  expect(await asJoost.isMember(conference)).toBe(false)

  // Olga admits him again, and Pim bans him.
  expect((await member(dep, conference, 'add', joost, olga)).code).toBe(0)
  expect(await asJoost.isMember(conference)).toBe(true)
  const banned = await member(dep, conference, 'ban', joost, pim)
  expect(banned.code, banned.stderr).toBe(0)
  expect(await asJoost.isMember(conference)).toBe(false)
  expect((await asJoost.join(conference, 'atmosphere27')).body.status).toBe('refused')
})

test('TC-29: Staff can’t loosen an owner’s removal or ban', async ({ viv }) => {
  const seeded = await seed(viv, { owners: ['kees'], staff: ['pim'] })
  const { conference, olga } = seeded
  const pim = admin(seeded, 'pim')
  const kees = admin(seeded, 'kees')
  const ana = await viv.createAccount(viv.handle('ana'))
  const bram = await viv.createAccount(viv.handle('bram'))
  const asAna = await joinWithCode(dep, ana, conference, 'atmosphere27')
  const asBram = await joinWithCode(dep, bram, conference, 'atmosphere27')
  expect((await member(dep, conference, 'remove', ana, olga)).code).toBe(0)
  expect((await member(dep, conference, 'ban', bram, olga)).code).toBe(0)

  // Pim can't admit Ana, or lift Bram's ban…
  expect((await member(dep, conference, 'add', ana, pim)).code, 'Pim admits Ana').not.toBe(0)
  expect((await member(dep, conference, 'unban', bram, pim)).code, 'Pim lifts Bram’s ban').not.toBe(0)
  // …and nothing changed.
  expect(await asAna.isMember(conference)).toBe(false)
  expect((await asBram.join(conference, 'atmosphere27')).body.status).toBe('refused')

  // Kees, an owner, can do either.
  const admitted = await member(dep, conference, 'add', ana, kees)
  expect(admitted.code, admitted.stderr).toBe(0)
  expect(await asAna.isMember(conference)).toBe(true)
  const lifted = await member(dep, conference, 'unban', bram, kees)
  expect(lifted.code, lifted.stderr).toBe(0)
  expect((await asBram.join(conference, 'atmosphere27')).body.status).toBe('joined')
})

test('TC-30: Rejoining after a removal depends on who removed you', async ({ viv }) => {
  const seeded = await seed(viv, { staff: ['pim'] })
  const { conference, olga } = seeded
  const pim = admin(seeded, 'pim')
  const ruud = await viv.createAccount(viv.handle('ruud'))
  const joost = await viv.createAccount(viv.handle('joost'))
  const asRuud = await joinWithCode(dep, ruud, conference, 'atmosphere27')
  const asJoost = await joinWithCode(dep, joost, conference, 'atmosphere27')
  expect((await member(dep, conference, 'remove', ruud, pim)).code).toBe(0)
  expect((await member(dep, conference, 'remove', joost, olga)).code).toBe(0)

  // Removed by staff: Ruud gets back in with the code.
  expect((await asRuud.join(conference, 'atmosphere27')).body.status).toBe('joined')
  expect(await asRuud.isMember(conference)).toBe(true)

  // Removed by an owner: Joost is refused…
  expect((await asJoost.join(conference, 'atmosphere27')).body.status).toBe('refused')
  expect(await asJoost.isMember(conference)).toBe(false)
  // …until an owner admits him.
  expect((await member(dep, conference, 'add', joost, olga)).code).toBe(0)
  expect(await asJoost.isMember(conference)).toBe(true)
})

test('TC-31: Only owners act on admins', async ({ viv }) => {
  const seeded = await seed(viv, { owners: ['kees'], staff: ['pim', 'lotte'] })
  const { conference, olga } = seeded
  const pim = admin(seeded, 'pim')
  const kees = admin(seeded, 'kees')
  const lotte = admin(seeded, 'lotte')

  expect((await member(dep, conference, 'remove', lotte, pim)).code, 'Pim removes Lotte').not.toBe(0)
  expect((await member(dep, conference, 'ban', kees, pim)).code, 'Pim bans Kees').not.toBe(0)
  expect((await setRole(dep, conference, pim, 'owner', pim)).code, 'Pim makes himself an owner').not.toBe(0)
  const asLotte = await Attendee.signIn(dep, lotte)
  const asKees = await Attendee.signIn(dep, kees)
  const asPim = await Attendee.signIn(dep, pim)
  expect(await asLotte.role(conference)).toBe('staff')
  expect(await asKees.role(conference)).toBe('owner')
  expect(await asPim.role(conference)).toBe('staff')

  const removed = await member(dep, conference, 'remove', lotte, olga)
  expect(removed.code, removed.stderr).toBe(0)
  expect(await asLotte.isMember(conference)).toBe(false)
})

test('TC-32: An owner demoted to staff keeps the weight of their owner decisions', async ({ viv }) => {
  const seeded = await seed(viv, { owners: ['kees'], staff: ['pim'] })
  const { conference, olga } = seeded
  const kees = admin(seeded, 'kees')
  const pim = admin(seeded, 'pim')
  const bram = await viv.createAccount(viv.handle('bram'))
  const asBram = await joinWithCode(dep, bram, conference, 'atmosphere27')
  expect((await member(dep, conference, 'ban', bram, kees)).code).toBe(0)

  const demoted = await setRole(dep, conference, kees, 'staff', olga)
  expect(demoted.code, demoted.stderr).toBe(0)
  expect(await (await Attendee.signIn(dep, kees)).role(conference)).toBe('staff')

  // The ban needs an owner to lift it: not Kees, not Pim.
  expect((await member(dep, conference, 'unban', bram, kees)).code, 'Kees lifts his own ban').not.toBe(0)
  expect((await member(dep, conference, 'unban', bram, pim)).code, 'Pim lifts it').not.toBe(0)
  expect((await asBram.join(conference, 'atmosphere27')).body.status).toBe('refused')

  // Olga, an owner, can.
  expect((await member(dep, conference, 'unban', bram, olga)).code).toBe(0)
  expect((await asBram.join(conference, 'atmosphere27')).body.status).toBe('joined')
})

test('TC-33: A former admin’s decisions stand', async ({ viv }) => {
  const seeded = await seed(viv, { staff: ['pim'] })
  const { conference, olga } = seeded
  const pim = admin(seeded, 'pim')
  const joost = await viv.createAccount(viv.handle('joost'))
  const ana = await viv.createAccount(viv.handle('ana'))
  expect((await member(dep, conference, 'add', joost, pim)).code).toBe(0)

  const removed = await removeAdmin(dep, conference, pim, olga)
  expect(removed.code, removed.stderr).toBe(0)

  // Joost is still a member…
  const asJoost = await Attendee.signIn(dep, joost)
  expect(await asJoost.isMember(conference)).toBe(true)
  // …and Pim can no longer decide anything.
  expect((await member(dep, conference, 'add', ana, pim)).code, 'Pim admits Ana').not.toBe(0)
  expect((await member(dep, conference, 'remove', joost, pim)).code, 'Pim removes Joost').not.toBe(0)
  expect(await (await Attendee.signIn(dep, ana)).isMember(conference)).toBe(false)
  expect(await asJoost.isMember(conference)).toBe(true)
})

test('TC-34: Between staff, the latest decision stands', async ({ viv }) => {
  const seeded = await seed(viv, { staff: ['pim', 'lotte'] })
  const { conference } = seeded
  const ana = await viv.createAccount(viv.handle('ana'))
  const asAna = await joinWithCode(dep, ana, conference, 'atmosphere27')

  expect((await member(dep, conference, 'remove', ana, admin(seeded, 'lotte'))).code).toBe(0)
  expect(await asAna.isMember(conference)).toBe(false)
  const admitted = await member(dep, conference, 'add', ana, admin(seeded, 'pim'))
  expect(admitted.code, admitted.stderr).toBe(0)
  expect(await asAna.isMember(conference)).toBe(true)
})

test('TC-35: Decisions made at the same moment are applied in order', async ({ viv }) => {
  const { conference, olga } = await seed(viv)
  const ana = await viv.createAccount(viv.handle('ana'))
  const asAna = await joinWithCode(dep, ana, conference, 'atmosphere27')

  for (let round = 1; round <= 3; round++) {
    if (!(await asAna.isMember(conference))) {
      expect((await member(dep, conference, 'add', ana, olga)).code, `round ${round} starts`).toBe(0)
    }
    // The CLI removes Ana while the server handles her leaving and rejoining.
    const [removal, server] = await Promise.all([
      member(dep, conference, 'remove', ana, olga),
      (async () => [await asAna.leave(conference), await asAna.join(conference, 'atmosphere27')])(),
    ])
    expect(removal.code, `round ${round}: the removal ends cleanly`).not.toBeNull()
    expect(removal.stderr).not.toMatch(/panic|database is locked/i)
    for (const answer of server) expect(answer.status, `round ${round}`).toBeLessThan(500)

    // Whatever order they ran in, the space's records match the last decision.
    const isMember = await asAna.isMember(conference)
    await memberRecord(
      conference,
      ana.did,
      (record) => (record !== undefined) === isMember,
      `round ${round}: Ana's member record ${isMember ? 'is' : 'isn’t'} in the space`,
    )
  }
})

test('TC-36: Staff can make an attendee a speaker', async ({ viv }) => {
  const seeded = await seed(viv, { staff: ['pim'] })
  const { conference, olga } = seeded
  const joost = await viv.createAccount(viv.handle('joost'))
  expect((await member(dep, conference, 'add', joost, olga)).code).toBe(0)

  const made = await setRole(dep, conference, joost, 'speaker', admin(seeded, 'pim'))
  expect(made.code, made.stderr).toBe(0)
  expect(await (await Attendee.signIn(dep, joost)).role(conference)).toBe('speaker')
  const record = await memberRecord(conference, joost.did, (r) => r?.value.role === 'speaker', 'Joost is a speaker')
  expect(record?.value.role).toBe('speaker')
})
