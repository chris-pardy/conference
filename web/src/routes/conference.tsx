import { type FormEvent, useCallback, useEffect, useState } from 'react'
import { Link, useLocation, useNavigate, useParams } from 'react-router'
import { api } from '../api'
import { useSession } from '../auth/session'
import './conference.css'

/** A conference as `app.eventside.conference.get` describes it. */
export interface ConferenceView {
  space: string
  event?: string
  name: string
  startsAt?: string
  endsAt?: string
  locations?: { locality?: string; name?: string; region?: string; country?: string }[]
  description?: string
  theme?: Record<string, string>
  join?: { methods: string[] }
  viewer?: { member: boolean; role?: string }
}

const EVENT = 'community.lexicon.calendar.event'

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
  if (!location) return undefined
  const parts = [location.name, location.locality, location.region].filter(Boolean)
  return [...new Set(parts)].join(', ') || undefined
}

type Load =
  | { kind: 'loading' }
  | { kind: 'missing' }
  | { kind: 'failed' }
  | { kind: 'ready'; conference: ConferenceView }

/** Loads a conference by its event's AT-URI, as whoever is signed in sees it. */
function useConference(uri: string | undefined): [Load, () => void] {
  const [load, setLoad] = useState<Load>({ kind: 'loading' })
  const [attempt, setAttempt] = useState(0)
  const { session } = useSession()
  const signedIn = session.kind === 'signedIn' ? session.user.did : session.kind
  // biome-ignore lint/correctness/useExhaustiveDependencies: reload when who's signed in changes, or on demand
  useEffect(() => {
    if (!uri) return
    const controller = new AbortController()
    api(`/xrpc/app.eventside.conference.get?${new URLSearchParams({ conference: uri })}`, {
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

/** `/c/:actor/:rkey`: a public conference, by its event (the organization's DID or handle). */
export default function PublicConferencePage() {
  const { actor, rkey } = useParams()
  const [load, reload] = useConference(actor && rkey ? `at://${actor}/${EVENT}/${rkey}` : undefined)
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
        <p className="conference__muted">There’s no conference at this address.</p>
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

/** What a member sees until feeds land: they're in, and they can leave. */
function Inside({ conference, reload }: { conference: ConferenceView; reload: () => void }) {
  const [leaving, setLeaving] = useState(false)
  const [problem, setProblem] = useState<string | undefined>()

  const leave = async () => {
    setLeaving(true)
    setProblem(undefined)
    try {
      const res = await api('/xrpc/app.eventside.conference.leave', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ conference: conference.space }),
      })
      const body = await res.json().catch(() => ({}))
      if (body.error === 'LastOwner') return setProblem('You’re the only owner: make someone else an owner first.')
      if (!res.ok) throw new Error(`leave answered ${res.status}`)
      reload()
    } catch {
      setProblem('Couldn’t leave. Check your connection and try again.')
    } finally {
      setLeaving(false)
    }
  }

  return (
    <section className="conference__panel" aria-label="Inside the conference">
      <p className="conference__status">You’re in{conference.viewer?.role ? `, as ${conference.viewer.role}` : ''}.</p>
      {problem && <p role="alert">{problem}</p>}
      <button type="button" className="conference__secondary" onClick={leave} disabled={leaving}>
        Leave
      </button>
    </section>
  )
}

/** How someone who isn't a member gets in, by the methods the conference has on. */
function WayIn({ conference, reload }: { conference: ConferenceView; reload: () => void }) {
  const { session } = useSession()
  const location = useLocation()
  const navigate = useNavigate()
  const [code, setCode] = useState('')
  const [entering, setEntering] = useState(false)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string | undefined>()
  const methods = new Set(conference.join?.methods ?? [])
  const canJoin = methods.has('code') || methods.has('list') || methods.has('open')

  if (session.kind !== 'signedIn') {
    const signIn = `/signin?${new URLSearchParams({ return_to: location.pathname + location.search })}`
    return (
      <section className="conference__panel" aria-label="Joining">
        {canJoin ? (
          <>
            <p>
              {methods.has('list') ? 'On the attendee list? ' : ''}
              <Link to={signIn}>Sign in</Link>
            </p>
            {methods.has('code') && (
              <div className="conference__actions">
                <button type="button" className="conference__secondary" onClick={() => navigate(signIn)}>
                  Enter a code
                </button>
              </div>
            )}
          </>
        ) : (
          <p className="conference__muted">This conference isn’t taking new members right now.</p>
        )}
      </section>
    )
  }

  const join = async (input: { code?: string }) => {
    setBusy(true)
    setMessage(undefined)
    try {
      const res = await api('/xrpc/app.eventside.conference.join', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ conference: conference.space, ...input }),
      })
      if (res.status === 429) return setMessage('Too many tries. Wait a few minutes and try again.')
      if (!res.ok) return setMessage('Joining didn’t work. Please try again.')
      const answer = await res.json().catch(() => ({}))
      if (answer.status !== 'joined') return setMessage('That didn’t get you in. Check the code and try again.')
      reload()
    } catch {
      setMessage('Joining didn’t work. Check your connection and try again.')
    } finally {
      setBusy(false)
    }
  }

  const submit = (e: FormEvent) => {
    e.preventDefault()
    void join(code.trim() ? { code: code.trim() } : {})
  }
  return (
    <section className="conference__panel" aria-label="Joining">
      {message && <p role="alert">{message}</p>}
      {canJoin ? (
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
      ) : (
        <p className="conference__muted">This conference isn’t taking new members right now.</p>
      )}
    </section>
  )
}
