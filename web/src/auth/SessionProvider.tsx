import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react'
import { api, connectSession } from '../api'
import { fetchSession, type Session, SessionContext } from './session'

/** Loads who's signed in once, and keeps it current as the session changes. */
export function SessionProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<Session>({ kind: 'loading' })

  useEffect(() => {
    const controller = new AbortController()
    fetchSession(controller.signal)
      .then(setSession)
      .catch(() => {
        if (!controller.signal.aborted) setSession({ kind: 'signedOut' })
      })
    return () => controller.abort()
  }, [])

  const csrfToken = session.kind === 'signedIn' ? session.user.csrfToken : undefined
  useEffect(() => {
    connectSession(csrfToken, (handle) => setSession({ kind: 'expired', handle }))
  }, [csrfToken])

  const signOut = useCallback(async () => {
    const res = await api('/oauth/logout', { method: 'POST' })
    if (res.ok || res.status === 401) setSession({ kind: 'signedOut' })
  }, [])

  const value = useMemo(() => ({ session, signOut }), [session, signOut])
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
}
