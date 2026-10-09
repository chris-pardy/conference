// A browser without a page: follows redirects with a cookie jar and gets
// through vivarium's no-JS consent screens (pick the account, then Allow or
// Deny), for flows that don't start at the app's own sign-in route, such as
// connecting an admin from the CLI, the email step, or another app's sign-in.
import { CookieJar } from './auth.ts'

export interface BrowsedTo {
  /** The last response, which wasn't a redirect the browser followed. */
  res: Response
  /** The URL that answered `res`. */
  url: string
  /** Where `res` redirects, if it does (a redirect `browse` stopped at). */
  location?: string
  /** The body of `res`. */
  text: string
}

export interface BrowseOptions {
  jar?: CookieJar
  /** The account to pick on vivarium's account screen. */
  account?: string
  decision?: 'allow' | 'deny'
  /** Stop at the first redirect to a URL starting with this, without opening it. */
  stopAt?: string
}

/** Opens `url` and keeps going: redirects, the account screen and the consent screen. */
export async function browse(url: string, opts: BrowseOptions = {}): Promise<BrowsedTo & { jar: CookieJar }> {
  const jar = opts.jar ?? new CookieJar()
  let next: { url: string; form?: Record<string, string> } = { url }
  for (let hop = 0; hop < 20; hop++) {
    const res = await jar.fetch(next.url, next.form ? { form: next.form } : {})
    const location = res.headers.get('location')
    if (res.status >= 300 && res.status < 400 && location) {
      const target = new URL(location, next.url).toString()
      if (opts.stopAt && target.startsWith(opts.stopAt)) {
        return { jar, res, url: next.url, location: target, text: await res.text() }
      }
      next = { url: target }
      continue
    }
    const text = await res.text()
    if (text.includes('/oauth/authorize/decide')) {
      const consent = formFields(text, '/oauth/authorize/decide')
      next = { url: consent.action, form: { ...consent.fields, decision: opts.decision ?? 'allow' } }
      continue
    }
    if (text.includes('/oauth/authorize/sign-in')) {
      if (!opts.account) throw new Error(`the authorization server asked for an account at ${next.url}`)
      const signIn = formFields(text, '/oauth/authorize/sign-in')
      next = { url: signIn.action, form: { ...signIn.fields, account: opts.account } }
      continue
    }
    return { jar, res, url: next.url, text }
  }
  throw new Error(`more than 20 redirects starting from ${url}`)
}

/** The action and hidden fields of the form posting to `actionPath`. */
export function formFields(html: string, actionPath: string): { action: string; fields: Record<string, string> } {
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
