import { expect, test, vi } from '@vivarium-dev/client/vitest'
import { getSession, signIn, signOut, sleep } from '../support/auth.ts'
import { createOrg, deploy } from '../support/conference.ts'

// Seeding an organization takes several CLI runs and OAuth round trips.
vi.setConfig({ testTimeout: 120_000, hookTimeout: 60_000 })

test('TC-41: Signing in still works the same for attendees', async ({ viv }) => {
  // Short enough to wait out an idle session, with the renewer running often.
  const dep = await deploy({ SESSION_IDLE_TIMEOUT: '2s', TOKEN_RENEW_INTERVAL: '1s' })
  try {
    // The organization sign-in is in use: an admin has connected.
    const olga = await viv.createAccount(viv.handle('olga'))
    await createOrg(dep, olga)
    const ana = await viv.createAccount(viv.handle('ana'))

    // Sign in, and the app says who's signed in.
    const first = await signIn(dep.url, ana.handle, { returnTo: '/somewhere' })
    expect(new URL(first.location as string).pathname).toBe('/somewhere')
    const session = await getSession(dep.url, first.jar)
    expect(session.status).toBe(200)
    expect(session.body).toMatchObject({ did: ana.did, handle: ana.handle })

    // Sign out.
    expect((await signOut(dep.url, first.jar, session.body.csrfToken)).status).toBe(200)
    const out = await getSession(dep.url, first.jar)
    expect(out.status).toBe(401)
    expect(out.body.error).toBe('AuthRequired')

    // A session left idle expires, and says whose it was.
    const second = await signIn(dep.url, ana.handle)
    expect((await getSession(dep.url, second.jar)).status).toBe(200)
    await sleep(3_500)
    const expired = await getSession(dep.url, second.jar)
    expect(expired.status).toBe(401)
    expect(expired.body).toMatchObject({ error: 'SessionExpired', handle: ana.handle, did: ana.did })

    // An admin signs in to the app like anyone else.
    const asOlga = await signIn(dep.url, olga.handle)
    const olgaSession = await getSession(dep.url, asOlga.jar)
    expect(olgaSession.status).toBe(200)
    expect(olgaSession.body.did).toBe(olga.did)
  } finally {
    await dep.stop()
  }
})
