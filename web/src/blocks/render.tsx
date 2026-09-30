import { Component, type ReactNode } from 'react'
import type { BlockData } from './bindings'
import { Placeholder } from './frame'
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
  if (!Array.isArray(blocks)) return null
  // Blocks keep their position as identity, so a source update re-renders but never re-mounts them.
  // biome-ignore lint/suspicious/noArrayIndexKey: see above
  return blocks.map((block, i) => <RenderBlock key={i} block={block} />)
}

/** A block that throws while rendering shows as unavailable; the rest of the card carries on. */
class BlockBoundary extends Component<{ type: string; block: BlockData; children: ReactNode }, { failed: boolean }> {
  state = { failed: false }

  static getDerivedStateFromError() {
    return { failed: true }
  }

  render() {
    if (this.state.failed) return <Placeholder type={this.props.type} block={this.props.block} state="unavailable" />
    return this.props.children
  }
}
