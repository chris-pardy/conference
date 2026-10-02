import { render } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { vi } from 'vitest'
import { ActionContext, type ActionIntent } from './ActionContext'
import { BlockCard, type Surface } from './BlockCard'
import { FixtureResolver, SourceResolverContext } from './SourceResolver'

// Cards for the tests, set at AtmosphereConf 2027 in Amsterdam.

export const DEFS = 'app.eventside.block.defs'
export const CARD_REF = { uri: 'at://did:plc:organizer/app.eventside.block.card/3lwelkom2027' }

/** A block of the given vocabulary type, e.g. block('header', { text: 'Hoi' }). */
export function block(type: string, props: Record<string, unknown> = {}) {
  return { $type: `${DEFS}#${type}`, ...props }
}

export function binding(source: string, path = '') {
  return { source, path }
}

/** A named record source. The fixture resolver answers by name, so the reference is only illustrative. */
export function recordSource(name: string) {
  return {
    name,
    ref: {
      $type: `${DEFS}#recordSource`,
      record: {
        space: 'ats://did:plc:organizer/app.eventside.conference/amsterdam-2027',
        did: 'did:plc:organizer',
        collection: 'app.eventside.demo.item',
        rkey: name,
      },
    },
  }
}

export function collectionSource(name: string) {
  return { name, ref: { $type: `${DEFS}#collectionSource`, collection: 'app.eventside.demo.question' } }
}

type CardProp = ComponentProps<typeof BlockCard>['card']

/** An app.eventside.block.card record. */
export function card(blocks: unknown[], extra: Record<string, unknown> = {}): CardProp {
  return {
    $type: 'app.eventside.block.card',
    blocks,
    timeZone: 'Europe/Amsterdam',
    createdAt: '2027-04-30T07:00:00.000Z',
    ...extra,
  } as unknown as CardProp
}

/** Renders a card the way a host does: a resolver and an action handler in context. */
export function renderCard(
  record: CardProp,
  opts: { surface?: Surface; resolver?: FixtureResolver; onAction?: (intent: ActionIntent) => void } = {},
) {
  const resolver = opts.resolver ?? new FixtureResolver({})
  const onAction = vi.fn(opts.onAction ?? (() => {}))
  const view = render(
    <SourceResolverContext value={resolver}>
      <ActionContext value={{ onAction }}>
        <BlockCard cardRef={CARD_REF} card={record} surface={opts.surface ?? 'feed'} />
      </ActionContext>
    </SourceResolverContext>,
  )
  return { ...view, resolver, onAction }
}

/** The rendered root of each block of a type, in document order. */
export function blocksOfType(container: HTMLElement, type: string): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>(`[data-block="${type}"]`)]
}

/** Byte offsets of the first occurrence of `part` in `text`, for rich text facets. */
export function byteSlice(text: string, part: string) {
  const at = text.indexOf(part)
  if (at < 0) throw new Error(`"${part}" is not in "${text}"`)
  const bytes = (s: string) => new TextEncoder().encode(s).length
  const byteStart = bytes(text.slice(0, at))
  return { byteStart, byteEnd: byteStart + bytes(part) }
}
