import type { CardRef } from './ActionContext'

/** Where a card is shown. */
export type Surface = 'feed' | 'compact' | 'sheet' | 'ephemeral'

/** An `app.gather.block.card` record. Replaced by the generated lexicon type. */
export type Card = { blocks: unknown[]; [field: string]: unknown }

export interface BlockCardProps {
  cardRef: CardRef
  card: Card
  surface: Surface
}

export function BlockCard(_props: BlockCardProps): never {
  throw new Error('not implemented')
}
