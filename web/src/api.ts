// The fetch every feature uses to talk to the appview. It sends the session's
// CSRF token on state-changing requests, and reports an ended session to the
// SessionProvider so the shell can offer to sign the person back in, or show
// the signed-out state when the session was signed out elsewhere.

import { fetchSession, type Session } from './auth/session'

interface Connection {
  /** The signed-in session's CSRF token and account, if there is one. */
  csrfToken?: string
  did?: string
  /** The session has expired. */
  expired(handle?: string, did?: string): void
  /** The session changed elsewhere, e.g. a sign-in in another window. */
  changed(session: Session): void
}

let connection: Connection | undefined

/** Called by the SessionProvider as the session changes. */
export function connectSession(next: Connection): void {
  connection = next
}

const SAFE_METHODS = new Set(['GET', 'HEAD', 'OPTIONS'])

/** The session `api()` fetched while answering a refused request, by response. */
const learned = new WeakMap<Response, Session>()

/**
 * The session `api()` learned while handling an `InvalidCsrfToken` refusal,
 * if it asked (`unavailable` when the appview couldn't be asked), so a caller
 * needn't ask again.
 */
export function sessionLearned(res: Response): Session | undefined {
  return learned.get(res)
}

async function errorOf(res: Response): Promise<{ error?: string; handle?: string; did?: string }> {
  return res
    .clone()
    .json()
    .catch(() => ({}))
}

export async function api(input: string, init: RequestInit = {}): Promise<Response> {
  const method = (init.method ?? 'GET').toUpperCase()
  const send = (token: string | undefined) => {
    const headers = new Headers(init.headers)
    if (!SAFE_METHODS.has(method) && token) headers.set('X-CSRF-Token', token)
    return fetch(input, { ...init, headers, credentials: 'same-origin' })
  }
  const sent = connection?.csrfToken
  let res = await send(sent)
  // A missing token, or one from before a sign-out and sign-in in another
  // window: learn the current session, and try once more if it's still the
  // same account. A stream body is spent by the first send, so it can't be
  // retried.
  if (res.status === 403 && (await errorOf(res)).error === 'InvalidCsrfToken') {
    const sentFor = connection?.did
    const current: Session = await fetchSession().catch(() => ({ kind: 'unavailable' }))
    if (current.kind !== 'unavailable') connection?.changed(current)
    const replayable = !(init.body instanceof ReadableStream)
    if (
      replayable &&
      current.kind === 'signedIn' &&
      sentFor &&
      current.user.did === sentFor &&
      current.user.csrfToken !== sent
    ) {
      if (connection) connection.csrfToken = current.user.csrfToken
      res = await send(current.user.csrfToken)
    }
    learned.set(res, current)
  }
  if (res.status === 401) {
    const body = await errorOf(res)
    if (body.error === 'SessionExpired') connection?.expired(body.handle, body.did)
    // Signed out elsewhere, e.g. in another window.
    else if (body.error === 'AuthRequired') connection?.changed({ kind: 'signedOut' })
  }
  return res
}
