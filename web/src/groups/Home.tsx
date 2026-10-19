import { type FormEvent, useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router'
import { useSession } from '../auth/session'
import { type GroupSummary, groups } from './api'
import './groups.css'

/** Signed out: what this is. Signed in: your groups. */
export default function Home() {
  const { session } = useSession()
  if (session.kind === 'loading') return <main className="gx-page" />
  if (session.kind !== 'signedIn') return <Welcome />
  return <MyGroups />
}

function Welcome() {
  return (
    <main className="gx-page gx-welcome">
      <p className="gx-eyebrow">Eventside</p>
      <h1 className="gx-welcome__title">Your group, live.</h1>
      <p className="gx-welcome__lede">
        Groups have feeds. Feeds have cards. Polls, questions, sign-ups and games update for everyone as people tap.
        Sign in with your atmosphere account (Bluesky or any PDS) to join AtmosphereConf 2027.
      </p>
      <Link className="gx-button gx-button--primary" to="/signin?return_to=%2F">
        Sign in
      </Link>
      <HealthLine />
    </main>
  )
}

type Health = { status: string; atproto: 'reachable' | 'unreachable' }

/** Whether the whole stack is up: the backend, and the atproto network it signs people in through. */
function HealthLine() {
  const [health, setHealth] = useState<Health | 'failed' | null>(null)
  useEffect(() => {
    const controller = new AbortController()
    fetch('/health', { signal: controller.signal })
      .then((res) => (res.ok ? (res.json() as Promise<Health>) : Promise.reject(new Error(`HTTP ${res.status}`))))
      .then(setHealth)
      .catch(() => {
        if (!controller.signal.aborted) setHealth('failed')
      })
    return () => controller.abort()
  }, [])
  if (health === null) return null
  return (
    <p className="gx-health">
      {health === 'failed' ? (
        'Backend: unavailable'
      ) : (
        <>
          <span>Backend: {health.status}</span>
          <span>atproto: {health.atproto === 'reachable' ? 'reachable' : 'offline'}</span>
        </>
      )}
    </p>
  )
}

function MyGroups() {
  const navigate = useNavigate()
  const [list, setList] = useState<GroupSummary[] | null>(null)
  const [failed, setFailed] = useState(false)
  const [name, setName] = useState('')
  const [invite, setInvite] = useState('')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    groups
      .list()
      .then(setList)
      .catch(() => setFailed(true))
  }, [])

  const create = (e: FormEvent) => {
    e.preventDefault()
    if (!name.trim()) return
    groups
      .create(name.trim(), '')
      .then(({ id }) => navigate(`/g/${id}`))
      .catch((err: Error) => setError(err.message))
  }

  const join = (e: FormEvent) => {
    e.preventDefault()
    if (!invite.trim()) return
    groups
      .join(invite.trim())
      .then(({ id }) => navigate(`/g/${id}`))
      .catch((err: Error) => setError(err.message))
  }

  return (
    <main className="gx-page">
      <h1 className="gx-title">Your groups</h1>
      {failed && <p className="gx-muted">Couldn’t load your groups. Try reloading.</p>}
      {list === null && !failed && <p className="gx-muted">Loading…</p>}
      <ul className="gx-groups">
        {list?.map((g) => (
          <li key={g.id}>
            <Link className="gx-group" to={`/g/${g.id}`}>
              <span className="gx-group__name">{g.name}</span>
              {g.description && <span className="gx-group__desc">{g.description}</span>}
              <span className="gx-group__meta">
                {g.memberCount} {g.memberCount === 1 ? 'person' : 'people'} · {g.feedCount}{' '}
                {g.feedCount === 1 ? 'feed' : 'feeds'}
                {g.role === 'owner' && ' · yours'}
              </span>
            </Link>
          </li>
        ))}
      </ul>

      <div className="gx-forms">
        <form className="gx-inline-form" onSubmit={create}>
          <label htmlFor="new-group">Start a group</label>
          <div className="gx-inline-form__row">
            <input
              id="new-group"
              className="gx-input"
              placeholder="Group name"
              value={name}
              maxLength={100}
              onChange={(e) => setName(e.target.value)}
            />
            <button type="submit" className="gx-button gx-button--primary">
              Create
            </button>
          </div>
        </form>
        <form className="gx-inline-form" onSubmit={join}>
          <label htmlFor="invite">Join with an invite code</label>
          <div className="gx-inline-form__row">
            <input
              id="invite"
              className="gx-input gx-input--code"
              placeholder="ABC123"
              value={invite}
              onChange={(e) => setInvite(e.target.value.toUpperCase())}
            />
            <button type="submit" className="gx-button">
              Join
            </button>
          </div>
        </form>
        {error && (
          <p className="gx-error" role="alert">
            {error}
          </p>
        )}
      </div>
    </main>
  )
}
