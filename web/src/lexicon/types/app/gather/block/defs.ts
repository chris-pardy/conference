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

const is$typed = _is$typed,
  validate = _validate
const id = 'app.gather.block.defs'

/** A group of blocks with an optional title. */
export interface Section {
  $type?: 'app.gather.block.defs#section'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  title?: string
  /** The blocks in the section. */
  blocks: (
    | $Typed<Section>
    | $Typed<Header>
    | $Typed<Divider>
    | $Typed<Context>
    | $Typed<RichText>
    | $Typed<Image>
    | $Typed<Stack>
    | $Typed<Columns>
    | $Typed<Button>
    | $Typed<ButtonGroup>
    | $Typed<TextInput>
    | $Typed<Select>
    | $Typed<Submit>
    | $Typed<List>
    | $Typed<Progress>
    | $Typed<Stat>
    | $Typed<Badge>
    | $Typed<Person>
    | $Typed<SessionRef>
    | $Typed<Room>
    | $Typed<Time>
    | $Typed<Copyable>
    | $Typed<Qr>
    | $Typed<Custom>
    | $Typed<Canvas>
    | { $type: string }
  )[]
}

const hashSection = 'section'

export function isSection<V>(v: V) {
  return is$typed(v, id, hashSection)
}

export function validateSection<V>(v: V) {
  return validate<Section & V>(v, id, hashSection)
}

/** A heading. */
export interface Header {
  $type?: 'app.gather.block.defs#header'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  text?: string
  bind?: Bind
}

const hashHeader = 'header'

export function isHeader<V>(v: V) {
  return is$typed(v, id, hashHeader)
}

export function validateHeader<V>(v: V) {
  return validate<Header & V>(v, id, hashHeader)
}

/** A horizontal rule. */
export interface Divider {
  $type?: 'app.gather.block.defs#divider'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
}

const hashDivider = 'divider'

export function isDivider<V>(v: V) {
  return is$typed(v, id, hashDivider)
}

export function validateDivider<V>(v: V) {
  return validate<Divider & V>(v, id, hashDivider)
}

/** Small print. */
export interface Context {
  $type?: 'app.gather.block.defs#context'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  text?: string
  bind?: Bind
}

const hashContext = 'context'

export function isContext<V>(v: V) {
  return is$typed(v, id, hashContext)
}

export function validateContext<V>(v: V) {
  return validate<Context & V>(v, id, hashContext)
}

/** Text with facets: mentions, links, tags and emphasis. */
export interface RichText {
  $type?: 'app.gather.block.defs#richText'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  text?: string
  facets?: Facet[]
  bind?: Bind
}

const hashRichText = 'richText'

export function isRichText<V>(v: V) {
  return is$typed(v, id, hashRichText)
}

export function validateRichText<V>(v: V) {
  return validate<RichText & V>(v, id, hashRichText)
}

/** An image from a URL. */
export interface Image {
  $type?: 'app.gather.block.defs#image'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  url?: string
  /** Alt text, shown in place of the image when it fails to load. */
  alt: string
  aspectRatio?: AspectRatio
  bind?: Bind
}

const hashImage = 'image'

export function isImage<V>(v: V) {
  return is$typed(v, id, hashImage)
}

export function validateImage<V>(v: V) {
  return validate<Image & V>(v, id, hashImage)
}

/** Blocks laid out vertically. */
export interface Stack {
  $type?: 'app.gather.block.defs#stack'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  /** The stacked blocks. */
  blocks: (
    | $Typed<Section>
    | $Typed<Header>
    | $Typed<Divider>
    | $Typed<Context>
    | $Typed<RichText>
    | $Typed<Image>
    | $Typed<Stack>
    | $Typed<Columns>
    | $Typed<Button>
    | $Typed<ButtonGroup>
    | $Typed<TextInput>
    | $Typed<Select>
    | $Typed<Submit>
    | $Typed<List>
    | $Typed<Progress>
    | $Typed<Stat>
    | $Typed<Badge>
    | $Typed<Person>
    | $Typed<SessionRef>
    | $Typed<Room>
    | $Typed<Time>
    | $Typed<Copyable>
    | $Typed<Qr>
    | $Typed<Custom>
    | $Typed<Canvas>
    | { $type: string }
  )[]
}

const hashStack = 'stack'

export function isStack<V>(v: V) {
  return is$typed(v, id, hashStack)
}

export function validateStack<V>(v: V) {
  return validate<Stack & V>(v, id, hashStack)
}

/** Blocks laid out side by side. */
export interface Columns {
  $type?: 'app.gather.block.defs#columns'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  columns: Column[]
}

const hashColumns = 'columns'

export function isColumns<V>(v: V) {
  return is$typed(v, id, hashColumns)
}

export function validateColumns<V>(v: V) {
  return validate<Columns & V>(v, id, hashColumns)
}

/** A button. It sends an action intent, or opens a sheet. */
export interface Button {
  $type?: 'app.gather.block.defs#button'
  /** Names the block in action intents. Required on interactive blocks. */
  id: string
  label?: string
  /** The action id in the intent. Omitted for a button that only opens a sheet. */
  action?: string
  /** The value in the intent. */
  value?: string
  variant?: 'default' | 'primary' | 'danger' | (string & {})
  opens?: Sheet
  bind?: Bind
}

const hashButton = 'button'

export function isButton<V>(v: V) {
  return is$typed(v, id, hashButton)
}

export function validateButton<V>(v: V) {
  return validate<Button & V>(v, id, hashButton)
}

/** A row of buttons sharing one action; the intent carries the chosen value. */
export interface ButtonGroup {
  $type?: 'app.gather.block.defs#buttonGroup'
  /** Names the block in action intents. Required on interactive blocks. */
  id: string
  /** Accessible name of the group. */
  label?: string
  action: string
  buttons: Option[]
}

const hashButtonGroup = 'buttonGroup'

export function isButtonGroup<V>(v: V) {
  return is$typed(v, id, hashButtonGroup)
}

export function validateButtonGroup<V>(v: V) {
  return validate<ButtonGroup & V>(v, id, hashButtonGroup)
}

/** A text field. A submit block in the same form sends its value. */
export interface TextInput {
  $type?: 'app.gather.block.defs#textInput'
  /** Names the block in action intents. Required on interactive blocks. */
  id: string
  label: string
  placeholder?: string
  required?: boolean
  minLength?: number
  maxLength?: number
  multiline?: boolean
}

const hashTextInput = 'textInput'

export function isTextInput<V>(v: V) {
  return is$typed(v, id, hashTextInput)
}

export function validateTextInput<V>(v: V) {
  return validate<TextInput & V>(v, id, hashTextInput)
}

/** A single or multiple choice. A submit block in the same form sends its value. */
export interface Select {
  $type?: 'app.gather.block.defs#select'
  /** Names the block in action intents. Required on interactive blocks. */
  id: string
  label: string
  options: Option[]
  multiple?: boolean
  maxSelections?: number
  required?: boolean
}

const hashSelect = 'select'

export function isSelect<V>(v: V) {
  return is$typed(v, id, hashSelect)
}

export function validateSelect<V>(v: V) {
  return validate<Select & V>(v, id, hashSelect)
}

/** Sends one intent whose value maps each input's id to its value, for every input in its form: the nearest list item, sheet or card. */
export interface Submit {
  $type?: 'app.gather.block.defs#submit'
  /** Names the block in action intents. Required on interactive blocks. */
  id: string
  label: string
  action: string
  variant?: 'default' | 'primary' | 'danger' | (string & {})
}

const hashSubmit = 'submit'

export function isSubmit<V>(v: V) {
  return is$typed(v, id, hashSubmit)
}

export function validateSubmit<V>(v: V) {
  return validate<Submit & V>(v, id, hashSubmit)
}

/** Repeats a template for each element of a bound array. The element is the source $item. */
export interface List {
  $type?: 'app.gather.block.defs#list'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  items: Binding
  /** JSON pointer into each element giving its stable key. */
  key?: string
  /** The blocks repeated for each element. */
  template: (
    | $Typed<Section>
    | $Typed<Header>
    | $Typed<Divider>
    | $Typed<Context>
    | $Typed<RichText>
    | $Typed<Image>
    | $Typed<Stack>
    | $Typed<Columns>
    | $Typed<Button>
    | $Typed<ButtonGroup>
    | $Typed<TextInput>
    | $Typed<Select>
    | $Typed<Submit>
    | $Typed<List>
    | $Typed<Progress>
    | $Typed<Stat>
    | $Typed<Badge>
    | $Typed<Person>
    | $Typed<SessionRef>
    | $Typed<Room>
    | $Typed<Time>
    | $Typed<Copyable>
    | $Typed<Qr>
    | $Typed<Custom>
    | $Typed<Canvas>
    | { $type: string }
  )[]
  /** Shown when the array is empty. */
  empty?: string
}

const hashList = 'list'

export function isList<V>(v: V) {
  return is$typed(v, id, hashList)
}

export function validateList<V>(v: V) {
  return validate<List & V>(v, id, hashList)
}

/** A progress or result bar. */
export interface Progress {
  $type?: 'app.gather.block.defs#progress'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  label?: string
  value?: number
  max: number
  bind?: Bind
}

const hashProgress = 'progress'

export function isProgress<V>(v: V) {
  return is$typed(v, id, hashProgress)
}

export function validateProgress<V>(v: V) {
  return validate<Progress & V>(v, id, hashProgress)
}

/** A number with a label. */
export interface Stat {
  $type?: 'app.gather.block.defs#stat'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  value?: string
  label?: string
  bind?: Bind
}

const hashStat = 'stat'

export function isStat<V>(v: V) {
  return is$typed(v, id, hashStat)
}

export function validateStat<V>(v: V) {
  return validate<Stat & V>(v, id, hashStat)
}

/** A short tag in a semantic tone. */
export interface Badge {
  $type?: 'app.gather.block.defs#badge'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  text?: string
  tone?: 'neutral' | 'info' | 'warning' | 'success' | 'danger' | (string & {})
  bind?: Bind
}

const hashBadge = 'badge'

export function isBadge<V>(v: V) {
  return is$typed(v, id, hashBadge)
}

export function validateBadge<V>(v: V) {
  return validate<Badge & V>(v, id, hashBadge)
}

/** A DID shown as avatar and name, from its profile. */
export interface Person {
  $type?: 'app.gather.block.defs#person'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  did?: string
  bind?: Bind
}

const hashPerson = 'person'

export function isPerson<V>(v: V) {
  return is$typed(v, id, hashPerson)
}

export function validatePerson<V>(v: V) {
  return validate<Person & V>(v, id, hashPerson)
}

/** A session: title, times and room. */
export interface SessionRef {
  $type?: 'app.gather.block.defs#sessionRef'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  title?: string
  start?: string
  end?: string
  room?: string
  /** Where a schedule feature can deep-link. */
  uri?: string
  bind?: Bind
}

const hashSessionRef = 'sessionRef'

export function isSessionRef<V>(v: V) {
  return is$typed(v, id, hashSessionRef)
}

export function validateSessionRef<V>(v: V) {
  return validate<SessionRef & V>(v, id, hashSessionRef)
}

/** A room or location. */
export interface Room {
  $type?: 'app.gather.block.defs#room'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  name?: string
  detail?: string
  uri?: string
  bind?: Bind
}

const hashRoom = 'room'

export function isRoom<V>(v: V) {
  return is$typed(v, id, hashRoom)
}

export function validateRoom<V>(v: V) {
  return validate<Room & V>(v, id, hashRoom)
}

/** A time, or a countdown to it that says "now" until end and then "ended". */
export interface Time {
  $type?: 'app.gather.block.defs#time'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  label?: string
  at?: string
  end?: string
  mode?: 'time' | 'countdown' | (string & {})
  bind?: Bind
}

const hashTime = 'time'

export function isTime<V>(v: V) {
  return is$typed(v, id, hashTime)
}

export function validateTime<V>(v: V) {
  return validate<Time & V>(v, id, hashTime)
}

/** A value with a copy button, e.g. a wifi password. */
export interface Copyable {
  $type?: 'app.gather.block.defs#copyable'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  label?: string
  value?: string
  bind?: Bind
}

const hashCopyable = 'copyable'

export function isCopyable<V>(v: V) {
  return is$typed(v, id, hashCopyable)
}

export function validateCopyable<V>(v: V) {
  return validate<Copyable & V>(v, id, hashCopyable)
}

/** A QR code generated on the client. */
export interface Qr {
  $type?: 'app.gather.block.defs#qr'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  label?: string
  value?: string
  bind?: Bind
}

const hashQr = 'qr'

export function isQr<V>(v: V) {
  return is$typed(v, id, hashQr)
}

export function validateQr<V>(v: V) {
  return validate<Qr & V>(v, id, hashQr)
}

/** Reserved for block-sandbox: a wasm module that returns blocks. */
export interface Custom {
  $type?: 'app.gather.block.defs#custom'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  module: ModuleRef
  /** The card sources passed to the module. */
  sources?: string[]
}

const hashCustom = 'custom'

export function isCustom<V>(v: V) {
  return is$typed(v, id, hashCustom)
}

export function validateCustom<V>(v: V) {
  return validate<Custom & V>(v, id, hashCustom)
}

/** Reserved for block-sandbox: draw commands from a module, reporting taps. */
export interface Canvas {
  $type?: 'app.gather.block.defs#canvas'
  /** Names the block in action intents. Required on interactive blocks. */
  id?: string
  module: ModuleRef
  width?: number
  height?: number
  animate?: Animate[]
}

const hashCanvas = 'canvas'

export function isCanvas<V>(v: V) {
  return is$typed(v, id, hashCanvas)
}

export function validateCanvas<V>(v: V) {
  return validate<Canvas & V>(v, id, hashCanvas)
}

/** One column of a columns block. */
export interface Column {
  $type?: 'app.gather.block.defs#column'
  blocks: (
    | $Typed<Section>
    | $Typed<Header>
    | $Typed<Divider>
    | $Typed<Context>
    | $Typed<RichText>
    | $Typed<Image>
    | $Typed<Stack>
    | $Typed<Columns>
    | $Typed<Button>
    | $Typed<ButtonGroup>
    | $Typed<TextInput>
    | $Typed<Select>
    | $Typed<Submit>
    | $Typed<List>
    | $Typed<Progress>
    | $Typed<Stat>
    | $Typed<Badge>
    | $Typed<Person>
    | $Typed<SessionRef>
    | $Typed<Room>
    | $Typed<Time>
    | $Typed<Copyable>
    | $Typed<Qr>
    | $Typed<Custom>
    | $Typed<Canvas>
    | { $type: string }
  )[]
}

const hashColumn = 'column'

export function isColumn<V>(v: V) {
  return is$typed(v, id, hashColumn)
}

export function validateColumn<V>(v: V) {
  return validate<Column & V>(v, id, hashColumn)
}

/** Blocks shown in a bottom sheet. */
export interface Sheet {
  $type?: 'app.gather.block.defs#sheet'
  title?: string
  blocks: (
    | $Typed<Section>
    | $Typed<Header>
    | $Typed<Divider>
    | $Typed<Context>
    | $Typed<RichText>
    | $Typed<Image>
    | $Typed<Stack>
    | $Typed<Columns>
    | $Typed<Button>
    | $Typed<ButtonGroup>
    | $Typed<TextInput>
    | $Typed<Select>
    | $Typed<Submit>
    | $Typed<List>
    | $Typed<Progress>
    | $Typed<Stat>
    | $Typed<Badge>
    | $Typed<Person>
    | $Typed<SessionRef>
    | $Typed<Room>
    | $Typed<Time>
    | $Typed<Copyable>
    | $Typed<Qr>
    | $Typed<Custom>
    | $Typed<Canvas>
    | { $type: string }
  )[]
}

const hashSheet = 'sheet'

export function isSheet<V>(v: V) {
  return is$typed(v, id, hashSheet)
}

export function validateSheet<V>(v: V) {
  return validate<Sheet & V>(v, id, hashSheet)
}

/** A labelled value. */
export interface Option {
  $type?: 'app.gather.block.defs#option'
  label: string
  value: string
}

const hashOption = 'option'

export function isOption<V>(v: V) {
  return is$typed(v, id, hashOption)
}

export function validateOption<V>(v: V) {
  return validate<Option & V>(v, id, hashOption)
}

export interface AspectRatio {
  $type?: 'app.gather.block.defs#aspectRatio'
  width: number
  height: number
}

const hashAspectRatio = 'aspectRatio'

export function isAspectRatio<V>(v: V) {
  return is$typed(v, id, hashAspectRatio)
}

export function validateAspectRatio<V>(v: V) {
  return validate<AspectRatio & V>(v, id, hashAspectRatio)
}

/** A value read from a named source of the card, or $item inside a list. */
export interface Binding {
  $type?: 'app.gather.block.defs#binding'
  source: string
  /** A JSON pointer into the source value. Empty means the whole value. */
  path?: string
}

const hashBinding = 'binding'

export function isBinding<V>(v: V) {
  return is$typed(v, id, hashBinding)
}

export function validateBinding<V>(v: V) {
  return validate<Binding & V>(v, id, hashBinding)
}

/** Bindings for a block's properties, by property name. A bound property replaces the literal one. */
export interface Bind {
  $type?: 'app.gather.block.defs#bind'
  text?: Binding
  title?: Binding
  label?: Binding
  value?: Binding
  max?: Binding
  url?: Binding
  alt?: Binding
  did?: Binding
  at?: Binding
  start?: Binding
  end?: Binding
  room?: Binding
  name?: Binding
  detail?: Binding
}

const hashBind = 'bind'

export function isBind<V>(v: V) {
  return is$typed(v, id, hashBind)
}

export function validateBind<V>(v: V) {
  return validate<Bind & V>(v, id, hashBind)
}

/** Reserved for block-sandbox: a value computed by a wasm module from declared sources. */
export interface Computed {
  $type?: 'app.gather.block.defs#computed'
  module: ModuleRef
  export: string
  inputs?: string[]
}

const hashComputed = 'computed'

export function isComputed<V>(v: V) {
  return is$typed(v, id, hashComputed)
}

export function validateComputed<V>(v: V) {
  return validate<Computed & V>(v, id, hashComputed)
}

/** A named source that bindings refer to. */
export interface Source {
  $type?: 'app.gather.block.defs#source'
  name: string
  ref:
    | $Typed<RecordSource>
    | $Typed<CollectionSource>
    | $Typed<ProfileSource>
    | $Typed<ViewSource>
    | { $type: string }
}

const hashSource = 'source'

export function isSource<V>(v: V) {
  return is$typed(v, id, hashSource)
}

export function validateSource<V>(v: V) {
  return validate<Source & V>(v, id, hashSource)
}

/** One record in a space. */
export interface RecordSource {
  $type?: 'app.gather.block.defs#recordSource'
  record: SpaceRecordRef
}

const hashRecordSource = 'recordSource'

export function isRecordSource<V>(v: V) {
  return is$typed(v, id, hashRecordSource)
}

export function validateRecordSource<V>(v: V) {
  return validate<RecordSource & V>(v, id, hashRecordSource)
}

/** A collection in the card's space. */
export interface CollectionSource {
  $type?: 'app.gather.block.defs#collectionSource'
  collection: string
  filter?: { [_ in string]: unknown }
}

const hashCollectionSource = 'collectionSource'

export function isCollectionSource<V>(v: V) {
  return is$typed(v, id, hashCollectionSource)
}

export function validateCollectionSource<V>(v: V) {
  return validate<CollectionSource & V>(v, id, hashCollectionSource)
}

/** A DID's profile. */
export interface ProfileSource {
  $type?: 'app.gather.block.defs#profileSource'
  did: string
}

const hashProfileSource = 'profileSource'

export function isProfileSource<V>(v: V) {
  return is$typed(v, id, hashProfileSource)
}

export function validateProfileSource<V>(v: V) {
  return validate<ProfileSource & V>(v, id, hashProfileSource)
}

/** A quasi-record computed by the appview (see block-actions). */
export interface ViewSource {
  $type?: 'app.gather.block.defs#viewSource'
  view: string
  params?: { [_ in string]: unknown }
}

const hashViewSource = 'viewSource'

export function isViewSource<V>(v: V) {
  return is$typed(v, id, hashViewSource)
}

export function validateViewSource<V>(v: V) {
  return validate<ViewSource & V>(v, id, hashViewSource)
}

/** A record in a space. Space record URIs have more segments than at-uris, so strongRef does not fit. */
export interface SpaceRecordRef {
  $type?: 'app.gather.block.defs#spaceRecordRef'
  /** The space URI. */
  space: string
  /** The author. */
  did: string
  collection: string
  rkey: string
  cid?: string
}

const hashSpaceRecordRef = 'spaceRecordRef'

export function isSpaceRecordRef<V>(v: V) {
  return is$typed(v, id, hashSpaceRecordRef)
}

export function validateSpaceRecordRef<V>(v: V) {
  return validate<SpaceRecordRef & V>(v, id, hashSpaceRecordRef)
}

/** A wasm module, optionally with a script for the shared JS runtime. */
export interface ModuleRef {
  $type?: 'app.gather.block.defs#moduleRef'
  wasm: ModuleBlob
  script?: ModuleBlob
  exports?: string[]
}

const hashModuleRef = 'moduleRef'

export function isModuleRef<V>(v: V) {
  return is$typed(v, id, hashModuleRef)
}

export function validateModuleRef<V>(v: V) {
  return validate<ModuleRef & V>(v, id, hashModuleRef)
}

/** A blob by CID, with the DID of the repo that holds it. */
export interface ModuleBlob {
  $type?: 'app.gather.block.defs#moduleBlob'
  cid: string
  did: string
}

const hashModuleBlob = 'moduleBlob'

export function isModuleBlob<V>(v: V) {
  return is$typed(v, id, hashModuleBlob)
}

export function validateModuleBlob<V>(v: V) {
  return validate<ModuleBlob & V>(v, id, hashModuleBlob)
}

/** A declarative animation the host runs. */
export interface Animate {
  $type?: 'app.gather.block.defs#animate'
  property: string
  from: string
  to: string
  /** Milliseconds. */
  duration: number
  easing?: 'linear' | 'ease-in' | 'ease-out' | 'ease-in-out' | (string & {})
  /** 0 repeats forever. */
  repeat?: number
}

const hashAnimate = 'animate'

export function isAnimate<V>(v: V) {
  return is$typed(v, id, hashAnimate)
}

export function validateAnimate<V>(v: V) {
  return validate<Animate & V>(v, id, hashAnimate)
}

/** Annotates a byte range of the text, as in app.bsky.richtext.facet. */
export interface Facet {
  $type?: 'app.gather.block.defs#facet'
  index: ByteSlice
  features: (
    | $Typed<Mention>
    | $Typed<Link>
    | $Typed<Tag>
    | $Typed<Bold>
    | $Typed<Italic>
    | { $type: string }
  )[]
}

const hashFacet = 'facet'

export function isFacet<V>(v: V) {
  return is$typed(v, id, hashFacet)
}

export function validateFacet<V>(v: V) {
  return validate<Facet & V>(v, id, hashFacet)
}

/** A range of UTF-8 bytes: start inclusive, end exclusive. */
export interface ByteSlice {
  $type?: 'app.gather.block.defs#byteSlice'
  byteStart: number
  byteEnd: number
}

const hashByteSlice = 'byteSlice'

export function isByteSlice<V>(v: V) {
  return is$typed(v, id, hashByteSlice)
}

export function validateByteSlice<V>(v: V) {
  return validate<ByteSlice & V>(v, id, hashByteSlice)
}

export interface Mention {
  $type?: 'app.gather.block.defs#mention'
  did: string
}

const hashMention = 'mention'

export function isMention<V>(v: V) {
  return is$typed(v, id, hashMention)
}

export function validateMention<V>(v: V) {
  return validate<Mention & V>(v, id, hashMention)
}

export interface Link {
  $type?: 'app.gather.block.defs#link'
  uri: string
}

const hashLink = 'link'

export function isLink<V>(v: V) {
  return is$typed(v, id, hashLink)
}

export function validateLink<V>(v: V) {
  return validate<Link & V>(v, id, hashLink)
}

export interface Tag {
  $type?: 'app.gather.block.defs#tag'
  tag: string
}

const hashTag = 'tag'

export function isTag<V>(v: V) {
  return is$typed(v, id, hashTag)
}

export function validateTag<V>(v: V) {
  return validate<Tag & V>(v, id, hashTag)
}

export interface Bold {
  $type?: 'app.gather.block.defs#bold'
}

const hashBold = 'bold'

export function isBold<V>(v: V) {
  return is$typed(v, id, hashBold)
}

export function validateBold<V>(v: V) {
  return validate<Bold & V>(v, id, hashBold)
}

export interface Italic {
  $type?: 'app.gather.block.defs#italic'
}

const hashItalic = 'italic'

export function isItalic<V>(v: V) {
  return is$typed(v, id, hashItalic)
}

export function validateItalic<V>(v: V) {
  return validate<Italic & V>(v, id, hashItalic)
}
