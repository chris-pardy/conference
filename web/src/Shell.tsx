import { useEffect, useRef, useState } from 'react'
import { Link, Outlet, useLocation } from 'react-router'
import { SessionProvider } from './auth/SessionProvider'
import { type SessionInfo, startSignIn, useSession } from './auth/session'
import './Shell.css'

/** The layout around every page: the session, and the header with the account control. */
export default function Shell() {
  return (
    <SessionProvider>
      <header className="shell__header">
        <Link to="/" className="shell__brand">
          Eventside
        </Link>
        <AccountControl />
      </header>
      <ExpiredBanner />
      <Outlet />
    </SessionProvider>
  )
}

/** Where "Sign in" should come back to: here, or wherever the sign-in page was headed. */
function useReturnTo(): string {
  const location = useLocation()
  if (location.pathname === '/signin') return new URLSearchParams(location.search).get('return_to') ?? '/'
  return location.pathname + location.search
}

function AccountControl() {
  const { session } = useSession()
  const returnTo = useReturnTo()
  if (session.kind === 'loading') return null
  if (session.kind === 'signedIn') return <AccountMenu user={session.user} />
  return (
    <Link className="shell__signin" to={`/signin?${new URLSearchParams({ return_to: returnTo })}`}>
      Sign in
    </Link>
  )
}

function AccountMenu({ user }: { user: SessionInfo }) {
  const { signOut } = useSession()
  const [open, setOpen] = useState(false)
  const menu = useRef<HTMLDivElement>(null)

  // Close on a click elsewhere or Escape.
  useEffect(() => {
    if (!open) return
    const onClick = (e: MouseEvent) => {
      if (!menu.current?.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('click', onClick)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('click', onClick)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  const initial = (user.displayName || user.handle).charAt(0).toUpperCase()
  return (
    <div className="shell__account" ref={menu}>
      <button
        type="button"
        className="shell__account-button"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        {user.avatar ? (
          <img className="shell__avatar" src={user.avatar} alt="" />
        ) : (
          <span className="shell__avatar shell__avatar--initial" aria-hidden="true">
            {initial}
          </span>
        )}
        <span className="shell__handle">{user.handle}</span>
      </button>
      {open && (
        <div className="shell__menu" role="menu">
          <button
            type="button"
            role="menuitem"
            className="shell__menu-item"
            onClick={() => {
              setOpen(false)
              void signOut()
            }}
          >
            Sign out
          </button>
        </div>
      )}
    </div>
  )
}

function ExpiredBanner() {
  const { session } = useSession()
  const location = useLocation()
  if (session.kind !== 'expired') return null
  const returnTo = location.pathname + location.search
  return (
    <div className="shell__expired" role="alert">
      <span>Your session has expired.</span>
      {session.handle ? (
        <button type="button" onClick={() => startSignIn(session.handle as string, returnTo)}>
          Sign in again
        </button>
      ) : (
        <Link to={`/signin?${new URLSearchParams({ return_to: returnTo })}`}>Sign in again</Link>
      )}
    </div>
  )
}
