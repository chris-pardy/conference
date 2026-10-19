import { type FormEvent, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { type EventSummary, type Feed, type NewEvent, type Profile, profile, type Rsvp } from './api'
import { dateBadge, fromLocalInput, localZone, toLocalInput, when, zoneName } from './format'

/** A person's profile, once loaded. */
export function useProfile(did: string | null | undefined): Profile | null {
  const [p, setP] = useState<Profile | null>(null)
  useEffect(() => {
    let live = true
    setP(null)
    if (did) profile(did).then((found) => live && setP(found))
    return () => {
      live = false
    }
  }, [did])
  return p
}

const nameOf = (p: Profile | null, did: string) => p?.displayName?.trim() || p?.handle || `${did.slice(0, 16)}…`

export function Avatar({ did, size = 32 }: { did: string; size?: number }) {
  const p = useProfile(did)
  const name = nameOf(p, did)
  const style = { width: size, height: size, fontSize: size * 0.42 }
  return p?.avatar ? (
    <img className="gx-avatar" src={p.avatar} alt="" title={name} style={style} />
  ) : (
    <span className="gx-avatar gx-avatar--none" title={name} style={style} aria-hidden="true">
      {name.replace(/^@/, '').slice(0, 1).toUpperCase()}
    </span>
  )
}

/** A person's avatar and name. */
export function PersonChip({ did, prefix }: { did: string; prefix?: string }) {
  const p = useProfile(did)
  return (
    <span className="gx-person">
      <Avatar did={did} size={24} />
      <span>
        {prefix}
        {nameOf(p, did)}
      </span>
    </span>
  )
}

/** Overlapping faces, with a count of the rest. */
export function Faces({ dids, total, max = 8 }: { dids: string[]; total: number; max?: number }) {
  const shown = dids.slice(0, max)
  const more = Math.max(0, total - shown.length)
  return (
    <span className="gx-faces">
      {shown.map((did) => (
        <Avatar key={did} did={did} size={32} />
      ))}
      {more > 0 && <span className="gx-faces__more">+{more}</span>}
    </span>
  )
}

const RSVP_LABEL: Record<Rsvp, string> = { going: 'Going', maybe: 'Maybe', no: 'Not going' }

export function RsvpBadge({ rsvp }: { rsvp: Rsvp | null }) {
  if (!rsvp) return null
  return <span className={`gx-rsvp-badge gx-rsvp-badge--${rsvp}`}>{RSVP_LABEL[rsvp]}</span>
}

/** An event in a list. */
export function EventTile({ event, showParent = true }: { event: EventSummary; showParent?: boolean }) {
  const [d, m] = dateBadge(event.startsAt, event.timeZone)
  return (
    <Link className="gx-event-tile" to={`/e/${event.id}`}>
      <span className="gx-date" aria-hidden="true">
        <span className="gx-date__m">{m}</span>
        <span className="gx-date__d">{d}</span>
      </span>
      <span className="gx-event-tile__body">
        {showParent && event.parent && <span className="gx-event-tile__parent">In {event.parent.title}</span>}
        <span className="gx-event-tile__title">{event.title}</span>
        <span className="gx-event-tile__meta">{when(event.startsAt, event.endsAt, event.timeZone)}</span>
        {event.location && <span className="gx-event-tile__meta">{event.location}</span>}
        <span className="gx-event-tile__foot">
          <span>
            {event.going} going{event.maybe > 0 && ` · ${event.maybe} maybe`}
            {event.subEventCount > 0 && ` · ${event.subEventCount} ${event.subEventCount === 1 ? 'plan' : 'plans'}`}
          </span>
          <RsvpBadge rsvp={event.rsvp} />
        </span>
      </span>
    </Link>
  )
}

interface EventFormProps {
  /** The zone times are entered in. */
  timeZone?: string
  /** For a plan: the parent's feeds, one of which gets the plan's card. */
  feeds?: Feed[]
  defaultFeedId?: string
  /** A sensible first start time. */
  defaultStart?: number
  submitLabel: string
  onSubmit(event: NewEvent): Promise<void>
  onCancel?(): void
}

/** The fields of a new event, or a new plan inside one. */
export function EventForm({
  timeZone = localZone(),
  feeds,
  defaultFeedId,
  defaultStart,
  submitLabel,
  onSubmit,
  onCancel,
}: EventFormProps) {
  const [title, setTitle] = useState('')
  const [start, setStart] = useState(defaultStart ? toLocalInput(defaultStart, timeZone) : '')
  const [end, setEnd] = useState(defaultStart ? toLocalInput(defaultStart + 2 * 3_600_000, timeZone) : '')
  const [location, setLocation] = useState('')
  const [description, setDescription] = useState('')
  const [feedId, setFeedId] = useState(defaultFeedId ?? feeds?.[0]?.id ?? '')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    const startsAt = fromLocalInput(start, timeZone)
    const endsAt = end ? fromLocalInput(end, timeZone) : null
    if (!title.trim()) return setError('Give it a name.')
    if (startsAt === null) return setError('When does it start?')
    if (endsAt !== null && endsAt <= startsAt) return setError('It has to end after it starts.')
    setBusy(true)
    setError(null)
    try {
      await onSubmit({
        title: title.trim(),
        description: description.trim(),
        startsAt,
        endsAt,
        timeZone,
        location: location.trim(),
        ...(feeds && feedId ? { feedId } : {}),
      })
      setTitle('')
      setLocation('')
      setDescription('')
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <form className="gx-event-form" onSubmit={(e) => void submit(e)}>
      <label className="gx-field">
        <span>What</span>
        <input
          className="gx-input"
          placeholder={feeds ? 'Dinner at Café de Jaren' : 'Event name'}
          value={title}
          maxLength={100}
          onChange={(e) => setTitle(e.target.value)}
        />
      </label>
      <div className="gx-field-row">
        <label className="gx-field">
          <span>Starts</span>
          <input
            className="gx-input"
            type="datetime-local"
            value={start}
            onChange={(e) => {
              setStart(e.target.value)
              const s = fromLocalInput(e.target.value, timeZone)
              const old = fromLocalInput(end, timeZone)
              if (s !== null && (old === null || old <= s)) setEnd(toLocalInput(s + 2 * 3_600_000, timeZone))
            }}
          />
        </label>
        <label className="gx-field">
          <span>Ends</span>
          <input className="gx-input" type="datetime-local" value={end} onChange={(e) => setEnd(e.target.value)} />
        </label>
      </div>
      <p className="gx-hint">Times in {zoneName(timeZone)}</p>
      <label className="gx-field">
        <span>Where</span>
        <input
          className="gx-input"
          placeholder={feeds ? 'Westerpark, by the pond' : 'Venue, city'}
          value={location}
          maxLength={200}
          onChange={(e) => setLocation(e.target.value)}
        />
      </label>
      <label className="gx-field">
        <span>Details</span>
        <textarea
          className="gx-input gx-textarea"
          placeholder={feeds ? 'Who it’s for, what to bring…' : 'What it is, who it’s for'}
          value={description}
          maxLength={2000}
          onChange={(e) => setDescription(e.target.value)}
        />
      </label>
      {feeds && feeds.length > 0 && (
        <label className="gx-field">
          <span>Announce it in</span>
          <select className="gx-input" value={feedId} onChange={(e) => setFeedId(e.target.value)}>
            {feeds.map((f) => (
              <option key={f.id} value={f.id}>
                {f.name}
              </option>
            ))}
          </select>
        </label>
      )}
      {error && (
        <p className="gx-error" role="alert">
          {error}
        </p>
      )}
      <div className="gx-event-form__foot">
        {onCancel && (
          <button type="button" className="gx-button" onClick={onCancel}>
            Cancel
          </button>
        )}
        <button type="submit" className="gx-button gx-button--primary" disabled={busy}>
          {busy ? 'Saving…' : submitLabel}
        </button>
      </div>
    </form>
  )
}
