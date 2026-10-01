import qrcode from 'qrcode-generator'
import { useEffect, useMemo, useRef, useState } from 'react'
import { type BlockData, settle, text, useBound, useSourceStates } from '../bindings'
import { useCard } from '../context'
import { blockAttrs, Placeholder } from '../frame'
import { safeHref } from '../richtext'

type Props = { block: BlockData }

const loadingOr = (state: 'loading' | 'unavailable' | 'ready') => (state === 'loading' ? 'loading' : 'unavailable')

interface Profile {
  handle?: unknown
  displayName?: unknown
  avatar?: unknown
}

export function Person({ block }: Props) {
  const { store } = useCard()
  const bound = useBound(block, ['did'])
  const did = text(bound.did)
  const sources = did?.startsWith('did:') ? [store.profile(did)] : []
  const [profile] = useSourceStates(sources)
  const state = settle(bound)
  if (state !== 'ready') return <Placeholder type="person" block={block} state={loadingOr(state)} />
  if (!profile || profile.state === 'unavailable')
    return <Placeholder type="person" block={block} state="unavailable" />
  if (profile.state === 'loading') return <Placeholder type="person" block={block} state="loading" />

  const p = (profile.value ?? {}) as Profile
  const handle = typeof p.handle === 'string' && p.handle ? p.handle : undefined
  const name = typeof p.displayName === 'string' && p.displayName.trim() ? p.displayName : undefined
  if (!name && !handle) return <Placeholder type="person" block={block} state="unavailable" />
  const avatar = safeHref(p.avatar)
  return (
    <div {...blockAttrs('person', block)} className="g-block g-person">
      {avatar ? (
        <img className="g-person__avatar" src={avatar} alt="" />
      ) : (
        <span className="g-person__avatar g-person__avatar--none" aria-hidden="true">
          {(name ?? handle ?? '?').slice(0, 1).toUpperCase()}
        </span>
      )}
      <span className="g-person__names">
        <span className="g-person__name">{name ?? `@${handle}`}</span>
        {name && handle && (
          <>
            {' '}
            <span className="g-person__handle">@{handle}</span>
          </>
        )}
      </span>
    </div>
  )
}

/** A clock time on the 24-hour clock, in the card's zone. */
function clock(date: Date, timeZone?: string): string {
  try {
    return new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit', hourCycle: 'h23', timeZone }).format(
      date,
    )
  } catch {
    // An unknown zone falls back to the viewer's.
    return new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(date)
  }
}

function day(date: Date, timeZone?: string): string {
  const format = (zone?: string) =>
    new Intl.DateTimeFormat('en-GB', { weekday: 'short', day: 'numeric', month: 'short', timeZone: zone }).format(date)
  try {
    return format(timeZone)
  } catch {
    return format()
  }
}

function dateOf(state: Parameters<typeof text>[0]): Date | undefined {
  const s = text(state)
  if (!s) return undefined
  const date = new Date(s)
  return Number.isNaN(date.getTime()) ? undefined : date
}

export function SessionRef({ block }: Props) {
  const { timeZone } = useCard()
  const bound = useBound(block, ['title', 'start', 'end', 'room'])
  const state = settle(bound)
  const title = text(bound.title)
  if (state !== 'ready' || !title) return <Placeholder type="sessionRef" block={block} state={loadingOr(state)} />
  const start = dateOf(bound.start)
  const end = dateOf(bound.end)
  const room = text(bound.room)
  const href = safeHref(block.uri)
  return (
    <div {...blockAttrs('sessionRef', block)} className="g-block g-session">
      {start && (
        <span className="g-session__time">
          <time dateTime={start.toISOString()}>{clock(start, timeZone)}</time>
          {end && (
            <>
              {'–'}
              <time dateTime={end.toISOString()}>{clock(end, timeZone)}</time>
            </>
          )}
          <span className="g-session__day"> · {day(start, timeZone)}</span>
        </span>
      )}
      <span className="g-session__title"> {href ? <a href={href}>{title}</a> : title} </span>
      {room && <span className="g-session__room">{room}</span>}
    </div>
  )
}

export function Room({ block }: Props) {
  const bound = useBound(block, ['name', 'detail'])
  const state = settle(bound)
  const name = text(bound.name)
  if (state !== 'ready' || !name) return <Placeholder type="room" block={block} state={loadingOr(state)} />
  const detail = text(bound.detail)
  const href = safeHref(block.uri)
  return (
    <div {...blockAttrs('room', block)} className="g-block g-room">
      <span className="g-room__name">{href ? <a href={href}>{name}</a> : name} </span>
      {detail && <span className="g-room__detail">{detail}</span>}
    </div>
  )
}

/** "in 3 d", "in 1 h 5 min", "in 2 min", "in 40 s". */
function remaining(ms: number): string {
  const s = Math.ceil(ms / 1000)
  if (s < 60) return `in ${s} s`
  const minutes = Math.ceil(s / 60)
  if (minutes < 60) return `in ${minutes} min`
  const h = Math.floor(minutes / 60)
  if (h >= 48) return `in ${Math.floor(h / 24)} d`
  const m = minutes % 60
  return m ? `in ${h} h ${m} min` : `in ${h} h`
}

/**
 * How long until a countdown's text next changes, or null once it never
 * will: every minute while minutes show, every second in the last minute,
 * then at the end, then never.
 */
function nextChange(now: number, at: number | undefined, end: number | undefined): number | null {
  if (at === undefined) return null
  const left = at - now
  if (left > 60_000) return left % 60_000 || 60_000
  if (left > 0) return left % 1000 || 1000
  if (end !== undefined && now < end) return end - now
  return null
}

/**
 * The current time, updated whenever `next` says the display will change.
 * The timer re-arms itself rather than waiting for a render, so ticks don't
 * drift. `key` restarts it when what it counts towards changes.
 */
function useNow(next: (now: number) => number | null, key: string): number {
  const [now, setNow] = useState(() => Date.now())
  const latest = useRef(next)
  latest.current = next
  // biome-ignore lint/correctness/useExhaustiveDependencies: `key` stands for what `next` depends on
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined
    const schedule = (from: number) => {
      const delay = latest.current(from)
      if (delay === null) return
      timer = setTimeout(() => {
        const t = Date.now()
        setNow(t)
        schedule(t)
      }, delay)
    }
    const t = Date.now()
    setNow(t)
    schedule(t)
    return () => clearTimeout(timer)
  }, [key])
  return now
}

export function Time({ block }: Props) {
  const { timeZone } = useCard()
  const bound = useBound(block, ['at', 'end', 'label'])
  const countdown = block.mode === 'countdown'
  const atMs = dateOf(bound.at)?.getTime()
  const endMs = dateOf(bound.end)?.getTime()
  const now = useNow((t) => (countdown ? nextChange(t, atMs, endMs) : null), `${countdown}:${atMs}:${endMs}`)
  const state = settle(bound)
  const at = dateOf(bound.at)
  if (state !== 'ready' || !at) return <Placeholder type="time" block={block} state={loadingOr(state)} />
  const end = dateOf(bound.end)
  const label = text(bound.label)

  let status: string | undefined
  if (countdown) {
    if (now < at.getTime()) status = remaining(at.getTime() - now)
    else if (end && now >= end.getTime()) status = 'ended'
    else status = 'now'
  }
  return (
    <div {...blockAttrs('time', block)} className="g-block g-time">
      {label && <span className="g-time__label">{label} </span>}
      <span className="g-time__row">
        <time className="g-time__at" dateTime={at.toISOString()}>
          {clock(at, timeZone)}
        </time>
        {status && (
          <span
            className={`g-time__status g-time__status--${status === 'now' || status === 'ended' ? status : 'soon'}`}
          >
            {' · '}
            {status}
          </span>
        )}
      </span>
    </div>
  )
}

export function Copyable({ block }: Props) {
  const bound = useBound(block, ['label', 'value'])
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!copied) return
    const timer = setTimeout(() => setCopied(false), 2000)
    return () => clearTimeout(timer)
  }, [copied])
  const state = settle(bound)
  const value = text(bound.value)
  if (state !== 'ready' || value === undefined) {
    return <Placeholder type="copyable" block={block} state={loadingOr(state)} />
  }
  const label = text(bound.label)
  return (
    <div {...blockAttrs('copyable', block)} className="g-block g-copyable">
      {label && <span className="g-label">{label}</span>}
      <span className="g-copyable__row">
        <code className="g-copyable__value">{value}</code>
        <button
          type="button"
          className="g-button g-button--default"
          aria-label={label ? `Copy ${label}` : 'Copy'}
          onClick={() => {
            navigator.clipboard?.writeText(value).then(
              () => setCopied(true),
              () => {},
            )
          }}
        >
          Copy
        </button>
      </span>
      <span className="g-copyable__status" role="status">
        {copied ? 'Copied' : ''}
      </span>
    </div>
  )
}

/** A QR code's modules, as an SVG path of unit squares. */
function useQrPath(value: string) {
  return useMemo(() => {
    const qr = qrcode(0, 'M')
    qr.addData(value, 'Byte')
    qr.make()
    const size = qr.getModuleCount()
    let d = ''
    for (let row = 0; row < size; row++) {
      for (let col = 0; col < size; col++) if (qr.isDark(row, col)) d += `M${col} ${row}h1v1h-1z`
    }
    return { size, d }
  }, [value])
}

export function Qr({ block }: Props) {
  const { surface } = useCard()
  const bound = useBound(block, ['label', 'value'])
  const state = settle(bound)
  const value = text(bound.value)
  const qr = useQrPath(value ?? '')
  if (surface === 'compact') return null
  if (state !== 'ready' || !value) return <Placeholder type="qr" block={block} state={loadingOr(state)} />
  const label = text(bound.label)
  // A quiet zone of four modules. The code stays black on white whatever the
  // theme, since scanners need that contrast.
  const quiet = 4
  const box = qr.size + quiet * 2
  return (
    <figure {...blockAttrs('qr', block)} className="g-block g-qr">
      <svg
        className="g-qr__code"
        viewBox={`${-quiet} ${-quiet} ${box} ${box}`}
        role="img"
        aria-label={`QR code: ${label ?? value}`}
        shapeRendering="crispEdges"
      >
        <rect x={-quiet} y={-quiet} width={box} height={box} fill="#fff" />
        <path d={qr.d} fill="#000" />
      </svg>
      {label && <figcaption className="g-qr__label">{label}</figcaption>}
    </figure>
  )
}
