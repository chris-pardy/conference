import { createContext, useContext } from 'react'
import type { OutputSchema as SessionInfo } from '../lexicon/types/app/eventside/auth/getSession'

export type { SessionInfo }

/** Where the browser's session stands. */
export type Session =
  | { kind: 'loading' }
  | { kind: 'signedOut' }
  | { kind: 'signedIn'; user: SessionInfo }
  | { kind: 'expired'; handle?: string }

export interface SessionContextValue {
  session: Session
  signOut(): Promise<void>
}

export const SessionContext = createContext<SessionContextValue | null>(null)

/** The signed-in state, from the `SessionProvider` in the app's shell. */
export function useSession(): SessionContextValue {
  const value = useContext(SessionContext)
  if (!value) throw new Error('useSession needs a SessionProvider above it')
  return value
}

export const GET_SESSION = '/xrpc/app.eventside.auth.getSession'

/** Reads who's signed in. Never throws: a failure reads as signed out. */
export async function fetchSession(signal?: AbortSignal): Promise<Session> {
  const res = await fetch(GET_SESSION, { signal, credentials: 'same-origin', cache: 'no-store' })
  if (res.ok) return { kind: 'signedIn', user: (await res.json()) as SessionInfo }
  const body = await res.json().catch(() => ({}))
  if (res.status === 401 && body.error === 'SessionExpired') return { kind: 'expired', handle: body.handle }
  return { kind: 'signedOut' }
}

/** Where to come back to after signing in: the page the person is on. */
export function currentPath(): string {
  return window.location.pathname + window.location.search
}

/** Sends the browser through sign-in, returning it to `returnTo`. */
export function startSignIn(handle: string, returnTo: string): void {
  const params = new URLSearchParams({ handle, return_to: returnTo })
  window.location.assign(`/oauth/login?${params}`)
}
