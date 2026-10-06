/**
 * GENERATED CODE - DO NOT MODIFY
 */
import {
  type LexiconDoc,
  Lexicons,
  ValidationError,
  type ValidationResult,
} from '@atproto/lexicon'
import { type $Typed, is$typed, maybe$typed } from './util.js'

export const schemaDict = {
  AppEventsideAuthGetSession: {
    lexicon: 1,
    id: 'app.eventside.auth.getSession',
    defs: {
      main: {
        type: 'query',
        description:
          'Who is signed in to the appview, from the session cookie. Never calls the authorization server.',
        output: {
          encoding: 'application/json',
          schema: {
            type: 'object',
            required: ['did', 'handle', 'scopes', 'csrfToken'],
            properties: {
              did: {
                type: 'string',
                format: 'did',
              },
              handle: {
                type: 'string',
                format: 'handle',
              },
              displayName: {
                type: 'string',
                maxGraphemes: 64,
                maxLength: 640,
              },
              avatar: {
                type: 'string',
                format: 'uri',
              },
              scopes: {
                type: 'array',
                description: 'The OAuth scopes the session was granted.',
                items: {
                  type: 'string',
                },
              },
              csrfToken: {
                type: 'string',
                description:
                  'Sent as X-CSRF-Token on every state-changing request.',
              },
            },
          },
        },
        errors: [
          {
            name: 'AuthRequired',
            description: 'Nobody is signed in.',
          },
          {
            name: 'SessionExpired',
            description:
              "The session ended (idle, refused renewal, or new sign-in scopes). The body also carries the session's handle and DID, to sign the same person back in.",
          },
        ],
      },
    },
  },
  AppEventsideBlockCard: {
    lexicon: 1,
    id: 'app.eventside.block.card',
    defs: {
      main: {
        type: 'record',
        description:
          'A card: a tree of blocks bound to named sources. Written into a space.',
        key: 'tid',
        record: {
          type: 'object',
          required: ['blocks', 'createdAt'],
          properties: {
            blocks: {
              type: 'array',
              description: 'The blocks, in order.',
              items: {
                type: 'union',
                refs: [
                  'lex:app.eventside.block.defs#section',
                  'lex:app.eventside.block.defs#header',
                  'lex:app.eventside.block.defs#divider',
                  'lex:app.eventside.block.defs#context',
                  'lex:app.eventside.block.defs#richText',
                  'lex:app.eventside.block.defs#image',
                  'lex:app.eventside.block.defs#stack',
                  'lex:app.eventside.block.defs#columns',
                  'lex:app.eventside.block.defs#button',
                  'lex:app.eventside.block.defs#buttonGroup',
                  'lex:app.eventside.block.defs#textInput',
                  'lex:app.eventside.block.defs#select',
                  'lex:app.eventside.block.defs#submit',
                  'lex:app.eventside.block.defs#list',
                  'lex:app.eventside.block.defs#progress',
                  'lex:app.eventside.block.defs#stat',
                  'lex:app.eventside.block.defs#badge',
                  'lex:app.eventside.block.defs#person',
                  'lex:app.eventside.block.defs#sessionRef',
                  'lex:app.eventside.block.defs#room',
                  'lex:app.eventside.block.defs#time',
                  'lex:app.eventside.block.defs#copyable',
                  'lex:app.eventside.block.defs#qr',
                  'lex:app.eventside.block.defs#custom',
                  'lex:app.eventside.block.defs#canvas',
                ],
              },
              maxLength: 50,
              minLength: 1,
            },
            sources: {
              type: 'array',
              items: {
                type: 'ref',
                ref: 'lex:app.eventside.block.defs#source',
              },
              maxLength: 20,
            },
            middleware: {
              type: 'array',
              description: 'Reserved for block-actions.',
              items: {
                type: 'ref',
                ref: 'lex:app.eventside.block.defs#moduleRef',
              },
              maxLength: 10,
            },
            timeZone: {
              type: 'string',
              maxLength: 64,
              description:
                "IANA zone for the card's times, e.g. Europe/Amsterdam. Defaults to the viewer's.",
            },
            fallbackText: {
              type: 'string',
              maxLength: 3000,
              description: 'Plain text for notifications and search.',
            },
            createdAt: {
              type: 'string',
              format: 'datetime',
            },
          },
        },
      },
    },
  },
  AppEventsideBlockDefs: {
    lexicon: 1,
    id: 'app.eventside.block.defs',
    defs: {
      section: {
        type: 'object',
        description: 'A group of blocks with an optional title.',
        required: ['blocks'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          title: {
            type: 'string',
            maxLength: 300,
          },
          blocks: {
            type: 'array',
            description: 'The blocks in the section.',
            items: {
              type: 'union',
              refs: [
                'lex:app.eventside.block.defs#section',
                'lex:app.eventside.block.defs#header',
                'lex:app.eventside.block.defs#divider',
                'lex:app.eventside.block.defs#context',
                'lex:app.eventside.block.defs#richText',
                'lex:app.eventside.block.defs#image',
                'lex:app.eventside.block.defs#stack',
                'lex:app.eventside.block.defs#columns',
                'lex:app.eventside.block.defs#button',
                'lex:app.eventside.block.defs#buttonGroup',
                'lex:app.eventside.block.defs#textInput',
                'lex:app.eventside.block.defs#select',
                'lex:app.eventside.block.defs#submit',
                'lex:app.eventside.block.defs#list',
                'lex:app.eventside.block.defs#progress',
                'lex:app.eventside.block.defs#stat',
                'lex:app.eventside.block.defs#badge',
                'lex:app.eventside.block.defs#person',
                'lex:app.eventside.block.defs#sessionRef',
                'lex:app.eventside.block.defs#room',
                'lex:app.eventside.block.defs#time',
                'lex:app.eventside.block.defs#copyable',
                'lex:app.eventside.block.defs#qr',
                'lex:app.eventside.block.defs#custom',
                'lex:app.eventside.block.defs#canvas',
              ],
            },
            maxLength: 50,
          },
        },
      },
      header: {
        type: 'object',
        description: 'A heading.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          text: {
            type: 'string',
            maxLength: 300,
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      divider: {
        type: 'object',
        description: 'A horizontal rule.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
        },
      },
      context: {
        type: 'object',
        description: 'Small print.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          text: {
            type: 'string',
            maxLength: 1000,
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      richText: {
        type: 'object',
        description: 'Text with facets: mentions, links, tags and emphasis.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          text: {
            type: 'string',
            maxLength: 10000,
          },
          facets: {
            type: 'array',
            items: {
              type: 'ref',
              ref: 'lex:app.eventside.block.defs#facet',
            },
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      image: {
        type: 'object',
        description: 'An image from a URL.',
        required: ['alt'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          url: {
            type: 'string',
            format: 'uri',
          },
          alt: {
            type: 'string',
            maxLength: 2000,
            description:
              'Alt text, shown in place of the image when it fails to load.',
          },
          aspectRatio: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#aspectRatio',
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      stack: {
        type: 'object',
        description: 'Blocks laid out vertically.',
        required: ['blocks'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          blocks: {
            type: 'array',
            description: 'The stacked blocks.',
            items: {
              type: 'union',
              refs: [
                'lex:app.eventside.block.defs#section',
                'lex:app.eventside.block.defs#header',
                'lex:app.eventside.block.defs#divider',
                'lex:app.eventside.block.defs#context',
                'lex:app.eventside.block.defs#richText',
                'lex:app.eventside.block.defs#image',
                'lex:app.eventside.block.defs#stack',
                'lex:app.eventside.block.defs#columns',
                'lex:app.eventside.block.defs#button',
                'lex:app.eventside.block.defs#buttonGroup',
                'lex:app.eventside.block.defs#textInput',
                'lex:app.eventside.block.defs#select',
                'lex:app.eventside.block.defs#submit',
                'lex:app.eventside.block.defs#list',
                'lex:app.eventside.block.defs#progress',
                'lex:app.eventside.block.defs#stat',
                'lex:app.eventside.block.defs#badge',
                'lex:app.eventside.block.defs#person',
                'lex:app.eventside.block.defs#sessionRef',
                'lex:app.eventside.block.defs#room',
                'lex:app.eventside.block.defs#time',
                'lex:app.eventside.block.defs#copyable',
                'lex:app.eventside.block.defs#qr',
                'lex:app.eventside.block.defs#custom',
                'lex:app.eventside.block.defs#canvas',
              ],
            },
            maxLength: 50,
          },
        },
      },
      columns: {
        type: 'object',
        description: 'Blocks laid out side by side.',
        required: ['columns'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          columns: {
            type: 'array',
            items: {
              type: 'ref',
              ref: 'lex:app.eventside.block.defs#column',
            },
            minLength: 1,
            maxLength: 4,
          },
        },
      },
      button: {
        type: 'object',
        description: 'A button. It sends an action intent, or opens a sheet.',
        required: ['id'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 100,
          },
          action: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'The action id in the intent. Omitted for a button that only opens a sheet.',
          },
          value: {
            type: 'string',
            maxLength: 1000,
            description: 'The value in the intent.',
          },
          variant: {
            type: 'string',
            knownValues: ['default', 'primary', 'danger'],
          },
          opens: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#sheet',
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      buttonGroup: {
        type: 'object',
        description:
          'A row of buttons sharing one action; the intent carries the chosen value.',
        required: ['id', 'action', 'buttons'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 300,
            description: 'Accessible name of the group.',
          },
          action: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
          },
          buttons: {
            type: 'array',
            items: {
              type: 'ref',
              ref: 'lex:app.eventside.block.defs#option',
            },
            minLength: 1,
            maxLength: 10,
          },
        },
      },
      textInput: {
        type: 'object',
        description:
          'A text field. A submit block in the same form sends its value.',
        required: ['id', 'label'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 300,
          },
          placeholder: {
            type: 'string',
            maxLength: 300,
          },
          required: {
            type: 'boolean',
          },
          minLength: {
            type: 'integer',
            minimum: 0,
          },
          maxLength: {
            type: 'integer',
            minimum: 1,
          },
          multiline: {
            type: 'boolean',
          },
        },
      },
      select: {
        type: 'object',
        description:
          'A single or multiple choice. A submit block in the same form sends its value.',
        required: ['id', 'label', 'options'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 300,
          },
          options: {
            type: 'array',
            items: {
              type: 'ref',
              ref: 'lex:app.eventside.block.defs#option',
            },
            minLength: 1,
            maxLength: 50,
          },
          multiple: {
            type: 'boolean',
          },
          maxSelections: {
            type: 'integer',
            minimum: 1,
          },
          required: {
            type: 'boolean',
          },
        },
      },
      submit: {
        type: 'object',
        description:
          "Sends one intent whose value maps each input's id to its value, for every input in its form: the nearest list item, sheet or card.",
        required: ['id', 'label', 'action'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 100,
          },
          action: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
          },
          variant: {
            type: 'string',
            knownValues: ['default', 'primary', 'danger'],
          },
        },
      },
      list: {
        type: 'object',
        description:
          'Repeats a template for each element of a bound array. The element is the source $item.',
        required: ['items', 'template'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          items: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          key: {
            type: 'string',
            maxLength: 300,
            description:
              'JSON pointer into each element giving its stable key.',
          },
          template: {
            type: 'array',
            description: 'The blocks repeated for each element.',
            items: {
              type: 'union',
              refs: [
                'lex:app.eventside.block.defs#section',
                'lex:app.eventside.block.defs#header',
                'lex:app.eventside.block.defs#divider',
                'lex:app.eventside.block.defs#context',
                'lex:app.eventside.block.defs#richText',
                'lex:app.eventside.block.defs#image',
                'lex:app.eventside.block.defs#stack',
                'lex:app.eventside.block.defs#columns',
                'lex:app.eventside.block.defs#button',
                'lex:app.eventside.block.defs#buttonGroup',
                'lex:app.eventside.block.defs#textInput',
                'lex:app.eventside.block.defs#select',
                'lex:app.eventside.block.defs#submit',
                'lex:app.eventside.block.defs#list',
                'lex:app.eventside.block.defs#progress',
                'lex:app.eventside.block.defs#stat',
                'lex:app.eventside.block.defs#badge',
                'lex:app.eventside.block.defs#person',
                'lex:app.eventside.block.defs#sessionRef',
                'lex:app.eventside.block.defs#room',
                'lex:app.eventside.block.defs#time',
                'lex:app.eventside.block.defs#copyable',
                'lex:app.eventside.block.defs#qr',
                'lex:app.eventside.block.defs#custom',
                'lex:app.eventside.block.defs#canvas',
              ],
            },
            maxLength: 50,
          },
          empty: {
            type: 'string',
            maxLength: 1000,
            description: 'Shown when the array is empty.',
          },
        },
      },
      progress: {
        type: 'object',
        description: 'A progress or result bar.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 300,
          },
          value: {
            type: 'integer',
            minimum: 0,
          },
          max: {
            type: 'integer',
            minimum: 1,
            default: 100,
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      stat: {
        type: 'object',
        description: 'A number with a label.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          value: {
            type: 'string',
            maxLength: 100,
          },
          label: {
            type: 'string',
            maxLength: 300,
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      badge: {
        type: 'object',
        description: 'A short tag in a semantic tone.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          text: {
            type: 'string',
            maxLength: 100,
          },
          tone: {
            type: 'string',
            knownValues: ['neutral', 'info', 'warning', 'success', 'danger'],
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      person: {
        type: 'object',
        description: 'A DID shown as avatar and name, from its profile.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          did: {
            type: 'string',
            format: 'did',
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      sessionRef: {
        type: 'object',
        description: 'A session: title, times and room.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          title: {
            type: 'string',
            maxLength: 300,
          },
          start: {
            type: 'string',
            format: 'datetime',
          },
          end: {
            type: 'string',
            format: 'datetime',
          },
          room: {
            type: 'string',
            maxLength: 300,
          },
          uri: {
            type: 'string',
            format: 'uri',
            description: 'Where a schedule feature can deep-link.',
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      room: {
        type: 'object',
        description: 'A room or location.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          name: {
            type: 'string',
            maxLength: 300,
          },
          detail: {
            type: 'string',
            maxLength: 300,
          },
          uri: {
            type: 'string',
            format: 'uri',
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      time: {
        type: 'object',
        description:
          'A time, or a countdown to it that says "now" until end and then "ended".',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 300,
          },
          at: {
            type: 'string',
            format: 'datetime',
          },
          end: {
            type: 'string',
            format: 'datetime',
          },
          mode: {
            type: 'string',
            knownValues: ['time', 'countdown'],
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      copyable: {
        type: 'object',
        description: 'A value with a copy button, e.g. a wifi password.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 300,
          },
          value: {
            type: 'string',
            maxLength: 1000,
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      qr: {
        type: 'object',
        description: 'A QR code generated on the client.',
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          label: {
            type: 'string',
            maxLength: 300,
          },
          value: {
            type: 'string',
            maxLength: 2000,
          },
          bind: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#bind',
            description:
              'Properties bound to sources instead of given literally.',
          },
        },
      },
      custom: {
        type: 'object',
        description:
          'Reserved for block-sandbox: a wasm module that returns blocks.',
        required: ['module'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          module: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#moduleRef',
          },
          sources: {
            type: 'array',
            items: {
              type: 'string',
              maxLength: 64,
            },
            description: 'The card sources passed to the module.',
          },
        },
      },
      canvas: {
        type: 'object',
        description:
          'Reserved for block-sandbox: draw commands from a module, reporting taps.',
        required: ['module'],
        properties: {
          id: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
            description:
              'Names the block in action intents. Required on interactive blocks.',
          },
          module: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#moduleRef',
          },
          width: {
            type: 'integer',
            minimum: 1,
          },
          height: {
            type: 'integer',
            minimum: 1,
          },
          animate: {
            type: 'array',
            items: {
              type: 'ref',
              ref: 'lex:app.eventside.block.defs#animate',
            },
          },
        },
      },
      column: {
        type: 'object',
        description: 'One column of a columns block.',
        required: ['blocks'],
        properties: {
          blocks: {
            type: 'array',
            items: {
              type: 'union',
              refs: [
                'lex:app.eventside.block.defs#section',
                'lex:app.eventside.block.defs#header',
                'lex:app.eventside.block.defs#divider',
                'lex:app.eventside.block.defs#context',
                'lex:app.eventside.block.defs#richText',
                'lex:app.eventside.block.defs#image',
                'lex:app.eventside.block.defs#stack',
                'lex:app.eventside.block.defs#columns',
                'lex:app.eventside.block.defs#button',
                'lex:app.eventside.block.defs#buttonGroup',
                'lex:app.eventside.block.defs#textInput',
                'lex:app.eventside.block.defs#select',
                'lex:app.eventside.block.defs#submit',
                'lex:app.eventside.block.defs#list',
                'lex:app.eventside.block.defs#progress',
                'lex:app.eventside.block.defs#stat',
                'lex:app.eventside.block.defs#badge',
                'lex:app.eventside.block.defs#person',
                'lex:app.eventside.block.defs#sessionRef',
                'lex:app.eventside.block.defs#room',
                'lex:app.eventside.block.defs#time',
                'lex:app.eventside.block.defs#copyable',
                'lex:app.eventside.block.defs#qr',
                'lex:app.eventside.block.defs#custom',
                'lex:app.eventside.block.defs#canvas',
              ],
            },
            maxLength: 50,
          },
        },
      },
      sheet: {
        type: 'object',
        description: 'Blocks shown in a bottom sheet.',
        required: ['blocks'],
        properties: {
          title: {
            type: 'string',
            maxLength: 300,
          },
          blocks: {
            type: 'array',
            items: {
              type: 'union',
              refs: [
                'lex:app.eventside.block.defs#section',
                'lex:app.eventside.block.defs#header',
                'lex:app.eventside.block.defs#divider',
                'lex:app.eventside.block.defs#context',
                'lex:app.eventside.block.defs#richText',
                'lex:app.eventside.block.defs#image',
                'lex:app.eventside.block.defs#stack',
                'lex:app.eventside.block.defs#columns',
                'lex:app.eventside.block.defs#button',
                'lex:app.eventside.block.defs#buttonGroup',
                'lex:app.eventside.block.defs#textInput',
                'lex:app.eventside.block.defs#select',
                'lex:app.eventside.block.defs#submit',
                'lex:app.eventside.block.defs#list',
                'lex:app.eventside.block.defs#progress',
                'lex:app.eventside.block.defs#stat',
                'lex:app.eventside.block.defs#badge',
                'lex:app.eventside.block.defs#person',
                'lex:app.eventside.block.defs#sessionRef',
                'lex:app.eventside.block.defs#room',
                'lex:app.eventside.block.defs#time',
                'lex:app.eventside.block.defs#copyable',
                'lex:app.eventside.block.defs#qr',
                'lex:app.eventside.block.defs#custom',
                'lex:app.eventside.block.defs#canvas',
              ],
            },
            maxLength: 50,
          },
        },
      },
      option: {
        type: 'object',
        description: 'A labelled value.',
        required: ['label', 'value'],
        properties: {
          label: {
            type: 'string',
            maxLength: 300,
          },
          value: {
            type: 'string',
            maxLength: 1000,
          },
        },
      },
      aspectRatio: {
        type: 'object',
        required: ['width', 'height'],
        properties: {
          width: {
            type: 'integer',
            minimum: 1,
          },
          height: {
            type: 'integer',
            minimum: 1,
          },
        },
      },
      binding: {
        type: 'object',
        description:
          'A value read from a named source of the card, or $item inside a list.',
        required: ['source'],
        properties: {
          source: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
          },
          path: {
            type: 'string',
            maxLength: 1000,
            description:
              'A JSON pointer into the source value. Empty means the whole value.',
          },
        },
      },
      bind: {
        type: 'object',
        description:
          "Bindings for a block's properties, by property name. A bound property replaces the literal one.",
        properties: {
          text: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          title: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          label: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          value: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          max: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          url: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          alt: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          did: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          at: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          start: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          end: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          room: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          name: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
          detail: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#binding',
          },
        },
      },
      computed: {
        type: 'object',
        description:
          'Reserved for block-sandbox: a value computed by a wasm module from declared sources.',
        required: ['module', 'export'],
        properties: {
          module: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#moduleRef',
          },
          export: {
            type: 'string',
            maxLength: 64,
          },
          inputs: {
            type: 'array',
            items: {
              type: 'string',
              maxLength: 64,
            },
          },
        },
      },
      source: {
        type: 'object',
        description: 'A named source that bindings refer to.',
        required: ['name', 'ref'],
        properties: {
          name: {
            type: 'string',
            minLength: 1,
            maxLength: 64,
          },
          ref: {
            type: 'union',
            refs: [
              'lex:app.eventside.block.defs#recordSource',
              'lex:app.eventside.block.defs#collectionSource',
              'lex:app.eventside.block.defs#profileSource',
              'lex:app.eventside.block.defs#viewSource',
            ],
          },
        },
      },
      recordSource: {
        type: 'object',
        description: 'One record in a space.',
        required: ['record'],
        properties: {
          record: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#spaceRecordRef',
          },
        },
      },
      collectionSource: {
        type: 'object',
        description: "A collection in the card's space.",
        required: ['collection'],
        properties: {
          collection: {
            type: 'string',
            format: 'nsid',
          },
          filter: {
            type: 'unknown',
          },
        },
      },
      profileSource: {
        type: 'object',
        description: "A DID's profile.",
        required: ['did'],
        properties: {
          did: {
            type: 'string',
            format: 'did',
          },
        },
      },
      viewSource: {
        type: 'object',
        description:
          'A quasi-record computed by the appview (see block-actions).',
        required: ['view'],
        properties: {
          view: {
            type: 'string',
            format: 'nsid',
          },
          params: {
            type: 'unknown',
          },
        },
      },
      spaceRecordRef: {
        type: 'object',
        description:
          'A record in a space. Space record URIs have more segments than at-uris, so strongRef does not fit.',
        required: ['space', 'did', 'collection', 'rkey'],
        properties: {
          space: {
            type: 'string',
            maxLength: 1000,
            description: 'The space URI.',
          },
          did: {
            type: 'string',
            format: 'did',
            description: 'The author.',
          },
          collection: {
            type: 'string',
            format: 'nsid',
          },
          rkey: {
            type: 'string',
            format: 'record-key',
          },
          cid: {
            type: 'string',
            format: 'cid',
          },
        },
      },
      moduleRef: {
        type: 'object',
        description:
          'A wasm module, optionally with a script for the shared JS runtime.',
        required: ['wasm'],
        properties: {
          wasm: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#moduleBlob',
          },
          script: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#moduleBlob',
          },
          exports: {
            type: 'array',
            items: {
              type: 'string',
              maxLength: 64,
            },
          },
        },
      },
      moduleBlob: {
        type: 'object',
        description: 'A blob by CID, with the DID of the repo that holds it.',
        required: ['cid', 'did'],
        properties: {
          cid: {
            type: 'string',
            format: 'cid',
          },
          did: {
            type: 'string',
            format: 'did',
          },
        },
      },
      animate: {
        type: 'object',
        description: 'A declarative animation the host runs.',
        required: ['property', 'from', 'to', 'duration'],
        properties: {
          property: {
            type: 'string',
            maxLength: 64,
          },
          from: {
            type: 'string',
            maxLength: 100,
          },
          to: {
            type: 'string',
            maxLength: 100,
          },
          duration: {
            type: 'integer',
            minimum: 0,
            description: 'Milliseconds.',
          },
          easing: {
            type: 'string',
            knownValues: ['linear', 'ease-in', 'ease-out', 'ease-in-out'],
          },
          repeat: {
            type: 'integer',
            minimum: 0,
            description: '0 repeats forever.',
          },
        },
      },
      facet: {
        type: 'object',
        description:
          'Annotates a byte range of the text, as in app.bsky.richtext.facet.',
        required: ['index', 'features'],
        properties: {
          index: {
            type: 'ref',
            ref: 'lex:app.eventside.block.defs#byteSlice',
          },
          features: {
            type: 'array',
            items: {
              type: 'union',
              refs: [
                'lex:app.eventside.block.defs#mention',
                'lex:app.eventside.block.defs#link',
                'lex:app.eventside.block.defs#tag',
                'lex:app.eventside.block.defs#bold',
                'lex:app.eventside.block.defs#italic',
              ],
            },
            maxLength: 8,
          },
        },
      },
      byteSlice: {
        type: 'object',
        description: 'A range of UTF-8 bytes: start inclusive, end exclusive.',
        required: ['byteStart', 'byteEnd'],
        properties: {
          byteStart: {
            type: 'integer',
            minimum: 0,
          },
          byteEnd: {
            type: 'integer',
            minimum: 0,
          },
        },
      },
      mention: {
        type: 'object',
        required: ['did'],
        properties: {
          did: {
            type: 'string',
            format: 'did',
          },
        },
      },
      link: {
        type: 'object',
        required: ['uri'],
        properties: {
          uri: {
            type: 'string',
            format: 'uri',
          },
        },
      },
      tag: {
        type: 'object',
        required: ['tag'],
        properties: {
          tag: {
            type: 'string',
            maxLength: 640,
          },
        },
      },
      bold: {
        type: 'object',
        properties: {},
      },
      italic: {
        type: 'object',
        properties: {},
      },
    },
  },
} as const satisfies Record<string, LexiconDoc>
export const schemas = Object.values(schemaDict) satisfies LexiconDoc[]
export const lexicons: Lexicons = new Lexicons(schemas)

export function validate<T extends { $type: string }>(
  v: unknown,
  id: string,
  hash: string,
  requiredType: true,
): ValidationResult<T>
export function validate<T extends { $type?: string }>(
  v: unknown,
  id: string,
  hash: string,
  requiredType?: false,
): ValidationResult<T>
export function validate(
  v: unknown,
  id: string,
  hash: string,
  requiredType?: boolean,
): ValidationResult {
  return (requiredType ? is$typed : maybe$typed)(v, id, hash)
    ? lexicons.validate(`${id}#${hash}`, v)
    : {
        success: false,
        error: new ValidationError(
          `Must be an object with "${hash === 'main' ? id : `${id}#${hash}`}" $type property`,
        ),
      }
}

export const ids = {
  AppEventsideAuthGetSession: 'app.eventside.auth.getSession',
  AppEventsideBlockCard: 'app.eventside.block.card',
  AppEventsideBlockDefs: 'app.eventside.block.defs',
} as const
