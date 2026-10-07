import { lazy, type ReactNode, StrictMode, Suspense } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Route, Routes } from 'react-router'
import App from './App'
import Shell from './Shell'

// The block gallery is a page of its own, loaded only when visited.
const BlockGallery = lazy(() => import('./routes/dev/blocks'))
const SignIn = lazy(() => import('./routes/signin'))
const conference = () => import('./routes/conference')
const PublicConference = lazy(() => conference().then((m) => ({ default: m.PublicConferencePage })))
const SpaceConference = lazy(() => conference().then((m) => ({ default: m.SpaceConferencePage })))
const JoinByCode = lazy(() => conference().then((m) => ({ default: m.JoinByCodePage })))
const MyConferences = lazy(() => conference().then((m) => ({ default: m.MyConferencesPage })))

/** A page loaded only when visited. */
const later = (page: ReactNode) => <Suspense fallback={null}>{page}</Suspense>

createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <BrowserRouter>
      <Routes>
        <Route element={<Shell />}>
          <Route path="/" element={<App />} />
          <Route
            path="/signin"
            element={
              <Suspense fallback={null}>
                <SignIn />
              </Suspense>
            }
          />
          <Route path="/c/:actor/:rkey" element={later(<PublicConference />)} />
          <Route path="/space/:authority/:skey" element={later(<SpaceConference />)} />
          <Route path="/join/:code" element={later(<JoinByCode />)} />
          <Route path="/conferences" element={later(<MyConferences />)} />
          <Route
            path="/dev/blocks"
            element={
              <Suspense fallback={null}>
                <BlockGallery />
              </Suspense>
            }
          />
        </Route>
      </Routes>
    </BrowserRouter>
  </StrictMode>,
)
