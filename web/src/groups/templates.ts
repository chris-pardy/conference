// Starting points for new cards. Each is plain card-kit JSON plus the
// config for the standard module: which reducer folds each action type
// into the card's shared state.

const D = 'app.eventside.block.defs#'
const b = (type: string, props: Record<string, unknown> = {}) => ({ $type: `${D}${type}`, ...props })
const bound = (block: Record<string, unknown>, prop: string, path: string, source = 'data') => ({
  ...block,
  bind: { ...((block.bind as object) ?? {}), [prop]: { source, path } },
})
const card = (blocks: unknown[]) => ({
  $type: 'app.eventside.block.card',
  blocks,
  sources: [{ name: 'data', ref: { $type: `${D}viewSource`, view: 'app.eventside.card.data' } }],
  createdAt: new Date().toISOString(),
})

export interface Template {
  name: string
  description: string
  ui: unknown
  config: Record<string, string>
}

const options = [
  ['Stroopwafels', 'stroopwafel'],
  ['Bitterballen', 'bitterballen'],
  ['Poffertjes', 'poffertjes'],
]

export const templates: Template[] = [
  {
    name: 'Announcement',
    description: 'Text only: no actions.',
    ui: card([
      b('header', { text: 'Lunch is served' }),
      b('richText', { text: 'Sandwiches and soup in the courtyard until 13:30. Vegan options on the left table.' }),
      b('badge', { text: 'Now', tone: 'success' }),
    ]),
    config: {},
  },
  {
    name: 'Poll',
    description: 'One vote each, live results bars.',
    ui: card([
      b('header', { text: 'Best Dutch snack?' }),
      b('buttonGroup', {
        id: 'snack',
        label: 'Pick one',
        action: 'snack',
        buttons: options.map(([label, value]) => ({ label, value })),
      }),
      ...options.map(([label, value]) => bound(b('progress', { label, max: 100 }), 'value', `/snack/percent/${value}`)),
      bound(b('stat', { label: 'votes' }), 'value', '/snack/total'),
    ]),
    config: { snack: 'vote' },
  },
  {
    name: 'Questions',
    description: 'Ask and upvote; most upvoted rises to the top.',
    ui: card([
      b('header', { text: 'Questions for the panel' }),
      b('textInput', { id: 'q', label: 'Your question', required: true, maxLength: 280 }),
      b('submit', { id: 'ask', label: 'Ask', action: 'ask', variant: 'primary' }),
      b('list', {
        items: { source: 'data', path: '/ask/items' },
        key: '/id',
        empty: 'No questions yet.',
        template: [
          b('section', {
            blocks: [
              bound(b('richText'), 'text', '/q', '$item'),
              b('columns', {
                columns: [
                  { blocks: [bound(b('person'), 'did', '/did', '$item')] },
                  { blocks: [bound(b('stat', { label: 'upvotes' }), 'value', '/votes', '$item')] },
                ],
              }),
              b('button', { id: 'up', label: 'Upvote', action: 'up', value: 'up' }),
            ],
          }),
        ],
      }),
    ]),
    config: { ask: 'append', up: 'upvote:ask' },
  },
  {
    name: 'Sign-up',
    description: 'Yes or no, with a live headcount.',
    ui: card([
      b('header', { text: 'Canal boat tour, Saturday 14:00' }),
      b('room', { name: 'Departs from Centraal, pier 5' }),
      b('buttonGroup', {
        id: 'going',
        label: 'Coming?',
        action: 'going',
        buttons: [
          { label: 'Count me in', value: 'yes' },
          { label: 'Not this time', value: 'no' },
        ],
      }),
      b('columns', {
        columns: [
          { blocks: [bound(b('stat', { label: 'going' }), 'value', '/going/counts/yes')] },
          { blocks: [bound(b('stat', { label: 'replied' }), 'value', '/going/total')] },
        ],
      }),
    ]),
    config: { going: 'vote' },
  },
  {
    name: 'Counter',
    description: 'Everyone adds to one running total.',
    ui: card([
      b('header', { text: 'Coffees drunk today' }),
      b('columns', {
        columns: [
          { blocks: [bound(b('stat', { label: 'everyone' }), 'value', '/coffee/total')] },
          { blocks: [bound(b('stat', { label: 'you' }), 'value', '/coffee/mine')] },
        ],
      }),
      b('button', { id: 'coffee', label: '+1 coffee', action: 'coffee', value: '1', variant: 'primary' }),
    ]),
    config: { coffee: 'count' },
  },
]
