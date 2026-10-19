// Dates for events: shown in the event's own time zone, and entered as
// wall-clock times there.

const parts = (ms: number, timeZone: string) => {
  const out: Record<string, string> = {}
  for (const p of new Intl.DateTimeFormat('en-GB', {
    timeZone,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  }).formatToParts(ms)) {
    out[p.type] = p.value
  }
  return out
}

const safeZone = (timeZone: string) => {
  try {
    new Intl.DateTimeFormat('en-GB', { timeZone })
    return timeZone
  } catch {
    return 'UTC'
  }
}

export const localZone = () => Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'

const day = (ms: number, timeZone: string, year = false) =>
  new Intl.DateTimeFormat('en-GB', {
    timeZone,
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    ...(year ? { year: 'numeric' } : {}),
  }).format(ms)

const time = (ms: number, timeZone: string) =>
  new Intl.DateTimeFormat('en-GB', { timeZone, hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(ms)

const sameDay = (a: number, b: number, timeZone: string) => {
  const x = parts(a, timeZone)
  const y = parts(b, timeZone)
  return x.year === y.year && x.month === y.month && x.day === y.day
}

/** "Sat 1 May, 14:00–16:00", or "Thu 29 Apr – Sun 2 May 2027" for a multi-day event. */
export function when(startsAt: number, endsAt: number | null, zone: string): string {
  const timeZone = safeZone(zone)
  if (!endsAt || sameDay(startsAt, endsAt, timeZone)) {
    const start = `${day(startsAt, timeZone)}, ${time(startsAt, timeZone)}`
    return endsAt ? `${start}–${time(endsAt, timeZone)}` : start
  }
  const days = Math.round((endsAt - startsAt) / 86_400_000)
  if (days >= 1 && parts(startsAt, timeZone).hour === '00') {
    // An all-day, multi-day event: just the dates.
    return `${day(startsAt, timeZone)} – ${day(endsAt - 1, timeZone, true)}`
  }
  return `${day(startsAt, timeZone)}, ${time(startsAt, timeZone)} – ${day(endsAt, timeZone)}, ${time(endsAt, timeZone)}`
}

/** The short day of the month and month, for a date badge: ["1", "MAY"]. */
export function dateBadge(ms: number, zone: string): [string, string] {
  const timeZone = safeZone(zone)
  const p = parts(ms, timeZone)
  const month = new Intl.DateTimeFormat('en-GB', { timeZone, month: 'short' }).format(ms)
  return [String(Number(p.day)), month.toUpperCase()]
}

/** The UTC offset of a time zone at an instant, in ms. */
function offset(ms: number, timeZone: string): number {
  const p = parts(ms, timeZone)
  const asUtc = Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour, +p.minute)
  return asUtc - Math.floor(ms / 60_000) * 60_000
}

/** A `datetime-local` value ("2027-05-01T14:00") as wall-clock time in a zone, to epoch ms. */
export function fromLocalInput(value: string, zone: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(value)
  if (!m) return null
  const timeZone = safeZone(zone)
  const guess = Date.UTC(+m[1], +m[2] - 1, +m[3], +m[4], +m[5])
  const first = guess - offset(guess, timeZone)
  // Once more, in case the guess was across a DST change.
  return guess - offset(first, timeZone)
}

/** Epoch ms to a `datetime-local` value in a zone. */
export function toLocalInput(ms: number, zone: string): string {
  const p = parts(ms, safeZone(zone))
  return `${p.year}-${p.month}-${p.day}T${p.hour}:${p.minute}`
}

/** A friendly name for a time zone: "Amsterdam". */
export function zoneName(zone: string): string {
  return zone.split('/').pop()?.replace(/_/g, ' ') ?? zone
}
