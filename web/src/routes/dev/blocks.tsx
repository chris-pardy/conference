import { useMemo, useState } from 'react'
import { ActionContext, type ActionIntent } from '../../blocks/ActionContext'
import { BlockCard, type Card, type Surface } from '../../blocks/BlockCard'
import { FixtureResolver, SourceResolverContext } from '../../blocks/SourceResolver'
import './blocks.css'

/**
 * A gallery card: a sample card record, the surface to show it on, and the
 * fixture sources and profiles it binds to. `{"$fixture": "loading"}` (or
 * "forbidden", "failing") stands for a source in that state.
 */
export interface GalleryEntry {
  title: string
  surface?: Surface
  card: Card
  sources?: Record<string, unknown>
  profiles?: Record<string, unknown>
}

const files = import.meta.glob<GalleryEntry>('../../blocks/gallery/*.card.json', { eager: true, import: 'default' })

const entries = Object.entries(files)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([path, entry]) => {
    const name = path.split('/').pop()?.replace('.card.json', '') ?? path
    return { name, uri: `at://did:plc:organizer/app.gather.block.card/${name}`, entry }
  })

type Logged = ActionIntent & { n: number }

function GalleryCard({ uri, entry }: { uri: string; entry: GalleryEntry }) {
  const resolver = useMemo(() => FixtureResolver.fromJson(entry.sources, entry.profiles), [entry])
  const cardRef = useMemo(() => ({ uri }), [uri])
  const surface = entry.surface ?? 'feed'
  const [shown, setShown] = useState(0)
  return (
    <section className="gallery__entry">
      <h2 className="gallery__caption">
        {entry.title} <span className="gallery__surface">{surface}</span>
      </h2>
      <SourceResolverContext value={resolver}>
        <BlockCard key={shown} cardRef={cardRef} card={entry.card} surface={surface} />
      </SourceResolverContext>
      {surface === 'ephemeral' && (
        <button type="button" className="gallery__again" onClick={() => setShown((n) => n + 1)}>
          Show it again
        </button>
      )}
    </section>
  )
}

/** The block gallery: every block, surface and state, rendered from fixtures, with the intents cards send. */
export default function BlockGallery() {
  const [log, setLog] = useState<Logged[]>([])
  const host = useMemo(
    () => ({ onAction: (intent: ActionIntent) => setLog((now) => [...now, { ...intent, n: now.length + 1 }]) }),
    [],
  )

  return (
    <main className="gallery">
      <header className="gallery__head">
        <h1>UI blocks</h1>
        <p>
          Every block in the vocabulary, on every surface, with sample data. Tap buttons and submit inputs to see the
          action intents a card hands its host.
        </p>
      </header>
      <ActionContext value={host}>
        <div className="gallery__cards">
          {entries.map(({ name, uri, entry }) => (
            <GalleryCard key={name} uri={uri} entry={entry} />
          ))}
        </div>
      </ActionContext>
      <section className="gallery__log" aria-labelledby="intents-title">
        <h2 id="intents-title">Action intents</h2>
        <div role="log" aria-labelledby="intents-title">
          {log.length === 0 ? (
            <p className="gallery__empty">Nothing sent yet.</p>
          ) : (
            <ol>
              {log.map((intent) => (
                <li key={intent.n}>
                  <dl>
                    <dt>card</dt>
                    <dd>{intent.card.uri}</dd>
                    <dt>block</dt>
                    <dd>{intent.blockId}</dd>
                    <dt>action</dt>
                    <dd>{intent.actionId}</dd>
                    <dt>value</dt>
                    <dd>
                      <code>{JSON.stringify(intent.value) ?? 'none'}</code>
                    </dd>
                  </dl>
                </li>
              ))}
            </ol>
          )}
        </div>
      </section>
    </main>
  )
}
