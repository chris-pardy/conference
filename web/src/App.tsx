import { useEffect, useState } from 'react'

type Health = { status: string; atproto: string }
type State = { kind: 'checking' } | { kind: 'ready'; health: Health } | { kind: 'failed' }

export default function App() {
  const [state, setState] = useState<State>({ kind: 'checking' })

  useEffect(() => {
    const controller = new AbortController()
    fetch('/health', { signal: controller.signal })
      .then((res) => (res.ok ? (res.json() as Promise<Health>) : Promise.reject(new Error(`HTTP ${res.status}`))))
      .then((health) => setState({ kind: 'ready', health }))
      .catch(() => {
        if (!controller.signal.aborted) setState({ kind: 'failed' })
      })
    return () => controller.abort()
  }, [])

  return (
    <main>
      <h1>Conference</h1>
      {state.kind === 'checking' && <p>Checking…</p>}
      {state.kind === 'failed' && <p>Backend: unavailable</p>}
      {state.kind === 'ready' && (
        <>
          <p>Backend: {state.health.status}</p>
          <p>atproto: {state.health.atproto}</p>
        </>
      )}
    </main>
  )
}
