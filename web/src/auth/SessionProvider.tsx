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
    connectSession(csrfToken, (handle, did) => setSession({ kind: 'expired', handle, did }))
  }, [csrfToken])

  const signOut = useCallback(async () => {
    try {
      const res = await api('/oauth/logout', { method: 'POST' })
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
