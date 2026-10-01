/** How far `timeZone` is ahead of UTC at `instant`, in milliseconds. */
function zoneOffset(instant: number, timeZone: string): number {
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat('en-US', {
      timeZone,
      hourCycle: 'h23',
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    })
      .formatToParts(instant)
      .map((p) => [p.type, Number(p.value)]),
  )
  const asUtc = Date.UTC(parts.year, parts.month - 1, parts.day, parts.hour, parts.minute, parts.second)
  return asUtc - (instant - (instant % 1000))
}

// The forms the lexicon's lenient datetime check accepts (see
// crates/blocks/src/datetime.rs): extended or basic date, a `T`, then a
// clock with any separator or none, an optional fraction after any one
// character, and `Z`, an offset (`±hh:mm`, `±hhmm`), or nothing.
const DATE = /^(\d{4})\D?(\d\d)\D?(\d\d)$/
const CLOCK = /^(\d\d)\D?(\d\d)(?:\D?(\d\d))?(?:.(\d+))?$/
const OFFSET = /^(\d\d)\D?(\d\d)$/

/**
 * The instant a record's datetime means, for every form validation accepts.
 * With an offset it's exact. Without one (also allowed) it's a wall-clock
 * time in the card's zone, so every viewer sees the same time; with no card
 * zone, the viewer's. Undefined when it names no real date.
 */
export function parseDatetime(s: string, timeZone?: string): Date | undefined {
  // As the validator does: the date before the first `T`, the time up to the next one.
  const [date, time] = s.split('T')
  const d = DATE.exec(date ?? '')
  if (!d || time === undefined) return fallback(s)

  let clock = time
  let offsetMinutes: number | undefined
  if (time.endsWith('Z')) {
    clock = time.replace('Z', '')
    offsetMinutes = 0
  } else {
    const sign = /[+-]/.exec(time)?.[0]
    if (sign) {
      // Anything after a second sign is ignored, as the validator ignores it.
      const [before, after] = time.split(/[+-]/)
      const o = OFFSET.exec(after ?? '')
      if (!o) return fallback(s)
      clock = before
      offsetMinutes = (sign === '-' ? -1 : 1) * (Number(o[1]) * 60 + Number(o[2]))
    }
  }
  const c = CLOCK.exec(clock)
  if (!c) return fallback(s)

  const [year, month, day] = [Number(d[1]), Number(d[2]), Number(d[3])]
  const [hour, minute, second] = [Number(c[1]), Number(c[2]), Number(c[3] ?? 0)]
  const ms = Math.round(Number(`0.${c[4] ?? '0'}`) * 1000)
  if (month < 1 || month > 12 || day < 1 || day > 31) return fallback(s)
  // Date.UTC rolls 24:00 and second 60 over, as JavaScript's Date does.
  const wall = Date.UTC(year, month - 1, day, hour, minute, second, ms)

  if (offsetMinutes !== undefined) return valid(wall - offsetMinutes * 60_000)
  if (timeZone) {
    try {
      // Twice, so a wall time near a daylight-saving change lands on the right side of it.
      const first = wall - zoneOffset(wall, timeZone)
      return valid(wall - zoneOffset(first, timeZone))
    } catch {
      // An unknown zone: the viewer's.
    }
  }
  return valid(new Date(year, month - 1, day, hour, minute, second, ms).getTime())
}

function valid(ms: number): Date | undefined {
  return Number.isNaN(ms) ? undefined : new Date(ms)
}

function fallback(s: string): Date | undefined {
  return valid(new Date(s).getTime())
}
