import { lazy, StrictMode, Suspense } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Route, Routes } from 'react-router'
import App from './App'
import Shell from './Shell'

// The block gallery is a page of its own, loaded only when visited.
const BlockGallery = lazy(() => import('./routes/dev/blocks'))
const SignIn = lazy(() => import('./routes/signin'))
const Conference = lazy(() => import('./routes/conference'))

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
          <Route
            path="/c/:actor/:rkey"
            element={
              <Suspense fallback={null}>
                <Conference />
              </Suspense>
            }
          />
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
