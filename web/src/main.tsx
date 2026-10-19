import { lazy, StrictMode, Suspense } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Route, Routes } from 'react-router'
import Home from './groups/Home'
import Shell from './Shell'

// The block gallery is a page of its own, loaded only when visited.
const BlockGallery = lazy(() => import('./routes/dev/blocks'))
const SignIn = lazy(() => import('./routes/signin'))
const GroupPage = lazy(() => import('./groups/GroupPage'))

createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <BrowserRouter>
      <Routes>
        <Route element={<Shell />}>
          <Route path="/" element={<Home />} />
          <Route
            path="/g/:groupId/:feedId?"
            element={
              <Suspense fallback={null}>
                <GroupPage />
              </Suspense>
            }
          />
          <Route
            path="/signin"
            element={
              <Suspense fallback={null}>
                <SignIn />
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
