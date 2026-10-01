import { Component, type ReactNode } from 'react'
import type { Binding, BlockData } from './bindings'
import { CardContext, type CardScope } from './context'
import { Placeholder } from './frame'
import { idsIn, useStableKeys } from './keys'
import { registry } from './registry'

export const DEFS = 'app.gather.block.defs'

/** A block's vocabulary type, e.g. "header", or null for anything this version doesn't know. */
export function blockType(block: unknown): string | null {
  const $type = (block as BlockData | null)?.$type
  if (typeof $type !== 'string' || !$type.startsWith(`${DEFS}#`)) return null
  const type = $type.slice(DEFS.length + 1)
  return Object.hasOwn(registry, type) ? type : null
}

/** One block, or nothing when its type is unknown (including the reserved custom and canvas). */
export function RenderBlock({ block }: { block: unknown }) {
  const type = blockType(block)
  if (!type) return null
  const Block = registry[type]
  return (
    <BlockBoundary type={type} block={block as BlockData}>
      <Block block={block as BlockData} />
    </BlockBoundary>
  )
}

/** A list of blocks, in order. */
export function Blocks({ blocks }: { blocks: unknown }) {
  const list = Array.isArray(blocks) ? blocks : []
  // A block's identity follows the ids in it (its own and those inside it),
  // else its type and its place among siblings of that type. A source update
  // never changes it, so blocks re-render without re-mounting; and when a
  // card is edited, what someone typed stays with the input it was typed into.
  const nth = list.map((block, i) => list.slice(0, i).filter((b) => b?.$type === block?.$type).length)
  const keys = useStableKeys(
    list,
    (block) => idsIn([block]),
    (block, i) => `${(block as BlockData | null)?.$type}@${nth[i]}`,
  )
  if (!Array.isArray(blocks)) return null
  return list.map((block, i) => <RenderBlock key={keys[i]} block={block} />)
}

/**
 * A block that throws while rendering shows as unavailable; the rest of the
 * card carries on. It tries again when the block changes, or when any of the
 * card's sources does (its data may have been the problem).
 */
class BlockBoundary extends Component<{ type: string; block: BlockData; children: ReactNode }, { failed: boolean }> {
  static contextType = CardContext
  declare context: CardScope | null
  state = { failed: false }
  private stopWatching?: () => void

  static getDerivedStateFromError() {
    return { failed: true }
  }

  componentDidMount() {
    this.watch()
  }

  componentDidUpdate(prev: { block: BlockData }) {
    if (this.state.failed && prev.block !== this.props.block) {
      this.setState({ failed: false })
      return
    }
    this.watch()
  }

  componentWillUnmount() {
    this.stopWatching?.()
  }

  /**
   * While failed, keeps the block's own sources watched (the block itself
   * is gone) and retries when one of them changes. Once it renders again,
   * lets go a tick later, after the block has picked the sources up.
   */
  private watch() {
    const store = this.context?.store
    if (!store) return
    if (!this.state.failed) {
      const stop = this.stopWatching
      this.stopWatching = undefined
      if (stop) setTimeout(stop)
      return
    }
    if (this.stopWatching) return
    const { block } = this.props
    const bindings = [...Object.values(block.bind ?? {}), block.items as Binding | undefined]
    const sources = bindings.flatMap((b) => {
      const named = b && typeof b.source === 'string' ? store.named(b.source) : null
      return named ? [named] : []
    })
    const keys = sources.map(([key]) => key)
    let seen: string | undefined
    this.stopWatching = store.subscribe(sources, () => {
      if (seen !== undefined && store.version(keys) !== seen) this.setState({ failed: false })
    })
    seen = store.version(keys)
  }

  render() {
    if (this.state.failed) return <Placeholder type={this.props.type} block={this.props.block} state="unavailable" />
    return this.props.children
  }
}
