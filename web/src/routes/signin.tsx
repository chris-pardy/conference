import { type FormEvent, useState } from 'react'
import { useSearchParams } from 'react-router'
import { startSignIn } from '../auth/session'
import './signin.css'

/** What the appview's error codes mean to the person signing in. */
const MESSAGES: Record<string, string> = {
  handle_not_found: 'We couldn’t find that handle. Check it and try again.',
  access_denied: 'Sign-in was cancelled.',
  account_mismatch: 'That account didn’t match the handle you entered. Sign in with the same account.',
  request_expired: 'That sign-in took too long. Please try again.',
  server_unavailable: 'Your account’s server couldn’t be reached. Please try again in a moment.',
  resolution_failed: 'We found that handle, but couldn’t look up its account. Please try again in a moment.',
  issuer_mismatch: 'Your account’s server didn’t check out, so we stopped. Please try again.',
  scope_missing: 'Sign-in needs every permission it asks for. Please try again and allow them all.',
  invalid_request: 'That sign-in link isn’t valid any more. Start again from this page.',
  authorization_failed: 'Your account’s server didn’t complete the sign-in. Please try again.',
  server_error: 'Something went wrong on our side. Please try again.',
}
const FALLBACK = 'Sign-in didn’t work. Please try again.'

/** The message for an error code from the URL. Only the codes above count, never inherited keys like `__proto__`. */
function messageFor(error: string): string {
  return Object.hasOwn(MESSAGES, error) ? MESSAGES[error] : FALLBACK
}

export default function SignIn() {
  const [params] = useSearchParams()
  const returnTo = params.get('return_to') ?? '/'
  const error = params.get('error')
  const [handle, setHandle] = useState('')

  const submit = (e: FormEvent) => {
    e.preventDefault()
    if (handle.trim()) startSignIn(handle.trim(), returnTo)
  }

  return (
    <main className="signin">
      <h1>Sign in</h1>
      <p className="signin__lede">Use your atproto account, the one you use for Bluesky and other apps.</p>
      {error && (
        <p className="signin__error" role="alert">
          {messageFor(error)}
        </p>
      )}
      <form className="signin__form" onSubmit={submit}>
        <label htmlFor="signin-handle">Handle</label>
        <input
          id="signin-handle"
          name="handle"
          type="text"
          autoComplete="username"
          autoCapitalize="none"
          spellCheck={false}
          placeholder="you.bsky.social"
          value={handle}
          onChange={(e) => setHandle(e.target.value)}
          required
        />
        <button type="submit">Sign in</button>
      </form>
      <p className="signin__create">
        New here? <a href={`/oauth/signup?${new URLSearchParams({ return_to: returnTo })}`}>Create account</a>
      </p>
    </main>
  )
}
