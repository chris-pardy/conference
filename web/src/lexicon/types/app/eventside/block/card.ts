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
import type * as AppEventsideBlockDefs from './defs.js'

const is$typed = _is$typed,
  validate = _validate
const id = 'app.eventside.block.card'

export interface Main {
  $type: 'app.eventside.block.card'
  /** The blocks, in order. */
  blocks: (
    | $Typed<AppEventsideBlockDefs.Section>
    | $Typed<AppEventsideBlockDefs.Header>
    | $Typed<AppEventsideBlockDefs.Divider>
    | $Typed<AppEventsideBlockDefs.Context>
    | $Typed<AppEventsideBlockDefs.RichText>
    | $Typed<AppEventsideBlockDefs.Image>
    | $Typed<AppEventsideBlockDefs.Stack>
    | $Typed<AppEventsideBlockDefs.Columns>
    | $Typed<AppEventsideBlockDefs.Button>
    | $Typed<AppEventsideBlockDefs.ButtonGroup>
    | $Typed<AppEventsideBlockDefs.TextInput>
    | $Typed<AppEventsideBlockDefs.Select>
    | $Typed<AppEventsideBlockDefs.Submit>
    | $Typed<AppEventsideBlockDefs.List>
    | $Typed<AppEventsideBlockDefs.Progress>
    | $Typed<AppEventsideBlockDefs.Stat>
    | $Typed<AppEventsideBlockDefs.Badge>
    | $Typed<AppEventsideBlockDefs.Person>
    | $Typed<AppEventsideBlockDefs.SessionRef>
    | $Typed<AppEventsideBlockDefs.Room>
    | $Typed<AppEventsideBlockDefs.Time>
    | $Typed<AppEventsideBlockDefs.Copyable>
    | $Typed<AppEventsideBlockDefs.Qr>
    | $Typed<AppEventsideBlockDefs.Custom>
    | $Typed<AppEventsideBlockDefs.Canvas>
    | { $type: string }
  )[]
  sources?: AppEventsideBlockDefs.Source[]
  /** Reserved for block-actions. */
  middleware?: AppEventsideBlockDefs.ModuleRef[]
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
