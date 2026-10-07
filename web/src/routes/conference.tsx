import { type FormEvent, useCallback, useEffect, useState } from 'react'
import { Link, useLocation, useParams } from 'react-router'
import { api } from '../api'
import { useSession } from '../auth/session'
import './conference.css'

/** A conference as `app.eventside.conference.getConference` describes it. */
export interface ConferenceView {
  space: string
  event?: string
  name: string
  startsAt?: string
  endsAt?: string
  locations?: { locality?: string; name?: string; region?: string; country?: string }[]
  description?: string
  theme?: Record<string, string>
  inviteOnly?: boolean
  join?: { methods: string[] }
  viewer?: { member: boolean; role?: string; request?: 'pending' | 'denied' }
}

interface JoinAnswer {
  status: 'joined' | 'pending' | 'refused' | 'emailNeeded'
  conference?: string
  canRequest?: boolean
  verifyUrl?: string
}

const EVENT = 'community.lexicon.calendar.event'
const CONFERENCE_SPACE = 'app.eventside.conference'

/** Where a conference's page is: its public event's, or (invite-only) its space's. */
export function conferencePath(conference: Pick<ConferenceView, 'space' | 'event'>): string {
  const event = conference.event && /^at:\/\/([^/]+)\/[^/]+\/([^/]+)$/.exec(conference.event)
  if (event && !conference.event?.includes('/space/')) return `/c/${event[1]}/${event[2]}`
  const space = /^at:\/\/([^/]+)\/space\/[^/]+\/([^/]+)$/.exec(conference.space)
  return space ? `/space/${space[1]}/${space[2]}` : '/conferences'
}

/** Theme tokens a conference may set: names like `color-primary`, values without markup. */
function safeTokens(theme: Record<string, string> | undefined): [string, string][] {
  return Object.entries(theme ?? {}).filter(
    ([name, value]) => /^[a-z0-9-]{1,64}$/.test(name) && typeof value === 'string' && !/[;{}<>]/.test(value),
  )
}

/** Sets a conference's theme on :root while its page is open. */
function useTheme(theme: Record<string, string> | undefined) {
  useEffect(() => {
    const root = document.documentElement
    const tokens = safeTokens(theme)
    for (const [name, value] of tokens) root.style.setProperty(`--g-${name}`, value)
    return () => {
      for (const [name] of tokens) root.style.removeProperty(`--g-${name}`)
    }
  }, [theme])
}

function dateRange(startsAt?: string, endsAt?: string): string | undefined {
  if (!startsAt) return undefined
  const start = new Date(startsAt)
  if (Number.isNaN(start.getTime())) return undefined
  const day = (d: Date) => d.toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })
  const end = endsAt ? new Date(endsAt) : undefined
  if (!end || Number.isNaN(end.getTime())) return `${day(start)} ${start.getFullYear()}`
  return `${day(start)} – ${day(end)} ${end.getFullYear()}`
}

function place(conference: ConferenceView): string | undefined {
  const location = conference.locations?.[0]
  return location ? [location.name, location.locality, location.region].filter(Boolean).join(', ') : undefined
}

type Load =
  | { kind: 'loading' }
  | { kind: 'missing' }
  | { kind: 'failed' }
  | { kind: 'ready'; conference: ConferenceView }

/** Loads a conference by its event's AT-URI or its space URI. */
function useConference(uri: string | undefined): [Load, () => void] {
  const [load, setLoad] = useState<Load>({ kind: 'loading' })
  const [attempt, setAttempt] = useState(0)
  const { session } = useSession()
  const signedIn = session.kind === 'signedIn' ? session.user.did : session.kind
  // biome-ignore lint/correctness/useExhaustiveDependencies: reload when who's signed in changes, or on demand
  useEffect(() => {
    if (!uri) return
    const controller = new AbortController()
    api(`/xrpc/app.eventside.conference.getConference?${new URLSearchParams({ conference: uri })}`, {
      signal: controller.signal,
    })
      .then(async (res) => {
        if (res.status === 404) return setLoad({ kind: 'missing' })
        if (!res.ok) return setLoad({ kind: 'failed' })
        setLoad({ kind: 'ready', conference: (await res.json()) as ConferenceView })
      })
      .catch(() => {
        if (!controller.signal.aborted) setLoad({ kind: 'failed' })
      })
    return () => controller.abort()
  }, [uri, attempt, signedIn])
  return [load, useCallback(() => setAttempt((n) => n + 1), [])]
}

/** `/c/:actor/:rkey`: a public conference, by its event (the super admin's DID or handle). */
export function PublicConferencePage() {
  const { actor, rkey } = useParams()
  return <ConferencePage uri={actor && rkey ? `at://${actor}/${EVENT}/${rkey}` : undefined} />
}

/** `/space/:authority/:skey`: a conference by its space, for members of an invite-only one. */
export function SpaceConferencePage() {
  const { authority, skey } = useParams()
  return <ConferencePage uri={authority && skey ? `at://${authority}/space/${CONFERENCE_SPACE}/${skey}` : undefined} />
}

function ConferencePage({ uri }: { uri?: string }) {
  const [load, reload] = useConference(uri)
  if (load.kind === 'loading') {
    return (
      <main className="conference">
        <p className="conference__muted">Loading…</p>
      </main>
    )
  }
  if (load.kind === 'missing') {
    return (
      <main className="conference">
        <h1>Nothing here</h1>
        <p className="conference__muted">There’s no conference at this address, or it isn’t one you can see.</p>
      </main>
    )
  }
  if (load.kind === 'failed') {
    return (
      <main className="conference">
        <p role="alert">This conference couldn’t be loaded. Check your connection and try again.</p>
        <button type="button" className="conference__secondary" onClick={reload}>
          Try again
        </button>
      </main>
    )
  }
  return <Conference conference={load.conference} reload={reload} />
}

/** A conference's page: its public facts, then the inside for members or the way in for everyone else. */
function Conference({ conference, reload }: { conference: ConferenceView; reload: () => void }) {
  useTheme(conference.theme)
  const dates = dateRange(conference.startsAt, conference.endsAt)
  const where = place(conference)
  return (
    <main className="conference">
      <header className="conference__header">
        <h1>{conference.name}</h1>
        {(dates || where) && (
          <p className="conference__facts">
            {dates && <span>{dates}</span>}
            {dates && where && <span aria-hidden="true"> · </span>}
            {where && <span>{where}</span>}
          </p>
        )}
        {conference.description && <p className="conference__description">{conference.description}</p>}
      </header>
      {conference.viewer?.member ? (
        <Inside conference={conference} reload={reload} />
      ) : (
        <WayIn conference={conference} reload={reload} />
      )}
    </main>
  )
}

/** What a member sees: they're in, the organizer's announcements, and leaving. */
function Inside({ conference, reload }: { conference: ConferenceView; reload: () => void }) {
  const [announcements, setAnnouncements] = useState<{ text?: string }[]>([])
  const [leaving, setLeaving] = useState(false)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    const controller = new AbortController()
    const params = new URLSearchParams({
      conference: conference.space,
      collection: 'app.eventside.conference.announcement',
    })
    api(`/xrpc/app.eventside.conference.listRecords?${params}`, { signal: controller.signal })
      .then(async (res) => {
        if (res.ok)
          setAnnouncements(((await res.json()).records ?? []).map((r: { value: { text?: string } }) => r.value))
      })
      .catch(() => {})
    return () => controller.abort()
  }, [conference.space])

  const leave = async () => {
    setLeaving(true)
    setFailed(false)
    try {
      const res = await api('/xrpc/app.eventside.conference.leave', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ conference: conference.space }),
      })
      if (!res.ok) throw new Error(`leave answered ${res.status}`)
      reload()
    } catch {
      setFailed(true)
    } finally {
      setLeaving(false)
    }
  }

  return (
    <section className="conference__panel" aria-label="Inside the conference">
      <p className="conference__status">You’re in{conference.viewer?.role ? `, as ${conference.viewer.role}` : ''}.</p>
      {announcements.length > 0 && (
        <ul className="conference__announcements">
          {announcements.map((a, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: announcements have no key of their own here
            <li key={i}>{a.text}</li>
          ))}
        </ul>
      )}
      {failed && <p role="alert">Couldn’t leave. Check your connection and try again.</p>}
      <button type="button" className="conference__secondary" onClick={leave} disabled={leaving}>
        Leave
      </button>
    </section>
  )
}

type Step =
  | { kind: 'idle' }
  | { kind: 'busy' }
  | { kind: 'email'; verifyUrl: string; canRequest: boolean }
  | { kind: 'message'; text: string }

/** How someone who isn't a member gets in, by the methods the conference has on. */
function WayIn({ conference, reload }: { conference: ConferenceView; reload: () => void }) {
  const { session } = useSession()
  const location = useLocation()
  const [code, setCode] = useState('')
  const [entering, setEntering] = useState(false)
  const [step, setStep] = useState<Step>({ kind: 'idle' })
  const methods = new Set(conference.join?.methods ?? [])
  const request = conference.viewer?.request

  if (session.kind !== 'signedIn') {
    const returnTo = location.pathname + location.search
    return (
      <section className="conference__panel" aria-label="Joining">
        <p>
          <Link to={`/signin?${new URLSearchParams({ return_to: returnTo })}`}>Sign in</Link> to join this conference.
        </p>
      </section>
    )
  }

  const join = async (input: { code?: string; request?: boolean }) => {
    setStep({ kind: 'busy' })
    try {
      const res = await api('/xrpc/app.eventside.conference.join', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ conference: conference.space, ...input }),
      })
      const body = await res.json().catch(() => ({}))
      if (res.status === 400 && body.error === 'InvalidCode')
        return setStep({ kind: 'message', text: 'That code isn’t valid.' })
      if (res.status === 429) return setStep({ kind: 'message', text: 'Too many tries. Wait a minute and try again.' })
      if (!res.ok) return setStep({ kind: 'message', text: 'Joining didn’t work. Please try again.' })
      const answer = body as JoinAnswer
      if (answer.status === 'emailNeeded' && answer.verifyUrl) {
        return setStep({ kind: 'email', verifyUrl: answer.verifyUrl, canRequest: !!answer.canRequest })
      }
      if (answer.status === 'refused') return setStep({ kind: 'message', text: 'You can’t join this conference.' })
      setStep({ kind: 'idle' })
      reload()
    } catch {
      setStep({ kind: 'message', text: 'Joining didn’t work. Check your connection and try again.' })
    }
  }

  if (step.kind === 'email') {
    const verify = () => {
      const returnTo = location.pathname + location.search
      window.location.assign(`${step.verifyUrl}&${new URLSearchParams({ return_to: returnTo })}`)
    }
    return (
      <section className="conference__panel" aria-label="Joining">
        <p>This conference admits people on its attendee list by email. Your account’s server can confirm yours.</p>
        <div className="conference__actions">
          <button type="button" className="conference__primary" onClick={verify}>
            Verify my email
          </button>
          {step.canRequest && (
            <button type="button" className="conference__secondary" onClick={() => join({ request: true })}>
              Request to join
            </button>
          )}
        </div>
      </section>
    )
  }

  if (request === 'pending') {
    return (
      <section className="conference__panel" aria-label="Joining">
        <p className="conference__status">Your request to join is pending. An organizer will look at it.</p>
      </section>
    )
  }

  const submit = (e: FormEvent) => {
    e.preventDefault()
    void join(code.trim() ? { code: code.trim() } : {})
  }
  const canJoin = methods.has('code') || methods.has('list') || methods.has('open')
  const busy = step.kind === 'busy'
  return (
    <section className="conference__panel" aria-label="Joining">
      {request === 'denied' && <p className="conference__status">You weren’t admitted to this conference.</p>}
      {step.kind === 'message' && <p role="alert">{step.text}</p>}
      {canJoin && (
        <form className="conference__join" onSubmit={submit}>
          {entering && (
            <>
              <label htmlFor="conference-code">Invite code</label>
              <input
                id="conference-code"
                name="code"
                type="text"
                autoCapitalize="none"
                autoComplete="off"
                spellCheck={false}
                // biome-ignore lint/a11y/noAutofocus: shown because the person asked to enter a code
                autoFocus
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />
            </>
          )}
          <div className="conference__actions">
            {(entering || methods.has('list') || methods.has('open')) && (
              <button type="submit" className="conference__primary" disabled={busy}>
                Join
              </button>
            )}
            {methods.has('code') && !entering && (
              <button type="button" className="conference__secondary" onClick={() => setEntering(true)}>
                Enter a code
              </button>
            )}
          </div>
        </form>
      )}
      {methods.has('request') && request !== 'denied' && (
        <button type="button" className="conference__secondary" onClick={() => join({ request: true })} disabled={busy}>
          Request to join
        </button>
      )}
      {!canJoin && !methods.has('request') && (
        <p className="conference__muted">This conference isn’t taking new members right now.</p>
      )}
    </section>
  )
}

/** `/join/:code`: an invite link. It names no conference: the code does. */
export function JoinByCodePage() {
  const { code = '' } = useParams()
  const { session } = useSession()
  const location = useLocation()
  const [joined, setJoined] = useState<string | undefined>()
  const [problem, setProblem] = useState<string | undefined>()
  const [busy, setBusy] = useState(false)

  if (joined) return <ConferencePage uri={joined} />

  if (session.kind !== 'signedIn') {
    return (
      <main className="conference">
        <h1>You’re invited</h1>
        <p>
          <Link to={`/signin?${new URLSearchParams({ return_to: location.pathname })}`}>Sign in</Link> with your atproto
          account to join.
        </p>
      </main>
    )
  }

  const join = async () => {
    setBusy(true)
    setProblem(undefined)
    try {
      const res = await api('/xrpc/app.eventside.conference.join', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ code }),
      })
      const body = await res.json().catch(() => ({}))
      if (res.status === 400 && body.error === 'InvalidCode') return setProblem('This invite isn’t valid any more.')
      if (!res.ok) return setProblem('Joining didn’t work. Please try again.')
      if (body.status === 'joined' && body.conference) return setJoined(body.conference)
      if (body.status === 'pending') return setProblem('Your request to join is pending.')
      setProblem('You can’t join with this invite.')
    } catch {
      setProblem('Joining didn’t work. Check your connection and try again.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="conference">
      <h1>You’re invited</h1>
      <p>Join to see what’s inside.</p>
      {problem && <p role="alert">{problem}</p>}
      <button type="button" className="conference__primary" onClick={join} disabled={busy}>
        Join
      </button>
    </main>
  )
}

/** `/conferences`: the conferences you're a member of. */
export function MyConferencesPage() {
  const { session } = useSession()
  const [conferences, setConferences] = useState<ConferenceView[] | undefined>()
  const [failed, setFailed] = useState(false)
  const signedIn = session.kind === 'signedIn'

  useEffect(() => {
    if (!signedIn) return
    const controller = new AbortController()
    api('/xrpc/app.eventside.conference.listMyConferences', { signal: controller.signal })
      .then(async (res) => {
        if (!res.ok) throw new Error(`listMyConferences answered ${res.status}`)
        setConferences((await res.json()).conferences)
      })
      .catch(() => {
        if (!controller.signal.aborted) setFailed(true)
      })
    return () => controller.abort()
  }, [signedIn])

  return (
    <main className="conference">
      <h1>Your conferences</h1>
      {!signedIn && session.kind !== 'loading' && (
        <p>
          <Link to={`/signin?${new URLSearchParams({ return_to: '/conferences' })}`}>Sign in</Link> to see your
          conferences.
        </p>
      )}
      {failed && <p role="alert">Your conferences couldn’t be loaded. Check your connection and try again.</p>}
      {conferences?.length === 0 && <p className="conference__muted">You haven’t joined any conferences yet.</p>}
      {conferences && conferences.length > 0 && (
        <ul className="conference__list">
          {conferences.map((c) => (
            <li key={c.space}>
              <Link to={conferencePath(c)}>{c.name}</Link>
              {dateRange(c.startsAt, c.endsAt) && (
                <span className="conference__muted"> {dateRange(c.startsAt, c.endsAt)}</span>
              )}
            </li>
          ))}
        </ul>
      )}
    </main>
  )
}
