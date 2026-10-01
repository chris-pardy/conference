// An explicit offset or Z after the time: "…T19:00:00Z", "…+02:00", "…-0530".
const HAS_OFFSET = /T.*(Z|[+-]\d\d(:?\d\d)?)$/i
const WALL_CLOCK = /^(\d{4})-(\d\d)-(\d\d)T(\d\d):(\d\d)(?::(\d\d)(?:[.,](\d+))?)?$/

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

/**
 * The instant a record's datetime means. With an offset it's exact. Without
 * one (the lexicon allows it) it's a wall-clock time in the card's zone, so
 * every viewer sees the same time; with no card zone, the viewer's.
 */
export function parseDatetime(s: string, timeZone?: string): Date | undefined {
  const m = WALL_CLOCK.exec(s)
  if (!HAS_OFFSET.test(s) && m && timeZone) {
    const [, y, mo, d, h, mi, sec = '0', frac = '0'] = m
    const ms = Math.round(Number(`0.${frac}`) * 1000)
    const wall = Date.UTC(Number(y), Number(mo) - 1, Number(d), Number(h), Number(mi), Number(sec), ms)
    try {
      // Twice, so a wall time near a daylight-saving change lands on the right side of it.
      const first = wall - zoneOffset(wall, timeZone)
      return new Date(wall - zoneOffset(first, timeZone))
    } catch {
      // An unknown zone: fall through to the viewer's.
    }
  }
  const date = new Date(s)
  return Number.isNaN(date.getTime()) ? undefined : date
}
