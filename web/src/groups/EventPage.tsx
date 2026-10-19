import { type FormEvent, useCallback, useEffect, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { useSession } from '../auth/session'
import {
  type CardView,
  cards as cardsApi,
  type EventDetail,
  type EventSummary,
  events,
  type Feed,
  feeds as feedsApi,
  type Rsvp,
} from './api'
import { Composer } from './Composer'
import { when, zoneName } from './format'
import { LiveCard, serverActions, type Viewer } from './LiveCard'
import { EventForm, EventTile, Faces, PersonChip } from './parts'
import './groups.css'

/** The plans tab, beside the feeds. */
const PLANS = 'plans'

/** An event: who's going, and for those going, its feeds and the plans made inside it, live. */
export default function EventPage() {
  const { session } = useSession()
  if (session.kind === 'loading') return <main className="gx-page" />
  if (session.kind !== 'signedIn') {
    return (
      <main className="gx-page">
        <p>
          <Link to={`/signin?${new URLSearchParams({ return_to: location.pathname })}`}>Sign in</Link> to see this
          event.
        </p>
      </main>
    )
  }
  return <SignedInEvent viewer={{ did: session.user.did, handle: session.user.handle }} />
}

/** A first start time for a plan: the next whole hour, or the event's start if that's later. */
function nextHour(event: EventSummary): number {
  const hour = 3_600_000
  const from = Math.max(Date.now(), event.startsAt)
  return Math.ceil(from / hour) * hour
}

function SignedInEvent({ viewer }: { viewer: Viewer }) {
  const { eventId = '', tab } = useParams()
  const navigate = useNavigate()
  const [event, setEvent] = useState<EventDetail | null>(null)
  const [feedList, setFeedList] = useState<Feed[]>([])
  const [subEvents, setSubEvents] = useState<EventSummary[]>([])
  const [missing, setMissing] = useState(false)
  const [cardList, setCardList] = useState<CardView[] | null>(null)
  const [composing, setComposing] = useState(false)
  const [planning, setPlanning] = useState(false)
  const [live, setLive] = useState(false)
  const [newFeed, setNewFeed] = useState<string | null>(null)
  const [rsvping, setRsvping] = useState<Rsvp | null>(null)
  const [rsvpError, setRsvpError] = useState<string | null>(null)

  const canView = event?.canView ?? false
  const onPlans = tab === PLANS
  const current = onPlans ? undefined : (tab ?? feedList[0]?.id)

  const load = useCallback(() => {
    events
      .get(eventId)
      .then(({ event, feeds, subEvents }) => {
        setEvent(event)
        setFeedList(feeds)
        setSubEvents(subEvents)
      })
      .catch(() => setMissing(true))
  }, [eventId])

  useEffect(() => {
    setEvent(null)
    setMissing(false)
    setPlanning(false)
    setComposing(false)
    load()
  }, [load])

  const loadCards = useCallback(() => {
    if (!current || !canView) return
    feedsApi
      .cards(current)
      .then(setCardList)
      .catch(() => setCardList([]))
  }, [current, canView])

  useEffect(() => {
    setCardList(null)
    loadCards()
  }, [loadCards])

  const upsert = useCallback(
    (card: CardView) => {
      if (card.feedId !== current) return
      setCardList((list) => {
        if (!list) return list
        const at = list.findIndex((c) => c.id === card.id)
        if (at === -1) return [card, ...list]
        // An older copy (a dispatch answer that lost the race with the stream) never wins.
        if (list[at].version > card.version) return list
        const next = list.slice()
        next[at] = card
        return next
      })
    },
    [current],
  )

  const upsertSubEvent = useCallback((e: EventSummary) => {
    setSubEvents((list) => {
      const at = list.findIndex((x) => x.id === e.id)
      const next = at === -1 ? [...list, e] : list.map((x) => (x.id === e.id ? e : x))
      return next.sort((a, b) => a.startsAt - b.startsAt)
    })
  }, [])

  // Updates come down the event's stream as generic payloads, for those going.
  useEffect(() => {
    if (!eventId || !canView) {
      setLive(false)
      return
    }
    return events.subscribe(eventId, (payload) => {
      switch (payload.type) {
        case 'hello':
          setLive(true)
          break
        case 'resync':
          load()
          loadCards()
          break
        case 'card':
          upsert(payload.card)
          break
        case 'cardRemoved':
          setCardList((list) => list?.filter((c) => c.id !== payload.cardId) ?? list)
          break
        case 'feed':
          setFeedList((list) => (list.some((f) => f.id === payload.feed.id) ? list : [...list, payload.feed]))
          break
        case 'actions':
          serverActions.deliver(payload.cardId, payload.actions)
          break
        case 'event':
          setEvent(payload.event)
          break
        case 'subEvent':
          upsertSubEvent(payload.event)
          break
      }
    })
  }, [eventId, canView, upsert, upsertSubEvent, load, loadCards])

  const rsvp = (status: Rsvp) => {
    if (!event) return
    setRsvping(status)
    setRsvpError(null)
    events
      .rsvp(event.id, status)
      .then((updated) => {
        setEvent(updated)
        // Going opens the event up: fetch its feeds and plans.
        if (updated.canView !== event.canView) load()
      })
      .catch((err: Error) => setRsvpError(err.message))
      .finally(() => setRsvping(null))
  }

  const addFeed = (e: FormEvent) => {
    e.preventDefault()
    const name = newFeed?.trim()
    if (!name) return
    feedsApi.create(eventId, name).then((feed) => {
      setFeedList((list) => (list.some((f) => f.id === feed.id) ? list : [...list, feed]))
      setNewFeed(null)
      navigate(`/e/${eventId}/${feed.id}`)
    })
  }

  if (missing) {
    return (
      <main className="gx-page">
        <p>This event doesn’t exist, or you can’t see it.</p>
        <Link to="/">Events</Link>
      </main>
    )
  }
  if (!event) return <main className="gx-page" />

  const isHost = event.host === viewer.did

  return (
    <main className="gx-page gx-event-page">
      <header className="gx-event-head">
        {event.parent ? (
          <Link to={`/e/${event.parent.id}/${PLANS}`} className="gx-back">
            ← {event.parent.title}
          </Link>
        ) : (
          <Link to="/" className="gx-back">
            ← Events
          </Link>
        )}
        {event.parent && <p className="gx-eyebrow">A plan inside {event.parent.title}</p>}
        <h1 className="gx-title">{event.title}</h1>
        <dl className="gx-event-facts">
          <div>
            <dt>When</dt>
            <dd>
              {when(event.startsAt, event.endsAt, event.timeZone)}
              <span className="gx-muted"> · {zoneName(event.timeZone)}</span>
            </dd>
          </div>
          {event.location && (
            <div>
              <dt>Where</dt>
              <dd>{event.location}</dd>
            </div>
          )}
          {event.host && (
            <div>
              <dt>Host</dt>
              <dd>
                <PersonChip did={event.host} />
              </dd>
            </div>
          )}
        </dl>
        {event.description && <p className="gx-event-desc">{event.description}</p>}

        <div className="gx-attending">
          {event.attendees.length > 0 && <Faces dids={event.attendees} total={event.going} />}
          <span className="gx-attending__count">
            <strong>{event.going}</strong> going
            {event.maybe > 0 && (
              <>
                {' · '}
                <strong>{event.maybe}</strong> maybe
              </>
            )}
          </span>
          {canView && <span className={`gx-live ${live ? 'gx-live--on' : ''}`}>{live ? 'Live' : 'Connecting…'}</span>}
        </div>

        <fieldset className="gx-rsvp" disabled={rsvping !== null || (isHost && event.rsvp === 'going')}>
          <legend className="gx-visually-hidden">Are you going?</legend>
          {(
            [
              ['going', 'I’m going'],
              ['maybe', 'Maybe'],
              ['no', 'Can’t go'],
            ] as const
          ).map(([status, label]) => (
            <button
              key={status}
              type="button"
              className={`gx-rsvp__option gx-rsvp__option--${status}`}
              aria-pressed={event.rsvp === status}
              onClick={() => rsvp(status)}
            >
              {rsvping === status ? '…' : label}
            </button>
          ))}
        </fieldset>
        {isHost && <p className="gx-hint">You’re hosting, so you’re going.</p>}
        {rsvpError && (
          <p className="gx-error" role="alert">
            {rsvpError}
          </p>
        )}
      </header>

      {!canView ? (
        <section className="gx-locked" aria-label="Locked">
          <span className="gx-locked__icon" aria-hidden="true">
            🔒
          </span>
          <p className="gx-locked__title">Say you’re going to see the feeds, people and plans</p>
          <p className="gx-muted">
            Live polls, questions and games happen inside
            {event.subEventCount > 0
              ? `, along with ${event.subEventCount} ${event.subEventCount === 1 ? 'plan' : 'plans'} people have made.`
              : '.'}
          </p>
          <button
            type="button"
            className="gx-button gx-button--primary"
            disabled={rsvping !== null}
            onClick={() => rsvp('going')}
          >
            I’m going
          </button>
        </section>
      ) : (
        <>
          <nav className="gx-tabs" aria-label="Feeds and plans">
            {feedList.map((f) => (
              <Link
                key={f.id}
                to={`/e/${eventId}/${f.id}`}
                className="gx-tab"
                aria-current={f.id === current ? 'page' : undefined}
              >
                {f.name}
              </Link>
            ))}
            <Link
              to={`/e/${eventId}/${PLANS}`}
              className="gx-tab gx-tab--plans"
              aria-current={onPlans ? 'page' : undefined}
            >
              Plans
              {subEvents.length > 0 && <span className="gx-tab__count">{subEvents.length}</span>}
            </Link>
            {newFeed === null ? (
              <button type="button" className="gx-tab gx-tab--add" onClick={() => setNewFeed('')}>
                + Feed
              </button>
            ) : (
              <form className="gx-tab-form" onSubmit={addFeed}>
                <input
                  className="gx-input"
                  // biome-ignore lint/a11y/noAutofocus: opened by the person just now
                  autoFocus
                  aria-label="New feed name"
                  placeholder="Feed name"
                  maxLength={60}
                  value={newFeed}
                  onChange={(e) => setNewFeed(e.target.value)}
                  onKeyDown={(e) => e.key === 'Escape' && setNewFeed(null)}
                />
                <button type="submit" className="gx-button gx-button--primary">
                  Add
                </button>
              </form>
            )}
          </nav>

          {onPlans ? (
            <Plans
              event={event}
              feeds={feedList}
              subEvents={subEvents}
              planning={planning}
              setPlanning={setPlanning}
              onCreated={(created) => {
                upsertSubEvent(created)
                setPlanning(false)
              }}
            />
          ) : (
            <>
              {current && (
                <div className="gx-feed-actions">
                  <button
                    type="button"
                    className="gx-button"
                    onClick={() => {
                      setPlanning(true)
                      navigate(`/e/${eventId}/${PLANS}`)
                    }}
                  >
                    Make a plan
                  </button>
                  <button type="button" className="gx-button gx-button--primary" onClick={() => setComposing(true)}>
                    New card
                  </button>
                </div>
              )}

              {composing && current && (
                <Composer
                  feedId={current}
                  viewer={viewer}
                  onClose={() => setComposing(false)}
                  onPosted={(card) => {
                    upsert(card)
                    setComposing(false)
                  }}
                />
              )}

              <section className="gx-feed" aria-label="Cards">
                {cardList === null && <p className="gx-muted">Loading…</p>}
                {cardList?.length === 0 && <p className="gx-muted">No cards in this feed yet.</p>}
                {cardList?.map((card) => (
                  <LiveCard
                    key={card.id}
                    card={card}
                    viewer={viewer}
                    onUpdate={upsert}
                    canRemove={card.author === viewer.did || isHost}
                    onRemove={() =>
                      cardsApi.remove(card.id).then(() => setCardList((l) => l?.filter((c) => c.id !== card.id) ?? l))
                    }
                  />
                ))}
              </section>
            </>
          )}
        </>
      )}
    </main>
  )
}

interface PlansProps {
  event: EventDetail
  feeds: Feed[]
  subEvents: EventSummary[]
  planning: boolean
  setPlanning(open: boolean): void
  onCreated(event: EventSummary): void
}

/** Plans: events made inside this one, seen only by the people going to it. */
function Plans({ event, feeds, subEvents, planning, setPlanning, onCreated }: PlansProps) {
  return (
    <section className="gx-plans" aria-label="Plans">
      <div className="gx-plan-hero">
        <div>
          <h2 className="gx-plan-hero__title">Make a plan</h2>
          <p className="gx-plan-hero__lede">
            Dinner, a canal walk, a side meetup. A plan is its own event inside {event.title}, with its own RSVPs and
            feed. Only people going to {event.title} can see it.
          </p>
        </div>
        {!planning && (
          <button type="button" className="gx-button gx-button--primary" onClick={() => setPlanning(true)}>
            Make a plan
          </button>
        )}
      </div>
      {planning && (
        <div className="gx-panel">
          <EventForm
            timeZone={event.timeZone}
            feeds={feeds}
            defaultStart={nextHour(event)}
            submitLabel="Make the plan"
            onCancel={() => setPlanning(false)}
            onSubmit={async (draft) => {
              onCreated(await events.create({ ...draft, parentId: event.id }))
            }}
          />
        </div>
      )}
      {subEvents.length === 0 ? (
        <p className="gx-muted">No plans yet. Make the first one.</p>
      ) : (
        <ul className="gx-event-list">
          {subEvents.map((e) => (
            <li key={e.id}>
              <EventTile event={e} showParent={false} />
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
