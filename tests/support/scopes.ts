// The scopes the appview asks for at sign-in by default (`LOGIN_SCOPES` in
// crates/server/src/config.rs). Later features grow this list; tests that
// check the default scopes read it from here.

/** Writing your own join and leave records into any conference's intake space (conference-space). */
export const INTAKE_SCOPE =
  'space:app.eventside.intake?authority=*&action=create&action=delete&collection=app.eventside.intake.join&collection=app.eventside.intake.leave'

/** The default sign-in scope list, in the order the appview asks for it. */
export const SIGN_IN_SCOPES: readonly string[] = ['atproto', INTAKE_SCOPE]
