import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router'
import { useSession } from '../auth/session'
import { type EventSummary, events } from './api'
import { EventForm, EventTile } from './parts'
import './groups.css'

/** Signed out: what this is. Signed in: events. */
export default function Home() {
  const { session } = useSession()
  if (session.kind === 'loading') return <main className="gx-page" />
  if (session.kind !== 'signedIn') return <Welcome />
  return <MyEvents />
}

function Welcome() {
  return (
    <main className="gx-page gx-welcome">
      <p className="gx-eyebrow">Eventside</p>
      <h1 className="gx-welcome__title">Your event, live.</h1>
      <p className="gx-welcome__lede">
        Say you’re going, and the event opens up: live polls, questions and games, and the plans people make inside it.
        Sign in with your atmosphere account (Bluesky or any PDS) to RSVP to AtmosphereConf 2027.
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

function MyEvents() {
  const navigate = useNavigate()
  const [list, setList] = useState<EventSummary[] | null>(null)
  const [failed, setFailed] = useState(false)
  const [creating, setCreating] = useState(false)

  useEffect(() => {
    events
      .list()
      .then(setList)
      .catch(() => setFailed(true))
  }, [])

  const topLevel = (list ?? []).filter((e) => !e.parent)
  const going = topLevel.filter((e) => e.rsvp === 'going')
  const upcoming = topLevel.filter((e) => e.rsvp !== 'going')
  // Plans inside events you're going to, under their event.
  const plans = new Map<string, { title: string; events: EventSummary[] }>()
  for (const e of list ?? []) {
    if (!e.parent) continue
    const group = plans.get(e.parent.id) ?? { title: e.parent.title, events: [] }
    group.events.push(e)
    plans.set(e.parent.id, group)
  }

  return (
    <main className="gx-page">
      <div className="gx-page-head">
        <h1 className="gx-title">Events</h1>
        {!creating && (
          <button type="button" className="gx-button gx-button--primary" onClick={() => setCreating(true)}>
            Create an event
          </button>
        )}
      </div>
      {creating && (
        <section className="gx-panel" aria-label="Create an event">
          <h2 className="gx-panel__title">Create an event</h2>
          <EventForm
            submitLabel="Create event"
            onCancel={() => setCreating(false)}
            onSubmit={async (event) => {
              const created = await events.create(event)
              navigate(`/e/${created.id}`)
            }}
          />
        </section>
      )}
      {failed && <p className="gx-muted">Couldn’t load events. Try reloading.</p>}
      {list === null && !failed && <p className="gx-muted">Loading…</p>}

      {going.length > 0 && (
        <section className="gx-section" aria-labelledby="going-head">
          <h2 id="going-head" className="gx-section__title">
            Going
          </h2>
          <ul className="gx-event-list">
            {going.map((e) => (
              <li key={e.id}>
                <EventTile event={e} />
              </li>
            ))}
          </ul>
        </section>
      )}

      {[...plans.entries()].map(([parentId, group]) => (
        <section key={parentId} className="gx-section" aria-labelledby={`plans-${parentId}`}>
          <h2 id={`plans-${parentId}`} className="gx-section__title">
            Inside {group.title}
          </h2>
          <ul className="gx-event-list">
            {group.events.map((e) => (
              <li key={e.id}>
                <EventTile event={e} showParent={false} />
              </li>
            ))}
          </ul>
        </section>
      ))}

      {upcoming.length > 0 && (
        <section className="gx-section" aria-labelledby="upcoming-head">
          <h2 id="upcoming-head" className="gx-section__title">
            Upcoming
          </h2>
          <ul className="gx-event-list">
            {upcoming.map((e) => (
              <li key={e.id}>
                <EventTile event={e} />
              </li>
            ))}
          </ul>
        </section>
      )}
      {list?.length === 0 && <p className="gx-muted">No events yet. Create the first one.</p>}
    </main>
  )
}
