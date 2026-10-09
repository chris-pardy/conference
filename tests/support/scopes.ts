// The scopes the appview asks for at sign-in by default (`LOGIN_SCOPES` in
// crates/server/src/config.rs). attendee-sign-in asked for `atproto` alone;
// conference-space adds writing to conference spaces. Tests of the default
// list check its shape here, so a feature that grows it changes one place.

/** The space type every conference space has. */
export const CONFERENCE_SPACE_TYPE = 'app.eventside.private'

/** A `space:` scope, parsed: its space type and its parameters. */
export function parseSpaceScope(scope: string): { type: string; params: URLSearchParams } | undefined {
  if (!scope.startsWith('space:')) return undefined
  const rest = scope.slice('space:'.length)
  const q = rest.indexOf('?')
  return {
    type: decodeURIComponent(q === -1 ? rest : rest.slice(0, q)),
    params: new URLSearchParams(q === -1 ? '' : rest.slice(q + 1)),
  }
}

/** The actions a `space:` scope grants (all of them when it names none). */
export function spaceActions(scope: string): string[] {
  const parsed = parseSpaceScope(scope)
  if (!parsed) return []
  const actions = parsed.params.getAll('action')
  return actions.length ? actions : ['read', 'create', 'update', 'delete']
}

/**
 * Whether a scope list is the appview's default: `atproto`, plus only scopes
 * for eventside's own spaces.
 */
export function isDefaultScopeList(scopes: readonly string[]): boolean {
  return (
    scopes.filter((s) => s === 'atproto').length === 1 &&
    scopes.every((s) => s === 'atproto' || parseSpaceScope(s)?.type.startsWith('app.eventside.'))
  )
}

/** The scope in a list that lets an attendee write to conference spaces, if any. */
export function conferenceWriteScope(scopes: readonly string[]): string | undefined {
  return scopes.find((s) => {
    const parsed = parseSpaceScope(s)
    return parsed?.type === CONFERENCE_SPACE_TYPE && spaceActions(s).includes('create')
  })
}

/**
 * The client ID the appview uses for a scope list, exactly: its metadata
 * document's URL, which carries the list unless it's `atproto` alone
 * (`client_id` in crates/server/src/oauth.rs, from attendee-sign-in), so an
 * authorization server that cached one list's metadata never sees another
 * list under the same ID.
 */
export function clientIdFor(serverUrl: string, scope: string): string {
  const base = `${serverUrl}/oauth-client-metadata.json`
  if (scope === 'atproto') return base
  const encoded = [...Buffer.from(scope, 'utf8')]
    .map((byte) =>
      /[A-Za-z0-9]/.test(String.fromCharCode(byte))
        ? String.fromCharCode(byte)
        : `%${byte.toString(16).toUpperCase().padStart(2, '0')}`,
    )
    .join('')
  return `${base}?scope=${encoded}`
}
