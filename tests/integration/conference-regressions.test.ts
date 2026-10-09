import { expect, test, vi } from '@vivarium-dev/client/vitest'
import { getSession, signIn, signOut } from '../support/auth.ts'
import { Attendee, accountsIn, deploy, seedConference, sharedCode } from '../support/conference.ts'
import { conferenceWriteScope, isDefaultScopeList, spaceActions } from '../support/scopes.ts'

// Seeding takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 180_000, hookTimeout: 60_000 })

test('TC-42: Signing in still works, with the new permissions', async ({ viv }) => {
  // A server as it was before conference-space: identity only.
  const dep = await deploy({ OAUTH_SCOPES: 'atproto' })
  try {
    const joost = await viv.createAccount(viv.handle('joost'))
    const before = await signIn(dep.url, joost.handle)
    const old = await getSession(dep.url, before.jar)
    expect(old.body.scopes).toEqual(['atproto'])

    // The new release asks for the default scopes.
    await dep.restart({ OAUTH_SCOPES: undefined })

    // Ana signs in, and is asked for permission to write to conferences, but not to read them.
    const ana = await viv.createAccount(viv.handle('ana'))
    const flow = await signIn(dep.url, ana.handle, { returnTo: '/somewhere' })
    expect(new URL(flow.location as string).pathname).toBe('/somewhere')
    const session = await getSession(dep.url, flow.jar)
    expect(session.status).toBe(200)
    expect(session.body).toMatchObject({ did: ana.did, handle: ana.handle })
    const scopes: string[] = session.body.scopes
    expect(isDefaultScopeList(scopes), JSON.stringify(scopes)).toBe(true)
    const write = conferenceWriteScope(scopes)
    expect(write, `a scope to write to conferences among ${scopes}`).toBeDefined()
    expect(spaceActions(write as string)).not.toContain('read')

    // Signing out still works.
    expect((await signOut(dep.url, flow.jar, session.body.csrfToken)).status).toBe(200)
    expect((await getSession(dep.url, flow.jar)).status).toBe(401)

    // Joost, signed in before the change, is asked to sign in again before joining.
    const { conference, olga } = await seedConference(dep, accountsIn(viv), { methods: ['code'] })
    await sharedCode(dep, conference, 'atmosphere27', olga)
    const refused = await fetch(`${dep.url}/xrpc/app.eventside.conference.join`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        cookie: before.jar.header(dep.url),
        'x-csrf-token': old.body.csrfToken,
      },
      body: JSON.stringify({ conference: conference.space, code: 'atmosphere27' }),
    })
    expect([401, 403]).toContain(refused.status)
    const expired = await getSession(dep.url, before.jar)
    expect(expired.status).toBe(401)
    expect(expired.body.error).toBe('SessionExpired')
    // Once he signs in again, he joins.
    const again = await Attendee.signIn(dep, joost)
    expect((await again.join(conference, 'atmosphere27')).body.status).toBe('joined')
  } finally {
    await dep.stop()
  }
})
