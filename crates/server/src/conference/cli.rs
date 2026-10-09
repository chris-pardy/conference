//! `conference-server admin …`: the operator's CLI. It runs with the server's
//! environment (DATABASE_URL, PUBLIC_URL, ATPROTO_URL, …) against the same
//! database as the running server.
//!
//! Commands that decide something take `--as <handle>`, an admin who has
//! connected (`connect`), and exit non-zero with the reason on stderr when
//! refused. `--json` makes a command print one JSON object as the last line
//! of stdout.

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{Value, json};

use super::decide::{self, Action, Actor, Outcome, Role};
use super::{
    Conference, EVENT, FEED, METHODS, SIDECAR, SpaceUri, attest, did_of, iso, outbox, repo,
};
use crate::AppState;
use crate::auth::session::{self, ADMIN, ORG};
use crate::config::{ADMIN_SCOPES, Config, ORG_SCOPES};
use crate::crypto::tid_now;
use crate::db::now_ms;
use crate::keys::random_token;

/// The commands and flags the tests rely on.
pub const USAGE: &str = "\
usage: conference-server admin <command> [--json]

  org connect <handle>
      Connects the organization's account over OAuth: prints a URL to open,
      waits for sign-in. Its DID document is left untouched. --json: {did,
      handle}
  keys add
      Adds another #eventside_attest key to eventside's own DID document
      (its did:web). --json: {key}
  connect <handle>
      Connects an admin over OAuth: prints a URL to open, waits for sign-in.
  conference create --org <did> --as <handle> --name <name> --starts <iso>
      --ends <iso> --city <city> [--country <code>] [--description <text>]
      [--theme <json>]
      Creates the space on the organization's PDS (simplespace.createSpace
      under its session) with managingAppPolicy {managingApp: <eventside
      did:web>#eventside_access}, publishes the public event and sidecar,
      and makes the acting admin the first owner. --json: {space, event}
  admin add <handle> --role owner|staff --conference <space> --as <handle>
  admin remove <handle> --conference <space> --as <handle>
  join set --conference <space> --methods code,list,open --as <handle>
  list import <file.csv> --conference <space> --as <handle>
      CSV with a handle column. Reports rows whose handle doesn't resolve.
  code create <code> --conference <space> --as <handle>
      [--expires <iso>] [--max-uses <n>]
  member add|remove|ban|unban <handle> --conference <space> --as <handle>
  member role <handle> --role owner|staff|speaker|attendee
      --conference <space> --as <handle>
  apps allow <client-id> --conference <space> --as <handle> [--use read]
  apps disallow <client-id> --conference <space> --as <handle>
  records list --conference <space> --collection <nsid> --as <handle>
      The records eventside counts in the space. --json: {records: [{uri,
      repo, collection, rkey, value}]}
";

/// Flags that take no value.
const SWITCHES: &[&str] = &["json"];

/// How long a connect link stays open.
const CONNECT_MS: i64 = 10 * 60 * 1000;

/// How long a command waits for the records its decisions write.
const SETTLE: Duration = Duration::from_secs(30);

struct Args {
    words: Vec<String>,
    flags: BTreeMap<String, String>,
}

impl Args {
    fn parse(raw: Vec<String>) -> Result<Self, String> {
        let mut words = Vec::new();
        let mut flags = BTreeMap::new();
        let mut it = raw.into_iter();
        while let Some(arg) = it.next() {
            match arg.strip_prefix("--") {
                Some(name) if SWITCHES.contains(&name) => {
                    flags.insert(name.to_owned(), "true".to_owned());
                }
                Some(name) => {
                    let value = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
                    flags.insert(name.to_owned(), value);
                }
                None => words.push(arg),
            }
        }
        Ok(Self { words, flags })
    }

    fn flag(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(String::as_str)
    }

    fn need(&self, name: &str) -> Result<&str, String> {
        self.flag(name).ok_or_else(|| format!("--{name} is required\n\n{USAGE}"))
    }

    fn word(&self, i: usize, what: &str) -> Result<&str, String> {
        self.words.get(i).map(String::as_str).ok_or_else(|| format!("missing {what}\n\n{USAGE}"))
    }
}

/// What a command reports: a line for people, and JSON for `--json`.
struct Done {
    text: String,
    json: Value,
}

fn done(text: impl Into<String>, json: Value) -> Result<Done, String> {
    Ok(Done { text: text.into(), json })
}

pub async fn main(raw: Vec<String>) -> ExitCode {
    if raw.is_empty() || raw.iter().any(|a| a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let args = match Args::parse(raw) {
        Ok(args) => args,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(2);
        }
    };
    let state = match build().await {
        Ok(state) => state,
        Err(why) => {
            eprintln!("conference-server admin: {why}");
            return ExitCode::FAILURE;
        }
    };
    match run(&state, &args).await {
        Ok(out) => {
            if !out.text.is_empty() {
                println!("{}", out.text);
            }
            if args.flag("json").is_some() {
                println!("{}", out.json);
            }
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

async fn build() -> Result<AppState, String> {
    let config = Config::from_env()?;
    let public_url = config
        .public_url
        .clone()
        .ok_or("PUBLIC_URL must be set: it's eventside's origin, and its did:web")?;
    let state = AppState::build(config, public_url).await?;
    attest::ensure_key(&state.db).await?;
    Ok(state)
}

async fn run(state: &AppState, args: &Args) -> Result<Done, String> {
    let words: Vec<&str> = args.words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["org", "connect", ..] => connect(state, args, ORG, 2).await,
        ["connect", ..] => connect(state, args, ADMIN, 1).await,
        ["keys", "add", ..] => keys_add(state).await,
        ["conference", "create", ..] => conference_create(state, args).await,
        ["admin", "add", ..] => admin_add(state, args).await,
        ["admin", "remove", ..] => decision(state, args, 2, |_| Ok(Action::Dismiss)).await,
        ["join", "set", ..] => join_set(state, args).await,
        ["list", "import", ..] => list_import(state, args).await,
        ["code", "create", ..] => code_create(state, args).await,
        ["member", "add", ..] => {
            decision(state, args, 2, |_| {
                Ok(Action::Admit { role: Role::Attendee, method: "admin".into() })
            })
            .await
        }
        ["member", "remove", ..] => decision(state, args, 2, |_| Ok(Action::Remove)).await,
        ["member", "ban", ..] => decision(state, args, 2, |_| Ok(Action::Ban)).await,
        ["member", "unban", ..] => decision(state, args, 2, |_| Ok(Action::Unban)).await,
        ["member", "role", ..] => {
            decision(state, args, 2, |args| {
                let role = args.need("role")?;
                Role::parse(role).map(Action::SetRole).ok_or_else(|| {
                    format!("--role must be owner, staff, speaker or attendee, not {role}")
                })
            })
            .await
        }
        ["apps", "allow", ..] => apps(state, args, true).await,
        ["apps", "disallow", ..] => apps(state, args, false).await,
        ["records", "list", ..] => records_list(state, args).await,
        _ => Err(format!("unknown command: {}\n\n{USAGE}", words.join(" "))),
    }
}

/// `connect <handle>` and `org connect <handle>`: prints a link to sign in
/// with, and waits until someone has.
async fn connect(state: &AppState, args: &Args, kind: &str, at: usize) -> Result<Done, String> {
    let handle = args.word(at, "the account's handle")?;
    let did = did_of(state, handle).await?;
    let scopes = if kind == ORG { ORG_SCOPES } else { ADMIN_SCOPES };
    let id = random_token(24);
    let expires_at = now_ms() + CONNECT_MS;
    sqlx::query(
        "INSERT INTO admin_connects (id, kind, did, handle, scopes, expires_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&id)
    .bind(kind)
    .bind(&did)
    .bind(handle)
    .bind(scopes.join(" "))
    .bind(expires_at)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let what = if kind == ORG { "the organization's account" } else { "an admin" };
    println!("To connect {handle} as {what}, open this in a browser and sign in as {handle}:");
    println!("{}/oauth/connect?id={id}", state.oauth.public_url);
    loop {
        let (completed, error) = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
            "SELECT completed_at, error FROM admin_connects WHERE id = $1",
        )
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        if completed.is_some() {
            break;
        }
        if let Some(error) = error {
            return Err(format!("connecting {handle} failed: {error}"));
        }
        if now_ms() > expires_at {
            return Err(format!("connecting {handle} timed out: nobody signed in"));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    done(format!("Connected {handle} ({did})."), json!({ "did": did, "handle": handle }))
}

/// `keys add`: another signing key, which signs from now on.
async fn keys_add(state: &AppState) -> Result<Done, String> {
    let fragment = attest::add_key(&state.db).await?;
    let key = format!("{}#{fragment}", super::eventside_did(&state.oauth.public_url));
    done(format!("Added {key}; it signs from now on."), json!({ "key": key }))
}

/// The admin a command acts as (`--as`): someone who has connected.
async fn acting(state: &AppState, args: &Args) -> Result<(String, String), String> {
    let handle = args.need("as")?;
    let did = did_of(state, handle).await?;
    let id_hash = sqlx::query_scalar::<_, String>(
        "SELECT id_hash FROM sessions WHERE did = $1 AND kind = 'admin' AND ended_at IS NULL \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&did)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let not_connected = || {
        format!(
            "{handle} isn't connected as an admin. Run `conference-server admin connect {handle}` first."
        )
    };
    let id_hash = id_hash.ok_or_else(not_connected)?;
    let row = session::load(&state.db, &id_hash)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(not_connected)?;
    if session::outdated(state, &row) {
        return Err(format!(
            "{handle} needs to reconnect: run `conference-server admin connect {handle}`."
        ));
    }
    Ok((did, handle.to_owned()))
}

/// The conference a command names (`--conference`).
async fn conference(state: &AppState, args: &Args) -> Result<Conference, String> {
    let space = args.need("conference")?;
    super::load(&state.db, space)
        .await?
        .ok_or_else(|| format!("{space} isn't a conference eventside knows"))
}

/// The acting admin, who must be an owner or staff member of the conference.
async fn organizer(
    state: &AppState,
    args: &Args,
    conference: &Conference,
) -> Result<(String, Role), String> {
    let (did, handle) = acting(state, args).await?;
    match decide::role(&state.db, &conference.space, &did).await? {
        Some(role) if role.is_admin() => Ok((did, role)),
        _ => Err(format!("{handle} isn't an owner or staff member of {}", conference.space)),
    }
}

/// Waits for the records a command's decisions write.
async fn settle(state: &AppState, conference: &str) {
    if let Err(why) = outbox::settle(state, conference, SETTLE).await {
        eprintln!(
            "warning: the decision stands, but its record isn't in the space yet ({why}). The server writes it when it can."
        );
    }
}

/// `conference create`.
async fn conference_create(state: &AppState, args: &Args) -> Result<Done, String> {
    let (actor, _) = acting(state, args).await?;
    let org = args.need("org")?.to_owned();
    repo::org_session(state, &org).await?;
    let name = args.need("name")?.trim().to_owned();
    let time = |flag: &str| -> Result<i64, String> {
        let value = args.need(flag)?;
        parse_iso(value).ok_or_else(|| {
            format!("--{flag} must be a date and time like 2027-04-29T09:00:00+02:00, not {value}")
        })
    };
    let (starts_ms, ends_ms) = (time("starts")?, time("ends")?);
    if ends_ms < starts_ms {
        return Err("--ends can't be before --starts".into());
    }
    // Kept and published as RFC 3339, in UTC.
    let (starts, ends) = (iso(starts_ms), iso(ends_ms));
    let city = args.need("city")?.trim().to_owned();
    if name.is_empty() || city.is_empty() {
        return Err("--name and --city can't be empty".into());
    }
    let description = args.flag("description").map(str::to_owned);
    let theme: Value = match args.flag("theme") {
        Some(theme) => serde_json::from_str(theme)
            .map_err(|e| format!("--theme must be a JSON object: {e}"))?,
        None => json!({}),
    };
    if !theme.as_object().is_some_and(|t| t.values().all(Value::is_string)) {
        return Err("--theme must be a JSON object of token names to values".into());
    }

    // Recorded as pending first, so a create that stops partway (the PDS
    // unreachable, the process killed) is finished by running it again.
    let pending = sqlx::query_scalar::<_, String>(
        "SELECT rkey FROM conferences WHERE org = $1 AND name = $2 AND starts_at = $3 \
         AND created_by = $4 AND status = 'pending' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&org)
    .bind(&name)
    .bind(&starts)
    .bind(&actor)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let resuming = pending.is_some();
    let rkey = pending.unwrap_or_else(tid_now);
    let space = SpaceUri::conference(&org, &rkey);
    let event_uri = format!("at://{org}/{EVENT}/{rkey}");
    let saved = if resuming {
        sqlx::query(
            "UPDATE conferences SET ends_at = $1, city = $2, description = $3, theme = $4 \
             WHERE space = $5 AND status = 'pending'",
        )
        .bind(&ends)
        .bind(&city)
        .bind(&description)
        .bind(theme.to_string())
        .bind(&space)
    } else {
        sqlx::query(
            "INSERT INTO conferences (space, org, rkey, event, name, starts_at, ends_at, city, description, \
             theme, methods, created_by, created_at, status) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, '', $11, $12, 'pending')",
        )
        .bind(&space)
        .bind(&org)
        .bind(&rkey)
        .bind(&event_uri)
        .bind(&name)
        .bind(&starts)
        .bind(&ends)
        .bind(&city)
        .bind(&description)
        .bind(theme.to_string())
        .bind(&actor)
        .bind(now_ms())
    };
    saved.execute(&state.db).await.map_err(|e| e.to_string())?;
    match repo::create_space(state, &org, &rkey).await {
        Ok(made) if made == space => {}
        Ok(made) => {
            return Err(format!(
                "the organization's PDS made {made}, not the conference space asked for"
            ));
        }
        // Made by the run this one finishes.
        Err(why) if resuming && why.contains("SpaceAlreadyExists") => {}
        Err(why) => return Err(why),
    }
    let page = format!("{}/c/{org}/{rkey}", state.oauth.public_url);
    let mut address = json!({ "$type": "community.lexicon.location.address", "locality": city });
    if let Some(country) = args.flag("country") {
        address["country"] = json!(country);
    }
    let mut event = json!({
        "$type": EVENT,
        "name": name,
        "startsAt": starts,
        "endsAt": ends,
        "mode": "community.lexicon.calendar.event#inperson",
        "status": "community.lexicon.calendar.event#scheduled",
        "locations": [address],
        "uris": [{ "uri": page, "name": "Eventside" }],
        "createdAt": iso(now_ms()),
    });
    if let Some(description) = &description {
        event["description"] = json!(description);
    }
    let event_uri = repo::put_public(state, &org, EVENT, &rkey, event).await?;
    let conference = Conference {
        space: space.clone(),
        org: org.clone(),
        rkey: rkey.clone(),
        event: event_uri.clone(),
        name: name.clone(),
        starts_at: starts.clone(),
        ends_at: ends.clone(),
        city: city.clone(),
        description: description.clone(),
        theme: theme.clone(),
        methods: Vec::new(),
    };
    write_sidecar(state, &conference).await?;

    let mut tx = decide::begin(state).await?;
    let finished = sqlx::query(
        "UPDATE conferences SET status = 'ready', event = $1 WHERE space = $2 AND status = 'pending'",
    )
    .bind(&event_uri)
    .bind(&space)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if finished.rows_affected() != 1 {
        return Err(format!("{space} was finished by another run"));
    }
    let first_owner = Action::Admit { role: Role::Owner, method: "admin".into() };
    decide::decide_in(&mut tx, &space, &actor, first_owner, &Actor::Creator(actor.clone())).await?;
    decide::enqueue(&mut tx, &space, outbox::CREATED_ENTRY, None).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    decide::committed();
    settle(state, &space).await;
    done(
        format!("Created {name}: {space}\nIts public page: {page}"),
        json!({ "space": space, "event": event_uri, "feed": format!("{space}/{org}/{FEED}/main") }),
    )
}

/// The public sidecar next to the event: what eventside shows of how to get in.
async fn write_sidecar(state: &AppState, conference: &Conference) -> Result<(), String> {
    let sidecar = json!({
        "$type": SIDECAR,
        "event": conference.event,
        "space": conference.space,
        "methods": conference.methods,
        "theme": conference.theme,
        "template": "main",
        "createdAt": iso(now_ms()),
    });
    repo::put_public(state, &conference.org, SIDECAR, &conference.rkey, sidecar).await.map(drop)
}

/// A decision about the person a command names (word `at`), as the acting admin.
async fn decision(
    state: &AppState,
    args: &Args,
    at: usize,
    action: impl FnOnce(&Args) -> Result<Action, String>,
) -> Result<Done, String> {
    let who = args.word(at, "the person's handle")?;
    let action = action(args)?;
    let conference = conference(state, args).await?;
    let (actor, actor_handle) = acting(state, args).await?;
    let subject = did_of(state, who).await?;
    let outcome =
        decide::decide(state, &conference.space, &subject, action, &Actor::Admin(actor.clone()))
            .await?;
    match outcome {
        Outcome::Decided(seq) => {
            settle(state, &conference.space).await;
            done(
                format!("Done: decision {seq} about {who}, by {actor_handle}."),
                json!({ "seq": seq }),
            )
        }
        Outcome::Unchanged => done(format!("Nothing to change for {who}."), json!({})),
        Outcome::Refused(why) => Err(format!("refused: {why}")),
    }
}

/// `admin add <handle> --role owner|staff`.
async fn admin_add(state: &AppState, args: &Args) -> Result<Done, String> {
    let role = args.need("role")?;
    let role = Role::parse(role)
        .filter(|r| r.is_admin())
        .ok_or_else(|| format!("--role must be owner or staff, not {role}"))?;
    decision(state, args, 2, |_| Ok(Action::Appoint(role))).await
}

/// `join set --methods code,list,open`.
async fn join_set(state: &AppState, args: &Args) -> Result<Done, String> {
    let mut conference = conference(state, args).await?;
    organizer(state, args, &conference).await?;
    let methods = super::split_methods(args.need("methods")?);
    if let Some(unknown) = methods.iter().find(|m| !METHODS.contains(&m.as_str())) {
        return Err(format!("unknown join method {unknown}: use code, list or open"));
    }
    // The public sidecar first: if it can't be written, nothing changes.
    conference.methods = methods.clone();
    write_sidecar(state, &conference).await?;
    sqlx::query("UPDATE conferences SET methods = $1 WHERE space = $2")
        .bind(methods.join(","))
        .bind(&conference.space)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    done(
        format!(
            "Join methods: {}",
            if methods.is_empty() { "none".into() } else { methods.join(", ") }
        ),
        json!({ "methods": methods }),
    )
}

/// `list import <file.csv>`: a ticketing tool's export, with a handle column.
/// Each handle is resolved now, and the DID it names is the one admitted.
async fn list_import(state: &AppState, args: &Args) -> Result<Done, String> {
    let file = args.word(2, "the CSV file")?;
    let conference = conference(state, args).await?;
    organizer(state, args, &conference).await?;
    let text = std::fs::read_to_string(file).map_err(|e| format!("couldn't read {file}: {e}"))?;
    let mut rows = text.lines().filter(|l| !l.trim().is_empty()).map(csv_row);
    let header = rows.next().ok_or_else(|| format!("{file} is empty"))?;
    let column = header
        .iter()
        .position(|h| h.trim().eq_ignore_ascii_case("handle"))
        .ok_or_else(|| format!("{file} has no handle column"))?;
    let mut imported = 0;
    let mut problems = Vec::new();
    for (i, row) in rows.enumerate() {
        let line = i + 2;
        let Some(handle) =
            row.get(column).map(|h| h.trim().trim_start_matches('@')).filter(|h| !h.is_empty())
        else {
            problems.push(format!("row {line}: no handle"));
            continue;
        };
        let did = match did_of(state, handle).await {
            Ok(did) => did,
            Err(why) => {
                problems.push(format!("row {line}: {handle}: {why}"));
                continue;
            }
        };
        sqlx::query(
            "INSERT INTO attendee_list (conference, did, handle, imported_at) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (conference, did) DO NOTHING",
        )
        .bind(&conference.space)
        .bind(&did)
        .bind(handle)
        .bind(now_ms())
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        imported += 1;
    }
    for problem in &problems {
        eprintln!("not imported: {problem}");
    }
    done(
        format!(
            "Imported {imported} of {} rows.{}",
            imported + problems.len(),
            if problems.is_empty() {
                String::new()
            } else {
                format!("\nNot imported:\n  {}", problems.join("\n  "))
            }
        ),
        json!({ "imported": imported, "problems": problems }),
    )
}

/// One CSV row: commas separate fields, double quotes quote them.
fn csv_row(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    fields
}

/// `code create <code> [--expires <iso>] [--max-uses <n>]`: a shared code.
async fn code_create(state: &AppState, args: &Args) -> Result<Done, String> {
    let code = args.word(2, "the code")?.trim();
    if code.is_empty() {
        return Err("the code can't be empty".into());
    }
    let conference = conference(state, args).await?;
    let (actor, _) = organizer(state, args, &conference).await?;
    let expires_at = match args.flag("expires") {
        Some(at) => Some(
            parse_iso(at).ok_or_else(|| format!("--expires must be a date and time, not {at}"))?,
        ),
        None => None,
    };
    let max_uses = match args.flag("max-uses") {
        Some(n) => Some(
            n.parse::<i64>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("--max-uses must be a positive number, not {n}"))?,
        ),
        None => None,
    };
    sqlx::query(
        "INSERT INTO codes (conference, code_hash, expires_at, max_uses, uses, created_by, created_at) \
         VALUES ($1, $2, $3, $4, 0, $5, $6) \
         ON CONFLICT (conference, code_hash) DO UPDATE SET expires_at = $3, max_uses = $4",
    )
    .bind(&conference.space)
    .bind(super::join::code_hash(&conference.space, code))
    .bind(expires_at)
    .bind(max_uses)
    .bind(&actor)
    .bind(now_ms())
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    if !conference.has_method("code") {
        eprintln!("note: joining by code is off for this conference; turn it on with `join set`.");
    }
    done(format!("The code works for {}.", conference.name), json!({ "code": code }))
}

/// `apps allow|disallow <client-id>`: which other apps may read the space.
async fn apps(state: &AppState, args: &Args, allow: bool) -> Result<Done, String> {
    let client = args.word(2, "the app's client ID")?.to_owned();
    let conference = conference(state, args).await?;
    organizer(state, args, &conference).await?;
    let uses = args.flag("use").unwrap_or("read");
    if uses.split(',').any(|u| !["read", "cardProvider", "feedGenerator"].contains(&u.trim())) {
        return Err(format!("--use must be read, cardProvider or feedGenerator, not {uses}"));
    }
    let mut tx = decide::begin(state).await?;
    let query = if allow {
        sqlx::query(
            "INSERT INTO conference_apps (conference, client_id, uses) VALUES ($1, $2, $3) \
             ON CONFLICT (conference, client_id) DO UPDATE SET uses = $3",
        )
        .bind(&conference.space)
        .bind(&client)
        .bind(uses)
    } else {
        sqlx::query("DELETE FROM conference_apps WHERE conference = $1 AND client_id = $2")
            .bind(&conference.space)
            .bind(&client)
    };
    query.execute(&mut *tx).await.map_err(|e| e.to_string())?;
    decide::enqueue(&mut tx, &conference.space, outbox::APPS_ENTRY, None).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    settle(state, &conference.space).await;
    let text = if allow {
        format!("{client} may now {uses} {}.", conference.name)
    } else {
        format!("{client} is no longer allowed.")
    };
    done(text, json!({ "client": client, "allowed": allow }))
}

/// `records list --collection <nsid>`: the records eventside counts in the
/// space, from its records index (see `sync`).
async fn records_list(state: &AppState, args: &Args) -> Result<Done, String> {
    let conference = conference(state, args).await?;
    organizer(state, args, &conference).await?;
    let collection = args.need("collection")?;
    // Read what's new first, so the answer is current.
    if let Err(why) = super::sync::sync_space(state, &conference).await {
        eprintln!("warning: the index may be behind: {why}");
    }
    let records = super::sync::counted_records(state, &conference, collection).await?;
    done(format!("{} records counted.", records.len()), json!({ "records": records }))
}

/// Parses an RFC 3339 date and time to milliseconds since the epoch.
pub fn parse_iso(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, rest) = text.split_once(['T', 't', ' '])?;
    let mut d = date.splitn(3, '-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    let zone_at = rest.find(['Z', 'z', '+', '-'])?;
    let (time, zone) = rest.split_at(zone_at);
    let mut t = time.splitn(3, ':');
    let hour: i64 = t.next()?.parse().ok()?;
    let minute: i64 = t.next()?.parse().ok()?;
    let seconds = t.next().unwrap_or("0");
    let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let second: i64 = whole.parse().ok()?;
    let millis: i64 = if fraction.is_empty() {
        0
    } else {
        format!("{:0<3}", &fraction[..fraction.len().min(3)]).parse().ok()?
    };
    let offset_min = match zone {
        "Z" | "z" => 0,
        _ => {
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let (h, m) = zone[1..].split_once(':')?;
            sign * (h.parse::<i64>().ok()? * 60 + m.parse::<i64>().ok()?)
        }
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=12).contains(&month)
        || !(1..=month_days).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + hour * 3600 + minute * 60 + second - offset_min * 60) * 1000) + millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_parse_with_their_offset() {
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso("1970-01-01T00:00:01.250Z"), Some(1250));
        assert_eq!(parse_iso("2027-04-29T09:00:00+02:00"), parse_iso("2027-04-29T07:00:00Z"));
        assert_eq!(iso(parse_iso("2026-10-09T12:34:56.789Z").unwrap()), "2026-10-09T12:34:56.789Z");
        assert_eq!(parse_iso("tomorrow"), None);
        assert_eq!(parse_iso("2027-13-01T00:00:00Z"), None);
        assert_eq!(parse_iso("2027-02-31T00:00:00Z"), None);
        assert_eq!(parse_iso("2027-04-31T00:00:00Z"), None);
        assert_eq!(parse_iso("2027-02-29T00:00:00Z"), None);
        assert!(parse_iso("2028-02-29T00:00:00Z").is_some());
        assert!(parse_iso("2000-02-29T00:00:00Z").is_some());
        assert_eq!(parse_iso("2100-02-29T00:00:00Z"), None);
    }

    #[test]
    fn csv_rows_split_on_commas_outside_quotes() {
        assert_eq!(csv_row("name,handle"), vec!["name", "handle"]);
        assert_eq!(csv_row("\"de Vries, Ana\",ana.test"), vec!["de Vries, Ana", "ana.test"]);
        assert_eq!(csv_row("\"say \"\"hi\"\"\",x"), vec!["say \"hi\"", "x"]);
    }
}
