import { createContext, useContext } from 'react'
import type { OutputSchema as SessionInfo } from '../lexicon/types/app/eventside/auth/getSession'

export type { SessionInfo }

/** Where the browser's session stands. */
export type Session =
  | { kind: 'loading' }
  | { kind: 'signedOut' }
  | { kind: 'signedIn'; user: SessionInfo }
  | { kind: 'expired'; handle?: string; did?: string }
  /** The appview couldn't be asked (offline, or a 5xx): not the same as signed out. */
  | { kind: 'unavailable' }

export interface SessionContextValue {
  session: Session
  /** Signs out, resolving to whether it worked. */
  signOut(): Promise<boolean>
}

export const SessionContext = createContext<SessionContextValue | null>(null)

/** The signed-in state, from the `SessionProvider` in the app's shell. */
export function useSession(): SessionContextValue {
  const value = useContext(SessionContext)
  if (!value) throw new Error('useSession needs a SessionProvider above it')
  return value
}

export const GET_SESSION = '/xrpc/app.eventside.auth.getSession'

/**
 * Reads who's signed in. Only the appview's own answer decides: its 401
 * `AuthRequired` is signed out and its 401 `SessionExpired` is expired.
 * Anything else (a network error, a 5xx, or another status such as a
 * proxy's 429 or 408) reads as `unavailable`, never as signed out. Throws
 * only when aborted.
 */
export async function fetchSession(signal?: AbortSignal): Promise<Session> {
  let res: Response
  try {
    res = await fetch(GET_SESSION, { signal, credentials: 'same-origin', cache: 'no-store' })
  } catch (err) {
    if (signal?.aborted) throw err
    return { kind: 'unavailable' }
  }
  if (res.ok) {
    try {
      return { kind: 'signedIn', user: (await res.json()) as SessionInfo }
    } catch {
      return { kind: 'unavailable' }
    }
  }
  if (res.status !== 401) return { kind: 'unavailable' }
  const body = await res.json().catch(() => ({}))
  if (body.error === 'SessionExpired') return { kind: 'expired', handle: body.handle, did: body.did }
  if (body.error === 'AuthRequired') return { kind: 'signedOut' }
  return { kind: 'unavailable' }
}

/** Who to sign back in after a session expired: the handle, or the DID when the handle didn't verify. */
export function signInHint(session: { handle?: string; did?: string }): string | undefined {
  return session.handle && session.handle !== 'handle.invalid' ? session.handle : session.did
}

/** Sends the browser through sign-in, returning it to `returnTo`. A DID works as well as a handle. */
export function startSignIn(handle: string, returnTo: string): void {
  const params = new URLSearchParams({ handle, return_to: returnTo })
  window.location.assign(`/oauth/login?${params}`)
}
