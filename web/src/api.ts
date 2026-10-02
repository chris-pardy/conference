// The fetch every feature uses to talk to the appview. It sends the session's
// CSRF token on state-changing requests, and reports an ended session to the
// SessionProvider so the shell can offer to sign the person back in.

let csrfToken: string | undefined
let onExpired: ((handle?: string) => void) | undefined

/** Called by the SessionProvider as the session changes. */
export function connectSession(token: string | undefined, expired: (handle?: string) => void): void {
  csrfToken = token
  onExpired = expired
}

const SAFE_METHODS = new Set(['GET', 'HEAD', 'OPTIONS'])

export async function api(input: string, init: RequestInit = {}): Promise<Response> {
  const method = (init.method ?? 'GET').toUpperCase()
  const headers = new Headers(init.headers)
  if (!SAFE_METHODS.has(method) && csrfToken) headers.set('X-CSRF-Token', csrfToken)
  const res = await fetch(input, { ...init, headers, credentials: 'same-origin' })
  if (res.status === 401) {
    const body = await res
      .clone()
      .json()
      .catch(() => ({}))
    if (body.error === 'SessionExpired') onExpired?.(body.handle)
  }
  return res
}
