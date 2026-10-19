import { useMemo, useState } from 'react'
import { validateCard } from '../blocks/validate'
import { type CardView, cards } from './api'
import { LiveCard, type Viewer } from './LiveCard'
import { templates } from './templates'

interface ComposerProps {
  feedId: string
  viewer: Viewer
  onClose(): void
  onPosted(card: CardView): void
}

const pretty = (v: unknown) => JSON.stringify(v, null, 2)

/** Base64 of a file's bytes. */
async function base64(file: File): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer())
  let binary = ''
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  return btoa(binary)
}

/** Writes a card as card-kit JSON, previews it, and posts it to the feed. */
export function Composer({ feedId, viewer, onClose, onPosted }: ComposerProps) {
  const [chosen, setChosen] = useState(1)
  const [uiText, setUiText] = useState(pretty(templates[1].ui))
  const [reducersText, setReducersText] = useState(pretty(templates[1].reducers))
  const [module, setModule] = useState<File | null>(null)
  const [posting, setPosting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const choose = (i: number) => {
    setChosen(i)
    setUiText(pretty(templates[i].ui))
    setReducersText(pretty(templates[i].reducers))
  }

  const parsed = useMemo(() => {
    try {
      const ui = JSON.parse(uiText)
      const reducers = JSON.parse(reducersText || '{}')
      const check = validateCard(ui)
      if (!check.ok) return { error: `${check.error.path || 'card'}: ${check.error.message}` }
      return { ui, reducers }
    } catch (err) {
      return { error: (err as Error).message }
    }
  }, [uiText, reducersText])

  // The preview is a card with no data yet, as everyone sees it before anyone acts.
  const preview = useMemo<CardView | null>(
    () =>
      'ui' in parsed
        ? {
            id: 'preview',
            feedId,
            author: viewer.did,
            version: 0,
            createdAt: 0,
            ui: parsed.ui,
            reducers: parsed.reducers,
            data: { viewer },
            module: null,
          }
        : null,
    [parsed, feedId, viewer],
  )

  const post = async () => {
    if (!('ui' in parsed)) return
    setPosting(true)
    setError(null)
    try {
      const wasm = module ? await base64(module) : undefined
      onPosted(await cards.create(feedId, parsed.ui, parsed.reducers, wasm))
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setPosting(false)
    }
  }

  return (
    <section className="gx-composer" aria-label="New card">
      <div className="gx-composer__head">
        <h2>New card</h2>
        <button type="button" className="gx-button" onClick={onClose}>
          Cancel
        </button>
      </div>
      <div className="gx-chips" role="radiogroup" aria-label="Start from">
        {templates.map((t, i) => (
          // biome-ignore lint/a11y/useSemanticElements: chip buttons, styled as a radio group
          <button
            key={t.name}
            type="button"
            role="radio"
            aria-checked={i === chosen}
            className="gx-chip"
            title={t.description}
            onClick={() => choose(i)}
          >
            {t.name}
          </button>
        ))}
      </div>
      <div className="gx-composer__body">
        <div className="gx-composer__edit">
          <label className="gx-label" htmlFor="card-ui">
            Card kit UI
          </label>
          <textarea
            id="card-ui"
            className="gx-code"
            spellCheck={false}
            value={uiText}
            onChange={(e) => setUiText(e.target.value)}
          />
          <label className="gx-label" htmlFor="card-reducers">
            Reducers{' '}
            <span className="gx-muted">action id → vote · append · count · leaderboard · upvote:&lt;id&gt;</span>
          </label>
          <textarea
            id="card-reducers"
            className="gx-code gx-code--short"
            spellCheck={false}
            value={reducersText}
            onChange={(e) => setReducersText(e.target.value)}
          />
          <label className="gx-label" htmlFor="card-module">
            Wasm module <span className="gx-muted">optional: answers actions on each device</span>
          </label>
          <input
            id="card-module"
            type="file"
            accept=".wasm,application/wasm"
            onChange={(e) => setModule(e.target.files?.[0] ?? null)}
          />
        </div>
        <div className="gx-composer__preview">
          <p className="gx-label">Preview</p>
          {preview ? (
            <LiveCard card={preview} viewer={viewer} onUpdate={() => {}} preview />
          ) : (
            <p className="gx-error">{'error' in parsed ? parsed.error : ''}</p>
          )}
        </div>
      </div>
      {error && (
        <p className="gx-error" role="alert">
          {error}
        </p>
      )}
      <div className="gx-composer__foot">
        <button
          type="button"
          className="gx-button gx-button--primary"
          disabled={!preview || posting}
          onClick={() => void post()}
        >
          {posting ? 'Posting…' : 'Post to feed'}
        </button>
      </div>
    </section>
  )
}
