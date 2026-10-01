import { type ReactNode, useContext } from 'react'
import { ProfileLinkContext } from './context'
import { DEFS } from './render'

interface Facet {
  index?: { byteStart?: unknown; byteEnd?: unknown }
  features?: unknown
}

/** An image source: a web URL, or an inline raster or SVG image (which an <img> can't run scripts from). */
export function safeImageSrc(uri: unknown): string | undefined {
  if (typeof uri === 'string' && /^data:image\/(png|jpeg|gif|webp|svg\+xml)[;,]/.test(uri)) return uri
  // Images load by themselves, so never over plain http (mixed content on the PWA).
  const href = safeHref(uri)
  return href?.startsWith('https:') ? href : undefined
}

/** Only web links become anchors; anything else (javascript:, data:) stays text. */
export function safeHref(uri: unknown): string | undefined {
  if (typeof uri !== 'string') return undefined
  try {
    const url = new URL(uri)
    return url.protocol === 'https:' || url.protocol === 'http:' ? url.href : undefined
  } catch {
    return undefined
  }
}

// The DID syntax: only characters that are safe in a URL path.
const DID = /^did:[a-z]+:[a-zA-Z0-9._:%-]*[a-zA-Z0-9._-]$/

function applyFeature(
  feature: unknown,
  children: ReactNode,
  key: number,
  profileHref: (did: string) => string,
): ReactNode {
  const f = feature as { $type?: string; did?: unknown; uri?: unknown; tag?: unknown }
  switch (f?.$type) {
    case `${DEFS}#mention`:
      return typeof f.did === 'string' && DID.test(f.did) ? (
        <a key={key} href={profileHref(f.did)} target="_blank" rel="noopener noreferrer" className="g-mention">
          {children}
        </a>
      ) : (
        children
      )
    case `${DEFS}#link`: {
      const href = safeHref(f.uri)
      return href ? (
        <a key={key} href={href} target="_blank" rel="noopener noreferrer">
          {children}
        </a>
      ) : (
        children
      )
    }
    case `${DEFS}#tag`:
      return (
        <span key={key} className="g-tag">
          {children}
        </span>
      )
    case `${DEFS}#bold`:
      return <strong key={key}>{children}</strong>
    case `${DEFS}#italic`:
      return <em key={key}>{children}</em>
    default:
      return children
  }
}

/**
 * Text with facets over UTF-8 byte ranges, as in app.bsky posts. Facets that
 * overlap an earlier one, or fall outside the text, are ignored.
 */
export function RichText({ text, facets }: { text: string; facets?: unknown }): ReactNode {
  const profileHref = useContext(ProfileLinkContext)
  const bytes = new TextEncoder().encode(text)
  const decode = (start: number, end: number) => new TextDecoder().decode(bytes.slice(start, end))
  const ranges = (Array.isArray(facets) ? (facets as Facet[]) : [])
    .map((facet) => ({
      start: Number(facet.index?.byteStart),
      end: Number(facet.index?.byteEnd),
      features: Array.isArray(facet.features) ? facet.features : [],
    }))
    .filter((r) => Number.isInteger(r.start) && Number.isInteger(r.end) && r.start >= 0 && r.start < r.end)
    .filter((r) => r.end <= bytes.length)
    .sort((a, b) => a.start - b.start)

  const out: ReactNode[] = []
  let at = 0
  for (const range of ranges) {
    if (range.start < at) continue
    if (range.start > at) out.push(decode(at, range.start))
    let node: ReactNode = decode(range.start, range.end)
    // A link can't hold another link, so a range takes only its first mention or link.
    let linked = false
    range.features.forEach((feature, i) => {
      const $type = (feature as { $type?: unknown })?.$type
      const isLink = $type === `${DEFS}#mention` || $type === `${DEFS}#link`
      if (isLink && linked) return
      linked ||= isLink
      node = applyFeature(feature, node, i, profileHref)
    })
    out.push(<span key={range.start}>{node}</span>)
    at = range.end
  }
  if (at < bytes.length) out.push(decode(at, bytes.length))
  return out
}
