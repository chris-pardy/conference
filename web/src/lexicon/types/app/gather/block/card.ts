/**
 * GENERATED CODE - DO NOT MODIFY
 */
import { type ValidationResult, BlobRef } from '@atproto/lexicon'
import { CID } from 'multiformats/cid'
import { validate as _validate } from '../../../../lexicons.js'
import {
  type $Typed,
  is$typed as _is$typed,
  type OmitKey,
} from '../../../../util.js'
import type * as AppGatherBlockDefs from './defs.js'

const is$typed = _is$typed,
  validate = _validate
const id = 'app.gather.block.card'

export interface Main {
  $type: 'app.gather.block.card'
  /** The blocks, in order. */
  blocks: (
    | $Typed<AppGatherBlockDefs.Section>
    | $Typed<AppGatherBlockDefs.Header>
    | $Typed<AppGatherBlockDefs.Divider>
    | $Typed<AppGatherBlockDefs.Context>
    | $Typed<AppGatherBlockDefs.RichText>
    | $Typed<AppGatherBlockDefs.Image>
    | $Typed<AppGatherBlockDefs.Stack>
    | $Typed<AppGatherBlockDefs.Columns>
    | $Typed<AppGatherBlockDefs.Button>
    | $Typed<AppGatherBlockDefs.ButtonGroup>
    | $Typed<AppGatherBlockDefs.TextInput>
    | $Typed<AppGatherBlockDefs.Select>
    | $Typed<AppGatherBlockDefs.Submit>
    | $Typed<AppGatherBlockDefs.List>
    | $Typed<AppGatherBlockDefs.Progress>
    | $Typed<AppGatherBlockDefs.Stat>
    | $Typed<AppGatherBlockDefs.Badge>
    | $Typed<AppGatherBlockDefs.Person>
    | $Typed<AppGatherBlockDefs.SessionRef>
    | $Typed<AppGatherBlockDefs.Room>
    | $Typed<AppGatherBlockDefs.Time>
    | $Typed<AppGatherBlockDefs.Copyable>
    | $Typed<AppGatherBlockDefs.Qr>
    | $Typed<AppGatherBlockDefs.Custom>
    | $Typed<AppGatherBlockDefs.Canvas>
    | { $type: string }
  )[]
  sources?: AppGatherBlockDefs.Source[]
  /** Reserved for block-actions. */
  middleware?: AppGatherBlockDefs.ModuleRef[]
  /** IANA zone for the card's times, e.g. Europe/Amsterdam. Defaults to the viewer's. */
  timeZone?: string
  /** Plain text for notifications and search. */
  fallbackText?: string
  createdAt: string
  [k: string]: unknown
}

const hashMain = 'main'

export function isMain<V>(v: V) {
  return is$typed(v, id, hashMain)
}

export function validateMain<V>(v: V) {
  return validate<Main & V>(v, id, hashMain, true)
}

export {
  type Main as Record,
  isMain as isRecord,
  validateMain as validateRecord,
}
