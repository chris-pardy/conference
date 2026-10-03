import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react'
import { api, connectSession } from '../api'
import { fetchSession, type Session, SessionContext } from './session'

/** Loads who's signed in once, and keeps it current as the session changes. */
export function SessionProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<Session>({ kind: 'loading' })

  useEffect(() => {
    const controller = new AbortController()
    const load = () => {
      fetchSession(controller.signal)
        // Keep what's known if the appview can't be asked right now.
        .then((next) => setSession((prev) => (next.kind === 'unavailable' && prev.kind !== 'loading' ? prev : next)))
        .catch(() => {})
    }
    load()
    // Ask again when the connection, or the app, comes back.
    const onVisible = () => {
      if (document.visibilityState === 'visible') load()
    }
    window.addEventListener('online', load)
    document.addEventListener('visibilitychange', onVisible)
    return () => {
      controller.abort()
      window.removeEventListener('online', load)
      document.removeEventListener('visibilitychange', onVisible)
    }
  }, [])

  const csrfToken = session.kind === 'signedIn' ? session.user.csrfToken : undefined
  const did = session.kind === 'signedIn' ? session.user.did : undefined
  useEffect(() => {
    connectSession({
      csrfToken,
      did,
      expired: (handle, did) => setSession({ kind: 'expired', handle, did }),
      changed: setSession,
    })
  }, [csrfToken, did])

  const signOut = useCallback(async () => {
    try {
      const res = await api('/oauth/logout', { method: 'POST' })
      // Still refused after api() caught up with the session: another
      // account signed in elsewhere, and the shell now shows it. The session
      // this window meant to end is already over.
      if (res.status === 403) return true
      if (!res.ok && res.status !== 401) return false
    } catch {
      return false
    }
    setSession({ kind: 'signedOut' })
    return true
  }, [])

  const value = useMemo(() => ({ session, signOut }), [session, signOut])
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
}
