import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { api, connectSession, sessionLearned } from '../api'
import { fetchSession, type Session, SessionContext } from './session'

/** Loads who's signed in once, and keeps it current as the session changes. */
export function SessionProvider({ children }: { children: ReactNode }) {
  const [session, setSessionState] = useState<Session>({ kind: 'loading' })
  // Bumped whenever the session is set from something newer than a
  // getSession already in flight (sign-out, a change or expiry api() saw),
  // so that answer, when it lands, can't undo it.
  const generation = useRef(0)
  const setSession = useCallback((next: Session) => {
    generation.current++
    setSessionState(next)
  }, [])

  useEffect(() => {
    const controller = new AbortController()
    const load = () => {
      const started = generation.current
      fetchSession(controller.signal)
        .then((next) => {
          if (generation.current !== started) return
          // Keep what's known if the appview can't be asked right now.
          setSessionState((prev) => (next.kind === 'unavailable' && prev.kind !== 'loading' ? prev : next))
        })
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
  }, [csrfToken, did, setSession])

  const signOut = useCallback(async () => {
    try {
      const res = await api('/oauth/logout', { method: 'POST' })
      // Still refused after api() tried to catch up with the session. It
      // worked only if the session this window meant to end is seen to be
      // over: signed out, expired, or another account signed in elsewhere.
      // If the appview can't be asked, or it's still the same account, it
      // didn't. Uses what api() learned when it caught up, if it asked.
      if (res.status === 403) {
        const now = sessionLearned(res) ?? (await fetchSession().catch((): Session => ({ kind: 'unavailable' })))
        if (now.kind === 'unavailable' || (now.kind === 'signedIn' && now.user.did === did)) return false
        setSession(now)
        return true
      }
      if (!res.ok && res.status !== 401) return false
    } catch {
      return false
    }
    setSession({ kind: 'signedOut' })
    return true
  }, [did, setSession])

  const value = useMemo(() => ({ session, signOut }), [session, signOut])
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
}
