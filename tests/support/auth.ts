// Signs people in to the appview the way a browser would: the real OAuth flow
// against vivarium, over plain HTTP. Vivarium's consent screens are no-JS HTML
// forms (pick or type an account, then Allow or Deny), so following redirects
// and posting those forms with a cookie jar is all a browser does too.
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

/** Cookies per host, enough for the appview and vivarium. */
export class CookieJar {
  private readonly hosts = new Map<string, Map<string, string>>()

  /** The cookies this jar would send to `url`, as a Cookie header. */
  header(url: string): string {
    const cookies = this.hosts.get(new URL(url).hostname)
    return cookies ? [...cookies].map(([name, value]) => `${name}=${value}`).join('; ') : ''
  }

  get(url: string, name: string): string | undefined {
    return this.hosts.get(new URL(url).hostname)?.get(name)
  }

  set(url: string, name: string, value: string): void {
    const host = new URL(url).hostname
    if (!this.hosts.has(host)) this.hosts.set(host, new Map())
    this.hosts.get(host)?.set(name, value)
  }

  /** Takes in a response's Set-Cookie headers, honoring deletions. */
  store(url: string, res: Response): void {
    for (const line of res.headers.getSetCookie()) {
      const [pair, ...attrs] = line.split(';').map((part) => part.trim())
      const eq = pair.indexOf('=')
      const name = pair.slice(0, eq)
      const value = pair.slice(eq + 1)
      const expired = attrs.some((attr) => {
        const [key, val = ''] = attr.split('=')
        if (key.toLowerCase() === 'max-age') return Number(val) <= 0
        if (key.toLowerCase() === 'expires') return Date.parse(val) <= Date.now()
        return false
      })
      if (expired || value === '') this.hosts.get(new URL(url).hostname)?.delete(name)
      else this.set(url, name, value)
    }
  }

  /** A GET or form POST that doesn't follow redirects. */
  async fetch(
    url: string,
    init: { method?: string; form?: Record<string, string>; headers?: Record<string, string> } = {},
  ) {
    const headers: Record<string, string> = { ...init.headers }
    const cookie = this.header(url)
    if (cookie) headers.cookie = cookie
    const res = await fetch(url, {
      method: init.method ?? (init.form ? 'POST' : 'GET'),
      headers,
      body: init.form ? new URLSearchParams(init.form) : undefined,
      redirect: 'manual',
    })
    this.store(url, res)
    return res
  }
}

export interface SignInOptions {
  /** Signs in with this jar, e.g. a browser that already has cookies. */
  jar?: CookieJar
  /** Where the app should land afterwards. */
  returnTo?: string
  /** The account picked on the consent screen, if not the one signing in. */
  account?: string
  /** Allow (the default) or deny on the consent screen. */
  decision?: 'allow' | 'deny'
  /** Stop before opening the callback, returning its URL unopened. */
  stopAtCallback?: boolean
}

export interface FlowResult {
  jar: CookieJar
  /** The response that started the flow: a redirect to the authorization server, or an error. */
  start: Response
  /** The appview callback URL the authorization server sent the browser to. */
  callbackUrl?: string
  /** Where the appview's callback redirected, unless `stopAtCallback`. */
  location?: string
}

/** Signs in with a handle through `/oauth/login`. */
export function signIn(serverUrl: string, handle: string, opts: SignInOptions = {}): Promise<FlowResult> {
  const params = new URLSearchParams({ handle })
  if (opts.returnTo !== undefined) params.set('return_to', opts.returnTo)
  return runFlow(`${serverUrl}/oauth/login?${params}`, opts.account ?? handle, opts)
}

/** Creates an account through `/oauth/signup`; vivarium makes it on the fly. */
export function signUp(serverUrl: string, newHandle: string, opts: SignInOptions = {}): Promise<FlowResult> {
  const params = new URLSearchParams()
  if (opts.returnTo !== undefined) params.set('return_to', opts.returnTo)
  return runFlow(`${serverUrl}/oauth/signup?${params}`, opts.account ?? newHandle, opts)
}

async function runFlow(startUrl: string, account: string, opts: SignInOptions): Promise<FlowResult> {
  const jar = opts.jar ?? new CookieJar()
  const start = await jar.fetch(startUrl)
  const authorizeUrl = absolute(startUrl, start.headers.get('location'))
  const startOrigin = new URL(startUrl).origin
  // Not sent to an authorization server: the appview answered with an error.
  if (!isRedirect(start) || !authorizeUrl || new URL(authorizeUrl).origin === startOrigin) {
    return { jar, start, location: authorizeUrl }
  }

  let page = await jar.fetch(authorizeUrl)
  let html = await page.text()
  if (page.status !== 200) throw new Error(`the authorization page answered ${page.status}: ${html}`)

  // Step 1: pick (or type) the account, unless the server skipped straight to consent.
  if (!html.includes('/oauth/authorize/decide')) {
    const form = formFields(html, '/oauth/authorize/sign-in')
    page = await jar.fetch(form.action, { form: { ...form.fields, account } })
    if (isRedirect(page)) return finish(jar, start, page, opts)
    html = await page.text()
  }

  // Step 2: allow or deny.
  const consent = formFields(html, '/oauth/authorize/decide')
  const decided = await jar.fetch(consent.action, {
    form: { ...consent.fields, decision: opts.decision ?? 'allow' },
  })
  if (!isRedirect(decided)) throw new Error(`the consent form answered ${decided.status}: ${await decided.text()}`)
  return finish(jar, start, decided, opts)
}

async function finish(jar: CookieJar, start: Response, toCallback: Response, opts: SignInOptions): Promise<FlowResult> {
  const callbackUrl = toCallback.headers.get('location') ?? undefined
  if (!callbackUrl || opts.stopAtCallback) return { jar, start, callbackUrl }
  const callback = await jar.fetch(callbackUrl)
  return { jar, start, callbackUrl, location: absolute(callbackUrl, callback.headers.get('location')) }
}

/** Opens an appview URL (a callback, say) in a browser with this jar. */
export async function open(jar: CookieJar, url: string): Promise<{ res: Response; location?: string }> {
  const res = await jar.fetch(url)
  return { res, location: absolute(url, res.headers.get('location')) }
}

const isRedirect = (res: Response) => res.status >= 300 && res.status < 400

function absolute(base: string, location: string | null): string | undefined {
  return location ? new URL(location, base).toString() : undefined
}

/** The action and hidden fields of the form posting to `actionPath`. */
function formFields(html: string, actionPath: string): { action: string; fields: Record<string, string> } {
  const forms = [...html.matchAll(/<form\b([^>]*)>([\s\S]*?)<\/form>/g)]
  const form = forms.find(([, attrs]) => attrs.includes(actionPath))
  if (!form) throw new Error(`no form posting to ${actionPath} in:\n${html}`)
  const action = decode(/action="([^"]+)"/.exec(form[1])?.[1] ?? '')
  const fields: Record<string, string> = {}
  for (const [input] of form[2].matchAll(/<input\b[^>]*>/g)) {
    if (!/type="hidden"/.test(input)) continue
    const name = /name="([^"]+)"/.exec(input)?.[1]
    const value = /value="([^"]*)"/.exec(input)?.[1]
    if (name && value !== undefined) fields[name] = decode(value)
  }
  return { action, fields }
}

const decode = (s: string) =>
  s
    .replaceAll('&amp;', '&')
    .replaceAll('&quot;', '"')
    .replaceAll('&#39;', "'")
    .replaceAll('&lt;', '<')
    .replaceAll('&gt;', '>')

/** The appview's answer to "who's signed in", for a jar. */
export async function getSession(serverUrl: string, jar: CookieJar): Promise<{ status: number; body: SessionBody }> {
  const res = await jar.fetch(`${serverUrl}/xrpc/app.eventside.auth.getSession`)
  const text = await res.text()
  let body: SessionBody
  try {
    body = JSON.parse(text)
  } catch {
    body = { raw: text }
  }
  return { status: res.status, body }
}

// biome-ignore lint/suspicious/noExplicitAny: a JSON response under test
export type SessionBody = Record<string, any>

/** The session cookie the appview set for a jar (named `session` on plain HTTP). */
export const sessionCookie = (serverUrl: string, jar: CookieJar) => jar.get(serverUrl, 'session')

/** Signs out with the jar's session and its CSRF token. */
export async function signOut(serverUrl: string, jar: CookieJar, csrfToken?: string): Promise<Response> {
  return jar.fetch(`${serverUrl}/oauth/logout`, {
    method: 'POST',
    headers: csrfToken === undefined ? {} : { 'x-csrf-token': csrfToken },
  })
}

/** A person's stored tokens, read from the test's own database. */
export function storedTokens(
  databasePath: string,
  did: string,
): { accessToken: string; refreshToken: string } | undefined {
  // No database yet means no session yet.
  if (!existsSync(databasePath)) return undefined
  const db = new DatabaseSync(databasePath, { readOnly: true })
  try {
    const row = db
      .prepare(
        'SELECT access_token, refresh_token FROM sessions WHERE did = ? AND access_token IS NOT NULL ORDER BY created_at DESC LIMIT 1',
      )
      .get(did) as { access_token: string; refresh_token: string } | undefined
    return row && { accessToken: row.access_token, refreshToken: row.refresh_token }
  } finally {
    db.close()
  }
}

/** Whether vivarium still honors a token, by introspection. */
export async function tokenActive(vivariumUrl: string, token: string, clientId: string): Promise<boolean> {
  const res = await fetch(`${vivariumUrl}/oauth/introspect`, {
    method: 'POST',
    body: new URLSearchParams({ token, client_id: clientId }),
  })
  if (!res.ok) throw new Error(`introspection answered ${res.status}: ${await res.text()}`)
  return (await res.json()).active === true
}

const AVATAR_PNG = resolve(import.meta.dirname, '../../web/public/icons/icon-192.png')

/** Gives an account a public profile with a display name and an avatar. */
export async function giveProfile(
  vivariumUrl: string,
  account: { did: string; accessJwt: string },
  displayName: string,
): Promise<void> {
  const upload = await fetch(`${vivariumUrl}/xrpc/com.atproto.repo.uploadBlob`, {
    method: 'POST',
    headers: { authorization: `Bearer ${account.accessJwt}`, 'content-type': 'image/png' },
    body: readFileSync(AVATAR_PNG),
  })
  if (!upload.ok) throw new Error(`uploadBlob answered ${upload.status}: ${await upload.text()}`)
  const { blob } = await upload.json()
  const put = await fetch(`${vivariumUrl}/xrpc/com.atproto.repo.putRecord`, {
    method: 'POST',
    headers: { authorization: `Bearer ${account.accessJwt}`, 'content-type': 'application/json' },
    body: JSON.stringify({
      repo: account.did,
      collection: 'app.bsky.actor.profile',
      rkey: 'self',
      record: { $type: 'app.bsky.actor.profile', displayName, avatar: blob },
    }),
  })
  if (!put.ok) throw new Error(`putRecord answered ${put.status}: ${await put.text()}`)
}

export const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms))
