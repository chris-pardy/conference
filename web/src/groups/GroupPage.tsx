import { type FormEvent, useCallback, useEffect, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { useSession } from '../auth/session'
import { type CardView, cards as cardsApi, type Feed, feeds as feedsApi, type Group, groups } from './api'
import { Composer } from './Composer'
import { LiveCard, type Viewer } from './LiveCard'
import './groups.css'

/** A group: its feeds as tabs, and the current feed's cards, live. */
export default function GroupPage() {
  const { session } = useSession()
  if (session.kind === 'loading') return <main className="gx-page" />
  if (session.kind !== 'signedIn') {
    return (
      <main className="gx-page">
        <p>
          <Link to={`/signin?${new URLSearchParams({ return_to: location.pathname })}`}>Sign in</Link> to see this
          group.
        </p>
      </main>
    )
  }
  return <SignedInGroup viewer={{ did: session.user.did, handle: session.user.handle }} />
}

function SignedInGroup({ viewer }: { viewer: Viewer }) {
  const { groupId = '', feedId } = useParams()
  const navigate = useNavigate()
  const [group, setGroup] = useState<Group | null>(null)
  const [feedList, setFeedList] = useState<Feed[]>([])
  const [missing, setMissing] = useState(false)
  const [cardList, setCardList] = useState<CardView[] | null>(null)
  const [composing, setComposing] = useState(false)
  const [live, setLive] = useState(false)
  const [newFeed, setNewFeed] = useState<string | null>(null)

  const current = feedId ?? feedList[0]?.id

  useEffect(() => {
    setGroup(null)
    groups
      .get(groupId)
      .then(({ group, feeds }) => {
        setGroup(group)
        setFeedList(feeds)
      })
      .catch(() => setMissing(true))
  }, [groupId])

  const loadCards = useCallback(() => {
    if (!current) return
    feedsApi
      .cards(current)
      .then(setCardList)
      .catch(() => setCardList([]))
  }, [current])

  useEffect(() => {
    setCardList(null)
    loadCards()
  }, [loadCards])

  const upsert = useCallback(
    (card: CardView) => {
      if (card.feedId !== current) return
      setCardList((list) => {
        if (!list) return list
        const at = list.findIndex((c) => c.id === card.id)
        if (at === -1) return [card, ...list]
        const next = list.slice()
        next[at] = card
        return next
      })
    },
    [current],
  )

  // Updates come down the group's stream as generic payloads.
  useEffect(() => {
    if (!groupId) return
    return groups.subscribe(groupId, (payload) => {
      switch (payload.type) {
        case 'hello':
          setLive(true)
          break
        case 'resync':
          loadCards()
          break
        case 'card':
          upsert(payload.card)
          break
        case 'cardRemoved':
          setCardList((list) => list?.filter((c) => c.id !== payload.cardId) ?? list)
          break
        case 'feed':
          setFeedList((list) => (list.some((f) => f.id === payload.feed.id) ? list : [...list, payload.feed]))
          break
      }
    })
  }, [groupId, upsert, loadCards])

  const addFeed = (e: FormEvent) => {
    e.preventDefault()
    const name = newFeed?.trim()
    if (!name) return
    feedsApi.create(groupId, name).then((feed) => {
      setFeedList((list) => (list.some((f) => f.id === feed.id) ? list : [...list, feed]))
      setNewFeed(null)
      navigate(`/g/${groupId}/${feed.id}`)
    })
  }

  if (missing) {
    return (
      <main className="gx-page">
        <p>This group doesn’t exist, or you’re not in it.</p>
        <Link to="/">Your groups</Link>
      </main>
    )
  }
  if (!group) return <main className="gx-page" />

  return (
    <main className="gx-page gx-group-page">
      <header className="gx-group-head">
        <Link to="/" className="gx-back">
          ← Groups
        </Link>
        <h1 className="gx-title">{group.name}</h1>
        {group.description && <p className="gx-muted">{group.description}</p>}
        <p className="gx-group-head__meta">
          <span>
            {group.memberCount} {group.memberCount === 1 ? 'person' : 'people'}
          </span>
          <span className="gx-invite" title="Share this code to invite people">
            Invite code <code>{group.invite}</code>
          </span>
          <span className={`gx-live ${live ? 'gx-live--on' : ''}`}>{live ? 'Live' : 'Connecting…'}</span>
        </p>
      </header>

      <nav className="gx-tabs" aria-label="Feeds">
        {feedList.map((f) => (
          <Link
            key={f.id}
            to={`/g/${groupId}/${f.id}`}
            className="gx-tab"
            aria-current={f.id === current ? 'page' : undefined}
          >
            {f.name}
          </Link>
        ))}
        {newFeed === null ? (
          <button type="button" className="gx-tab gx-tab--add" onClick={() => setNewFeed('')}>
            + Feed
          </button>
        ) : (
          <form className="gx-tab-form" onSubmit={addFeed}>
            <input
              className="gx-input"
              // biome-ignore lint/a11y/noAutofocus: opened by the person just now
              autoFocus
              aria-label="New feed name"
              placeholder="Feed name"
              maxLength={60}
              value={newFeed}
              onChange={(e) => setNewFeed(e.target.value)}
              onKeyDown={(e) => e.key === 'Escape' && setNewFeed(null)}
            />
            <button type="submit" className="gx-button gx-button--primary">
              Add
            </button>
          </form>
        )}
      </nav>

      {current && (
        <div className="gx-feed-actions">
          <button type="button" className="gx-button gx-button--primary" onClick={() => setComposing(true)}>
            New card
          </button>
        </div>
      )}

      {composing && current && (
        <Composer
          feedId={current}
          viewer={viewer}
          onClose={() => setComposing(false)}
          onPosted={(card) => {
            upsert(card)
            setComposing(false)
          }}
        />
      )}

      <section className="gx-feed" aria-label="Cards">
        {cardList === null && <p className="gx-muted">Loading…</p>}
        {cardList?.length === 0 && <p className="gx-muted">No cards in this feed yet.</p>}
        {cardList?.map((card) => (
          <LiveCard
            key={card.id}
            card={card}
            viewer={viewer}
            onUpdate={upsert}
            onRemove={() =>
              cardsApi.remove(card.id).then(() => setCardList((l) => l?.filter((c) => c.id !== card.id) ?? l))
            }
          />
        ))}
      </section>
    </main>
  )
}
