import type { ComponentType } from 'react'
import type { BlockData } from './bindings'
import { Copyable, Person, Qr, Room, SessionRef, Time } from './blocks/conference'
import { Badge, List, Progress, Stat } from './blocks/data'
import { Button, ButtonGroup, Select, Submit, TextInput } from './blocks/inputs'
import { Columns, Context, Divider, Header, Image, RichTextBlock, Section, Stack } from './blocks/layout'

/**
 * The component for each block type this version renders, by the name after
 * `app.gather.block.defs#`. Anything else, including the reserved `custom`
 * and `canvas` until block-sandbox, is skipped. Adding a block type means a
 * lexicon def, a component here, and a gallery card.
 */
export const registry: Record<string, ComponentType<{ block: BlockData }>> = {
  section: Section,
  header: Header,
  divider: Divider,
  context: Context,
  richText: RichTextBlock,
  image: Image,
  stack: Stack,
  columns: Columns,
  button: Button,
  buttonGroup: ButtonGroup,
  textInput: TextInput,
  select: Select,
  submit: Submit,
  list: List,
  progress: Progress,
  stat: Stat,
  badge: Badge,
  person: Person,
  sessionRef: SessionRef,
  room: Room,
  time: Time,
  copyable: Copyable,
  qr: Qr,
}
