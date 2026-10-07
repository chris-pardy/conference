//! `conference-server admin …`: the operator's CLI. It runs with the server's
//! environment, against the same database, while the server runs. Every
//! action is written as an admin (`--as <handle>`, by default the super
//! admin) into their own repo, under the session they connected with, and
//! read back into the index before the command reports success.
//!
//! It exits non-zero with the reason on stderr when it refuses or fails. With
//! `--json`, its last line of stdout is one JSON object.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::admin::{Acting, connect_command};
use super::{EVENT, Row, SIDECAR};
use crate::AppState;
use crate::config::Config;
use crate::crypto::tid_now;
use crate::db::now_ms;
use crate::keys::random_token;
use crate::spacehost::index::{self, Conference, Org, Role};
use crate::spacehost::{CONFERENCE_TYPE, INTAKE_TYPE, SpaceUri, authority, sync};

const USAGE: &str = "usage: conference-server admin <command> [--json] [--as <handle>]

  org create --super-admin <handle> --recovery-key <did:key> [--name <name>]
  org show --org <did>
  org admin add|remove <handle> --org <did> [--role owner|staff]
  connect <handle>
  reindex --org <did>
  conference create --org <did> (--name … --starts … --ends … --city … [--description …] | --event <at-uri>)
                    [--invite-only] [--theme <json>] [--super-admin <handle>]
  conference super-admin <handle> --conference <space>
  join set --conference <space> --methods code,request,list,open
  codes issue --conference <space> (--shared <code> | --personal) [--expires <iso>] [--max-uses <n>]
  list import <csv> --conference <space>
  requests list --conference <space>
  requests approve|deny <handle> --conference <space>
  member add|remove|ban <handle> --conference <space>
  member role <handle> --role owner|staff|speaker|none --conference <space>
  apps add|remove <client-id> (--conference <space> | --org <did>)
  apps open|curate --conference <space> [--yes]
  rules set <file> --conference <space>";

/// The roles a person can have in a conference.
const ROLES: &[&str] = &["owner", "staff", "speaker"];

/// Flags that take no value.
const SWITCHES: &[&str] = &["json", "invite-only", "personal", "yes"];

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
    let args = match Args::parse(raw) {
        Ok(args) => args,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(2);
        }
    };
    if args.words.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
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
        .ok_or("PUBLIC_URL must be set: it's the space host our organizations' DIDs name")?;
    AppState::build(config, public_url).await
}

async fn run(state: &AppState, args: &Args) -> Result<Done, String> {
    let words: Vec<&str> = args.words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["org", "create", ..] => org_create(state, args).await,
        ["org", "show", ..] => org_show(state, args).await,
        ["org", "admin", "add", ..] => org_admin(state, args, true).await,
        ["org", "admin", "remove", ..] => org_admin(state, args, false).await,
        ["connect", ..] => connect(state, args).await,
        ["reindex", ..] => reindex(state, args).await,
        ["conference", "create", ..] => conference_create(state, args).await,
        ["conference", "super-admin", ..] => conference_handover(state, args).await,
        ["join", "set", ..] => join_set(state, args).await,
        ["codes", "issue", ..] => codes_issue(state, args).await,
        ["list", "import", ..] => list_import(state, args).await,
        ["requests", "list", ..] => requests_list(state, args).await,
        ["requests", action @ ("approve" | "deny"), ..] => decide(state, args, action).await,
        ["member", action @ ("add" | "remove" | "ban"), ..] => decide(state, args, action).await,
        ["member", "role", ..] => member_role(state, args).await,
        ["apps", action @ ("add" | "remove" | "open" | "curate"), ..] => {
            apps(state, args, action).await
        }
        ["rules", "set", ..] => rules_set(state, args).await,
        _ => Err(format!("unknown command: {}\n\n{USAGE}", words.join(" "))),
    }
}

/// A handle (or DID) to its DID.
async fn did_of(state: &AppState, who: &str) -> Result<String, String> {
    if who.starts_with("did:") {
        return Ok(who.to_owned());
    }
    state.resolver.resolve_handle(who).await.map_err(|e| format!("couldn't resolve {who}: {e:?}"))
}

/// A DID's handle, for messages; the DID when it has none.
async fn handle_of(state: &AppState, did: &str) -> String {
    match state.resolver.resolve_did(did).await {
        Ok(identity) if identity.handle != "handle.invalid" => identity.handle,
        _ => did.to_owned(),
    }
}

async fn org(state: &AppState, did: &str) -> Result<Arc<Org>, String> {
    index::load(state, did).await?.ok_or_else(|| format!("{did} isn't an organization we host"))
}

/// A conference, by its space URI, with its organization.
async fn conference(state: &AppState, args: &Args) -> Result<(SpaceUri, Arc<Org>), String> {
    let uri = args.need("conference")?;
    let space = SpaceUri::parse(uri)
        .filter(|s| s.kind == CONFERENCE_TYPE)
        .ok_or_else(|| format!("{uri} isn't a conference space"))?;
    let org = org(state, &space.authority).await?;
    if org.conference(uri).is_none() {
        return Err(format!("{uri} isn't one of {}'s conferences", org.did));
    }
    Ok((space, org))
}

/// The admin a command acts as: `--as`, or the organization's super admin.
async fn acting(
    state: &AppState,
    args: &Args,
    org: &Org,
) -> Result<(Acting, Option<Role>), String> {
    let (did, handle) = match args.flag("as") {
        Some(handle) => (did_of(state, handle).await?, handle.to_owned()),
        None => (org.super_admin.clone(), handle_of(state, &org.super_admin).await),
    };
    let role = org.admin_role(&did);
    if role.is_none() {
        return Err(format!("{handle} isn't an admin of {}", org.did));
    }
    Ok((Acting::new(state, &did, &handle).await?, role))
}

/// The conference's super admin, who writes its roles and rules.
async fn conference_super_admin(
    state: &AppState,
    org: &Org,
    conference: &Conference,
) -> Result<Acting, String> {
    let _ = org;
    let did = conference.super_admin().to_owned();
    let handle = handle_of(state, &did).await;
    Acting::new(state, &did, &handle).await
}

fn only_super_admin(org: &Org, acting: &Acting, what: &str) -> Result<(), String> {
    if acting.did == org.super_admin {
        Ok(())
    } else {
        Err(format!("only the super admin can {what}; {} can't", acting.handle))
    }
}

fn only_owners(role: Option<Role>, acting: &Acting, what: &str) -> Result<(), String> {
    if role == Some(Role::Owner) {
        Ok(())
    } else {
        Err(format!("only owners can {what}; {} is staff", acting.handle))
    }
}

/// Revokes the credentials of anyone a change took access from.
async fn after_change(state: &AppState, before: &Org) -> Result<Arc<Org>, String> {
    let after = org(state, &before.did).await?;
    super::revoke_lost(state, &super::access(before), &after).await;
    Ok(after)
}

async fn org_create(state: &AppState, args: &Args) -> Result<Done, String> {
    let super_admin = did_of(state, args.need("super-admin")?).await?;
    let recovery_key = args.need("recovery-key")?;
    let did = authority::mint(state, &super_admin, recovery_key, args.flag("name")).await?;
    done(
        format!(
            "Created the organization {did}, with us as its space host.\nNext: `{}` to connect its super admin.",
            connect_command(args.need("super-admin")?)
        ),
        json!({ "did": did }),
    )
}

async fn org_show(state: &AppState, args: &Args) -> Result<Done, String> {
    let org = org(state, args.need("org")?).await?;
    let mut admins = Vec::new();
    let mut lines = vec![format!("{} (super admin {})", org.did, org.super_admin)];
    for did in org.admins.keys() {
        let role = org.admin_role(did).map_or("staff", Role::as_str);
        let connected = connected(state, did).await?;
        lines.push(format!(
            "  {did} {role} {}",
            if connected { "connected" } else { "not connected" }
        ));
        admins.push(json!({ "did": did, "role": role, "connected": connected }));
    }
    done(
        lines.join("\n"),
        json!({ "did": org.did, "superAdmin": org.super_admin, "admins": admins }),
    )
}

async fn connected(state: &AppState, did: &str) -> Result<bool, String> {
    let live = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sessions WHERE did = $1 AND kind = 'admin' AND ended_at IS NULL",
    )
    .bind(did)
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(live > 0)
}

async fn org_admin(state: &AppState, args: &Args, add: bool) -> Result<Done, String> {
    let who = args.word(3, "the admin's handle")?;
    let subject = did_of(state, who).await?;
    let org = org(state, args.need("org")?).await?;
    let (acting, _) = acting(state, args, &org).await?;
    only_super_admin(&org, &acting, "add or remove admins")?;
    let admin_space = SpaceUri::admin(&org.did).to_string();
    if add {
        let role = args.flag("role").unwrap_or("staff");
        let role = Role::parse(role)
            .ok_or_else(|| format!("--role must be owner or staff, not {role}"))?;
        // The super admin is the organization's owner for good: an `admin`
        // record can't make them staff.
        if subject == org.super_admin && role != Role::Owner {
            return Err(format!(
                "{who} is the super admin of {}: always an owner, and can't be made staff",
                org.did
            ));
        }
        // A conference's super admin writes its roles and rules: an owner.
        if role != Role::Owner
            && let Some(conference) = org.conferences.values().find(|c| c.super_admin() == subject)
        {
            return Err(format!(
                "{who} is the super admin of the conference {}, which takes an owner: give it \
                 another first, with `conference super-admin <handle> --conference {}`",
                conference.space(),
                conference.space()
            ));
        }
        acting
            .put_in(
                state,
                &admin_space,
                index::ADMIN,
                &subject,
                json!({ "subject": subject, "role": role.as_str() }),
            )
            .await?;
        // Admins are members of every conference, with their role. A new
        // admin's period (and membership) starts with a `member` record of
        // the super admin's, so it keeps its start through a change of role,
        // and its end once they're removed.
        let now = index::iso(now_ms() as u64 * 1000);
        let new_admin = !org.admins.contains_key(&subject);
        for conference in org.conferences.values() {
            if new_admin {
                acting
                    .create_in(
                        state,
                        &admin_space,
                        index::MEMBER,
                        None,
                        json!({ "space": conference.space(), "subject": subject, "via": index::VIA_ADMIN, "since": now }),
                    )
                    .await?;
            }
            let writer = conference_super_admin(state, &org, conference).await?;
            writer
                .put_in(
                    state,
                    conference.space(),
                    index::ROLE,
                    &subject,
                    json!({ "subject": subject, "role": role.as_str() }),
                )
                .await?;
        }
        after_change(state, &org).await?;
        return done(
            format!("{who} is now {} of {}.", role.as_str(), org.did),
            json!({ "did": subject, "role": role.as_str() }),
        );
    }
    // The super admin is always an owner, with or without an `admin` record.
    let owners: BTreeSet<&String> = org
        .admins
        .iter()
        .filter(|(_, a)| a.role == Role::Owner)
        .map(|(d, _)| d)
        .chain(std::iter::once(&org.super_admin))
        .collect();
    if owners.len() == 1 && owners.contains(&subject) {
        return Err(format!("{who} is the last owner of {}: add another owner first", org.did));
    }
    if !org.admins.contains_key(&subject) {
        return Err(format!("{who} isn't an admin of {}", org.did));
    }
    // The super admin stays the super admin (and an owner, and a member of
    // every conference); only their place on the list of admins goes.
    if subject == org.super_admin {
        acting.delete_in(state, &admin_space, index::ADMIN, &subject).await?;
        after_change(state, &org).await?;
        return done(
            format!("{who} is off the list of admins, and still the super admin of {}.", org.did),
            json!({ "did": subject }),
        );
    }
    // A conference's super admin writes its roles and rules, which count
    // only while they're an admin.
    if let Some(conference) = org.conferences.values().find(|c| c.super_admin() == subject) {
        return Err(format!(
            "{who} is the super admin of the conference {}: give it another first, with \
             `conference super-admin <handle> --conference {}`",
            conference.space(),
            conference.space()
        ));
    }
    // Their admin period and membership of each conference end now; what
    // they wrote while they were one stays theirs.
    let now = index::iso(now_ms() as u64 * 1000);
    for conference in org.conferences.values() {
        acting
            .create_in(
                state,
                &admin_space,
                index::MEMBER,
                None,
                json!({ "space": conference.space(), "subject": subject, "via": index::VIA_ADMIN_REMOVED, "until": now }),
            )
            .await?;
    }
    acting.delete_in(state, &admin_space, index::ADMIN, &subject).await?;
    // Their own role in each conference goes, and so do the roles they
    // assigned (which stop counting anyway, now they're not an admin).
    for conference in org.conferences.values() {
        let assigned = conference
            .role_deciders
            .iter()
            .filter(|(_, decider)| **decider == subject)
            .map(|(person, _)| person.clone());
        let gone: BTreeSet<String> = assigned
            .chain(conference.roles.contains_key(&subject).then(|| subject.clone()))
            .collect();
        if gone.is_empty() {
            continue;
        }
        let writer = conference_super_admin(state, &org, conference).await?;
        for person in gone {
            writer.delete_in(state, conference.space(), index::ROLE, &person).await?;
        }
    }
    after_change(state, &org).await?;
    done(format!("{who} is no longer an admin of {}.", org.did), json!({ "did": subject }))
}

/// Connects an admin: prints a URL for them to open, waits while they sign
/// in, and (for a super admin starting out) writes their first records.
async fn connect(state: &AppState, args: &Args) -> Result<Done, String> {
    let handle = args.word(1, "the admin's handle")?;
    let did = did_of(state, handle).await?;
    let id = random_token(24);
    let expires_at = now_ms() + 10 * 60 * 1000;
    sqlx::query("INSERT INTO admin_connects (id, did, handle, scopes, expires_at) VALUES ($1, $2, $3, $4, $5)")
        .bind(&id)
        .bind(&did)
        .bind(handle)
        .bind(state.config.admin_scopes.join(" "))
        .bind(expires_at)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    println!("To connect {handle} as an admin, open this in a browser and sign in as them:");
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
    // A super admin's first records: themselves as owner, and the admin space's own.
    for authority in authority::all(&state.db).await? {
        if authority.super_admin != did {
            continue;
        }
        let org = org(state, &authority.did).await?;
        let admin_space = SpaceUri::admin(&org.did).to_string();
        let acting = Acting::new(state, &did, handle).await?;
        if org.admins.is_empty() {
            acting
                .put_in(
                    state,
                    &admin_space,
                    index::ADMIN,
                    &did,
                    json!({ "subject": did, "role": "owner" }),
                )
                .await?;
        }
        if org.admin_settings.us == 0 {
            let eventside = state.oauth.client_id_for("atproto");
            acting
                .create_in(
                    state,
                    &admin_space,
                    index::SPACE,
                    None,
                    json!({
                        "space": admin_space,
                        "type": crate::spacehost::ADMIN_TYPE,
                        "readPolicy": "members",
                        "writePolicy": "members",
                        "appAccess": "allowList",
                        "allowList": [eventside],
                    }),
                )
                .await?;
        }
    }
    done(format!("Connected {handle} as an admin."), json!({ "did": did }))
}

async fn reindex(state: &AppState, args: &Args) -> Result<Done, String> {
    let did = args.need("org")?;
    let before = org(state, did).await?;
    let sync::Reindexed { org, unread } = sync::reindex(state, did).await?;
    super::revoke_lost(state, &super::access(&before), &org).await;
    let missing = super::rebuild_rows(state, &org).await;
    if !unread.is_empty() || !missing.is_empty() {
        let mut why = vec![format!("Rebuilt {did}'s index, but not all of it:")];
        why.extend(unread.iter().map(|r| format!("  couldn't read {r}; its records were kept")));
        why.extend(missing.iter().map(|m| format!("  couldn't rebuild the conference {m}")));
        return Err(why.join("\n"));
    }
    let members: usize = org.conferences.values().map(|c| c.current_members().count()).sum();
    done(
        format!(
            "Rebuilt {did}'s index: {} admins, {} conferences, {members} members.",
            org.admins.len(),
            org.conferences.len()
        ),
        json!({ "did": did, "admins": org.admins.len(), "conferences": org.conferences.len() }),
    )
}

async fn conference_create(state: &AppState, args: &Args) -> Result<Done, String> {
    let org = org(state, args.need("org")?).await?;
    let (org_admin, _) =
        acting(state, &Args { words: vec![], flags: BTreeMap::new() }, &org).await?;
    let super_admin = match args.flag("super-admin") {
        Some(handle) => {
            let did = did_of(state, handle).await?;
            // They write the conference's records into its space, so they
            // must be someone whose writes it takes: an admin, and an owner,
            // since the roles and rules are theirs to write.
            only_owner_super_admin(&org, &did, handle)?;
            Acting::new(state, &did, handle).await?
        }
        None => Acting::new(state, &org.super_admin, &org_admin.handle).await?,
    };
    let invite_only = args.flag("invite-only").is_some();
    let theme: Option<Value> = match args.flag("theme") {
        Some(theme) => {
            Some(serde_json::from_str(theme).map_err(|e| format!("--theme isn't JSON: {e}"))?)
        }
        None => None,
    };

    // What the conference is: an event the super admin already published, or a new one.
    let (event_value, adopted) = match args.flag("event") {
        Some(uri) => (adopt(state, &super_admin, uri).await?, Some(uri.to_owned())),
        None => {
            let name = args.need("name")?;
            let starts = args.need("starts")?;
            let ends = args.need("ends")?;
            let city = args.need("city")?;
            for (flag, value) in [("starts", starts), ("ends", ends)] {
                index::parse_iso_ms(value)
                    .ok_or_else(|| format!("--{flag} isn't a date and time: {value}"))?;
            }
            let mut event = json!({
                "name": name,
                "startsAt": starts,
                "endsAt": ends,
                "mode": "community.lexicon.calendar.event#inperson",
                "status": "community.lexicon.calendar.event#scheduled",
                "locations": [{ "$type": "community.lexicon.location.address", "locality": city }],
            });
            if let Some(description) = args.flag("description") {
                event["description"] = json!(description);
            }
            (event, None)
        }
    };

    // Everything is checked; now the writes. If one fails, the ones before it
    // are undone, so a retry starts clean instead of leaving a half-made
    // conference (or a stray public event) behind.
    let new = NewConference {
        org: &org,
        org_admin: &org_admin,
        super_admin: &super_admin,
        skey: tid_now(),
        invite_only,
        theme,
        event_value,
        adopted,
    };
    let mut undo = Vec::new();
    match new.write(state, &mut undo).await {
        Ok(done) => Ok(done),
        Err(why) => {
            let mut left = Vec::new();
            for step in undo.into_iter().rev() {
                if let Err(also) = step.undo(state, &org_admin, &super_admin).await {
                    left.push(also);
                }
            }
            if left.is_empty() {
                Err(format!("{why}\nNothing was created."))
            } else {
                Err(format!("{why}\nUndoing it failed too: {}", left.join("; ")))
            }
        }
    }
}

/// Gives a conference another super admin (an owner of the organization):
/// they re-issue its role and rules records (and an invite-only one's event
/// and sidecar) in their own repo, a new settings snapshot names them, and
/// a public conference's sidecar is pointed at them. From the snapshot on,
/// only the new super admin's role and rules records count; the old ones
/// are then deleted. A public conference's event and sidecar stay where
/// they were published, in the first super admin's repo.
///
/// Everything is read before anything is written, and run again it
/// finishes a handover that was cut short.
async fn conference_handover(state: &AppState, args: &Args) -> Result<Done, String> {
    let who = args.word(2, "the new super admin's handle")?;
    let subject = did_of(state, who).await?;
    let (space, org) = conference(state, args).await?;
    let space = space.to_string();
    let (acting, _) = acting(state, args, &org).await?;
    only_super_admin(&org, &acting, "change a conference's super admin")?;
    only_owner_super_admin(&org, &subject, who)?;
    let conference = org.conference(&space).ok_or("no such conference")?;
    let current = conference.super_admin().to_owned();
    let sidecar = public_sidecar(state, conference).await?;
    let new = Acting::new(state, &subject, who).await?;
    let mut changed = false;
    // An invite-only conference's event is in the space, the one its sidecar
    // names. Every other calendar event in the space is someone's plan, the
    // super admins' own included, and stays theirs.
    let event_rkey = if conference.settings.invite_only {
        match in_space_event_rkey(state, &space, &current).await? {
            Some(rkey) => Some(rkey),
            None => in_space_event_rkey(state, &space, &subject).await?,
        }
    } else {
        None
    };

    if current != subject {
        // What the current super admin wrote into the space, re-issued by the new.
        let rows = sqlx::query_as::<_, (String, String, String, String, Option<i64>)>(
            "SELECT r.collection, r.rkey, r.rev, r.value, s.seen_at FROM space_records r \
             LEFT JOIN space_record_seen s ON s.space = r.space AND s.repo = r.repo \
             AND s.collection = r.collection AND s.rkey = r.rkey AND s.rev = r.rev \
             WHERE r.space = $1 AND r.repo = $2 AND r.collection IN ($3, $4, $5, $6) \
             AND r.value IS NOT NULL",
        )
        .bind(&space)
        .bind(&current)
        .bind(index::ROLE)
        .bind(index::RULES)
        .bind(EVENT)
        .bind(SIDECAR)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        let mut copies = Vec::new();
        for (collection, rkey, rev, value, seen_at) in rows {
            if !super_admins_record(&collection, &rkey, event_rkey.as_deref()) {
                continue;
            }
            let Ok(mut value) = serde_json::from_str::<Value>(&value) else { continue };
            let collection: &'static str = match collection.as_str() {
                index::ROLE => {
                    // A role keeps the time it took effect.
                    if value.get("since").is_none()
                        && let Some(us) = crate::crypto::tid_micros(&rev)
                    {
                        let seen_us = seen_at.map(|ms| ms.max(0) as u64 * 1000);
                        value["since"] = json!(index::iso(index::conference_us(us, seen_us)));
                    }
                    index::ROLE
                }
                index::RULES => index::RULES,
                EVENT => EVENT,
                _ => {
                    value["superAdmin"] = json!(subject);
                    if let Some(event) = value.get("event").and_then(Value::as_str) {
                        value["event"] = json!(event.replace(
                            &format!("{space}/{current}/"),
                            &format!("{space}/{subject}/")
                        ));
                    }
                    SIDECAR
                }
            };
            copies.push((collection, rkey, value));
        }
        for (collection, rkey, value) in copies {
            new.put_in(state, &space, collection, &rkey, value).await?;
        }
        // From this snapshot on, the new super admin's records are the ones.
        settings_snapshot(state, &acting, &org, &space, |v| v["superAdmin"] = json!(subject))
            .await?;
        changed = true;
    }

    // The rest finishes the handover, this one or one cut short before.
    if let Some((writer, rkey, mut value)) = sidecar
        && value.get("superAdmin").and_then(Value::as_str) != Some(subject.as_str())
    {
        value["superAdmin"] = json!(subject);
        writer.put_public(state, SIDECAR, &rkey, value).await?;
        changed = true;
    }
    let mut row = super::row(state, &space).await?.ok_or("no such conference")?;
    if row.super_admin != subject {
        row.super_admin = subject.clone();
        super::save(state, &row).await?;
        changed = true;
    }
    // Earlier super admins' records count no more; they're tidied away.
    let mut earlier = conference.past_super_admins(&org.super_admin);
    earlier.insert(current);
    earlier.remove(&subject);
    for repo in earlier {
        let left = sqlx::query_as::<_, (String, String)>(
            "SELECT collection, rkey FROM space_records WHERE space = $1 AND repo = $2 \
             AND collection IN ($3, $4, $5, $6) AND value IS NOT NULL",
        )
        .bind(&space)
        .bind(&repo)
        .bind(index::ROLE)
        .bind(index::RULES)
        .bind(EVENT)
        .bind(SIDECAR)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        let theirs = if conference.settings.invite_only {
            in_space_event_rkey(state, &space, &repo).await?.or_else(|| event_rkey.clone())
        } else {
            None
        };
        let mut left: Vec<(String, String)> = left
            .into_iter()
            .filter(|(collection, rkey)| super_admins_record(collection, rkey, theirs.as_deref()))
            .collect();
        // The event before the sidecar that names it, so a rerun still finds it.
        left.sort_by_key(|(collection, _)| collection == SIDECAR);
        if left.is_empty() {
            continue;
        }
        let old = match Acting::new(state, &repo, &handle_of(state, &repo).await).await {
            Ok(old) => old,
            Err(why) => {
                eprintln!("note: couldn't delete {repo}'s old records: {why}");
                continue;
            }
        };
        for (collection, rkey) in left {
            match old.delete_in(state, &space, &collection, &rkey).await {
                Ok(()) => changed = true,
                Err(why) => {
                    eprintln!("note: couldn't delete the old {collection} record {rkey}: {why}")
                }
            }
        }
    }
    if !changed {
        return done(
            format!("{who} is already its super admin."),
            json!({ "space": space, "superAdmin": subject }),
        );
    }
    after_change(state, &org).await?;
    done(
        format!("{who} is now the super admin of {space}."),
        json!({ "space": space, "superAdmin": subject }),
    )
}

/// Whether a record in the conference space is one the super admin keeps as
/// super admin (and a handover moves): the roles, the rules, the sidecar and,
/// for an invite-only conference, the event the sidecar names. Their other
/// records, such as plans, are their own.
fn super_admins_record(collection: &str, rkey: &str, event_rkey: Option<&str>) -> bool {
    match collection {
        index::ROLE => true,
        index::RULES | SIDECAR => rkey == "self",
        EVENT => event_rkey == Some(rkey),
        _ => false,
    }
}

/// The rkey of the event an invite-only conference's sidecar in `repo` names.
async fn in_space_event_rkey(
    state: &AppState,
    space: &str,
    repo: &str,
) -> Result<Option<String>, String> {
    let value = sqlx::query_scalar::<_, String>(
        "SELECT value FROM space_records WHERE space = $1 AND repo = $2 AND collection = $3 \
         AND rkey = 'self' AND value IS NOT NULL",
    )
    .bind(space)
    .bind(repo)
    .bind(SIDECAR)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(value
        .and_then(|v| serde_json::from_str::<Value>(&v).ok())
        .and_then(|v| v.get("event").and_then(Value::as_str).map(str::to_owned))
        .and_then(|uri| uri.rsplit('/').next().map(str::to_owned))
        .filter(|rkey| !rkey.is_empty()))
}

/// A conference's super admin writes its roles and rules, which are an
/// owner's to decide: they must be an owner.
fn only_owner_super_admin(org: &Org, did: &str, who: &str) -> Result<(), String> {
    if !org.is_admin(did) {
        return Err(format!(
            "{who} isn't an admin of {}: add them with `org admin add` first",
            org.did
        ));
    }
    if org.admin_role(did) != Some(Role::Owner) {
        return Err(format!(
            "{who} is staff of {}: a conference's super admin must be an owner",
            org.did
        ));
    }
    Ok(())
}

/// A public conference's sidecar, beside its event in the repo that
/// published it, with that account's session to update it. None for an
/// invite-only conference, or (with a note) when the account that published
/// it isn't connected.
async fn public_sidecar(
    state: &AppState,
    conference: &Conference,
) -> Result<Option<(Acting, String, Value)>, String> {
    let Some(event) = &conference.settings.event else { return Ok(None) };
    let parts: Vec<&str> = event.strip_prefix("at://").unwrap_or_default().split('/').collect();
    let [repo, _, rkey] = parts.as_slice() else {
        return Err(format!("the conference's event {event} isn't a record's AT-URI"));
    };
    let value = super::public_record(state, &format!("at://{repo}/{SIDECAR}/{rkey}")).await?;
    let handle = handle_of(state, repo).await;
    match Acting::new(state, repo, &handle).await {
        Ok(writer) => Ok(Some((writer, (*rkey).to_owned(), value))),
        Err(why) => {
            eprintln!("note: the public sidecar at://{repo}/{SIDECAR}/{rkey} isn't updated: {why}");
            Ok(None)
        }
    }
}

/// A record `conference create` wrote, to delete if a later step fails.
enum Written {
    /// In the super admin's public repo.
    Public { collection: &'static str, rkey: String },
    /// In a space, by the organization's super admin (`settings`) or the
    /// conference's.
    InSpace { settings: bool, space: String, collection: &'static str, rkey: String },
}

impl Written {
    async fn undo(
        self,
        state: &AppState,
        org_admin: &Acting,
        super_admin: &Acting,
    ) -> Result<(), String> {
        match self {
            Self::Public { collection, rkey } => {
                super_admin.delete_public(state, collection, &rkey).await
            }
            Self::InSpace { settings, space, collection, rkey } => {
                let writer = if settings { org_admin } else { super_admin };
                writer.delete_in(state, &space, collection, &rkey).await
            }
        }
    }
}

struct NewConference<'a> {
    org: &'a Org,
    org_admin: &'a Acting,
    super_admin: &'a Acting,
    skey: String,
    invite_only: bool,
    theme: Option<Value>,
    event_value: Value,
    adopted: Option<String>,
}

impl NewConference<'_> {
    async fn write(&self, state: &AppState, undo: &mut Vec<Written>) -> Result<Done, String> {
        let (org, super_admin, invite_only) = (self.org, self.super_admin, self.invite_only);
        let space = SpaceUri::new(&org.did, CONFERENCE_TYPE, &self.skey).to_string();
        let intake = SpaceUri::new(&org.did, INTAKE_TYPE, &self.skey).to_string();
        let admin_space = SpaceUri::admin(&org.did).to_string();
        let eventside = state.oauth.client_id_for("atproto");
        let visibility = if invite_only { "inviteOnly" } else { "public" };
        let mut settings = json!({
            "space": space,
            "type": CONFERENCE_TYPE,
            "readPolicy": "members",
            "writePolicy": "members",
            "appAccess": "allowList",
            "allowList": [eventside],
            "join": { "methods": [] },
            "intake": intake,
            "superAdmin": super_admin.did,
            "visibility": visibility,
        });

        // The entry point: public records in the super admin's repo, or inside the space.
        let mut sidecar = json!({
            "space": space,
            "intake": intake,
            "organization": org.did,
            "superAdmin": super_admin.did,
            "visibility": visibility,
        });
        if let Some(theme) = &self.theme {
            sidecar["theme"] = theme.clone();
        }
        let event_uri = if invite_only {
            None
        } else {
            let uri = match &self.adopted {
                Some(uri) => uri.clone(),
                None => {
                    let uri = super_admin
                        .create_public(state, EVENT, Some(&self.skey), self.event_value.clone())
                        .await?;
                    undo.push(Written::Public { collection: EVENT, rkey: self.skey.clone() });
                    uri
                }
            };
            let rkey = uri.rsplit('/').next().unwrap_or_default().to_owned();
            sidecar["event"] = json!(uri);
            super_admin.create_public(state, SIDECAR, Some(&rkey), sidecar.clone()).await?;
            undo.push(Written::Public { collection: SIDECAR, rkey });
            settings["event"] = json!(uri);
            Some(uri)
        };
        // Its settings before anything is written into the space, so the space
        // is ours (and takes the super admin's writes) by then.
        let rkey =
            self.org_admin.create_in(state, &admin_space, index::SPACE, None, settings).await?;
        undo.push(Written::InSpace {
            settings: true,
            space: admin_space.clone(),
            collection: index::SPACE,
            rkey,
        });
        if invite_only {
            let rkey = self.skey.clone();
            let event_uri = format!("{space}/{}/{EVENT}/{rkey}", super_admin.did);
            let in_space = |collection: &'static str, rkey: &str| Written::InSpace {
                settings: false,
                space: space.clone(),
                collection,
                rkey: rkey.to_owned(),
            };
            super_admin.put_in(state, &space, EVENT, &rkey, self.event_value.clone()).await?;
            undo.push(in_space(EVENT, &rkey));
            sidecar["event"] = json!(event_uri);
            super_admin.put_in(state, &space, SIDECAR, "self", sidecar).await?;
            undo.push(in_space(SIDECAR, "self"));
        }

        // The organization's admins: members (by the super admin's decision,
        // which a later removal ends) with their roles. Then the rules.
        let now = index::iso(now_ms() as u64 * 1000);
        for did in org.admins.keys() {
            let role = org.admin_role(did).unwrap_or(Role::Staff);
            let rkey = self
                .org_admin
                .create_in(
                    state,
                    &admin_space,
                    index::MEMBER,
                    None,
                    json!({ "space": space, "subject": did, "via": index::VIA_ADMIN, "since": now }),
                )
                .await?;
            undo.push(Written::InSpace {
                settings: true,
                space: admin_space.clone(),
                collection: index::MEMBER,
                rkey,
            });
            super_admin
                .put_in(
                    state,
                    &space,
                    index::ROLE,
                    did,
                    json!({ "subject": did, "role": role.as_str() }),
                )
                .await?;
            undo.push(Written::InSpace {
                settings: false,
                space: space.clone(),
                collection: index::ROLE,
                rkey: did.clone(),
            });
        }
        let rules: Vec<Value> = index::default_rules()
            .into_iter()
            .map(|(collection, writers)| json!({ "collection": collection, "writers": writers }))
            .collect();
        super_admin.put_in(state, &space, index::RULES, "self", json!({ "rules": rules })).await?;
        undo.push(Written::InSpace {
            settings: false,
            space: space.clone(),
            collection: index::RULES,
            rkey: "self".to_owned(),
        });

        super::save(
            state,
            &Row {
                space: space.clone(),
                org: org.did.clone(),
                intake: intake.clone(),
                super_admin: super_admin.did.clone(),
                event: event_uri.clone(),
                invite_only: i64::from(invite_only),
                info: super::info_from_event(&self.event_value, self.theme.as_ref()).to_string(),
            },
        )
        .await?;
        let name = self.event_value.get("name").and_then(Value::as_str).unwrap_or("The conference");
        let mut out = json!({ "space": space, "intake": intake });
        if let Some(event) = &event_uri {
            out["event"] = json!(event);
        }
        done(format!("Created {name}: {space}"), out)
    }
}

/// An event to adopt: it must be in the conference super admin's own repo.
async fn adopt(state: &AppState, super_admin: &Acting, uri: &str) -> Result<Value, String> {
    let rest = uri.strip_prefix("at://").ok_or_else(|| format!("{uri} isn't an AT-URI"))?;
    let parts: Vec<&str> = rest.split('/').collect();
    let [repo, collection, rkey] = parts.as_slice() else {
        return Err(format!("{uri} isn't a record's AT-URI"));
    };
    if *collection != EVENT {
        return Err(format!("{uri} isn't a calendar event"));
    }
    let repo = did_of(state, repo).await?;
    if repo != super_admin.did {
        return Err(format!(
            "the event must be in the super admin's own repo: {uri} is in {repo}'s, not {}'s",
            super_admin.handle
        ));
    }
    let pds = state.host.pds_of(state, &repo).await?;
    let url = format!("{pds}/xrpc/com.atproto.repo.getRecord");
    let res = state
        .http
        .guarded(&url)?
        .get(&url)
        .query(&[("repo", repo.as_str()), ("collection", collection), ("rkey", rkey)])
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("couldn't read {uri}: {}", res.status()));
    }
    let record: Value = crate::net::read_json(res).await?;
    record.get("value").cloned().ok_or_else(|| format!("{uri} has no value"))
}

/// A new snapshot of a space's settings, changed by `change`.
async fn settings_snapshot(
    state: &AppState,
    acting: &Acting,
    org: &Org,
    space: &str,
    change: impl FnOnce(&mut Value),
) -> Result<(), String> {
    let admin_space = SpaceUri::admin(&org.did).to_string();
    let mut value = latest_settings(state, org, space).await?;
    change(&mut value);
    value.as_object_mut().map(|o| o.remove("createdAt"));
    acting.create_in(state, &admin_space, index::SPACE, None, value).await?;
    Ok(())
}

/// The super admin's latest settings record for a space, or the admin
/// space's defaults.
async fn latest_settings(state: &AppState, org: &Org, space: &str) -> Result<Value, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT rev, value FROM space_records WHERE space = $1 AND repo = $2 AND collection = $3 AND value IS NOT NULL",
    )
    .bind(SpaceUri::admin(&org.did).to_string())
    .bind(&org.super_admin)
    .bind(index::SPACE)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let latest = rows
        .into_iter()
        .filter_map(|(rev, v)| Some((rev, serde_json::from_str::<Value>(&v).ok()?)))
        .filter(|(_, v)| v.get("space").and_then(Value::as_str) == Some(space))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, v)| v);
    Ok(latest.unwrap_or_else(|| {
        json!({
            "space": space,
            "type": crate::spacehost::ADMIN_TYPE,
            "readPolicy": "members",
            "writePolicy": "members",
            "appAccess": "allowList",
            "allowList": [state.oauth.client_id_for("atproto")],
        })
    }))
}

async fn join_set(state: &AppState, args: &Args) -> Result<Done, String> {
    let (space, org) = conference(state, args).await?;
    let (acting, _) = acting(state, args, &org).await?;
    only_super_admin(&org, &acting, "change a conference's join methods")?;
    let methods: Vec<String> = args
        .need("methods")?
        .split(',')
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .collect();
    for method in &methods {
        if !["code", "request", "list", "open"].contains(&method.as_str()) {
            return Err(format!("unknown join method {method}: use code, request, list or open"));
        }
    }
    settings_snapshot(state, &acting, &org, &space.to_string(), |v| {
        v["join"] = json!({ "methods": methods })
    })
    .await?;
    after_change(state, &org).await?;
    done(
        format!(
            "Join methods: {}",
            if methods.is_empty() { "none".into() } else { methods.join(", ") }
        ),
        json!({ "methods": methods }),
    )
}

async fn codes_issue(state: &AppState, args: &Args) -> Result<Done, String> {
    let (space, org) = conference(state, args).await?;
    let (acting, role) = acting(state, args, &org).await?;
    only_owners(role, &acting, "issue codes")?;
    let (code, personal) = match (args.flag("shared"), args.flag("personal")) {
        (Some(code), None) => (code.trim().to_owned(), false),
        // At least 80 bits of randomness.
        (None, Some(_)) => (random_token(12), true),
        _ => return Err(format!("give --shared <code> or --personal\n\n{USAGE}")),
    };
    if code.is_empty()
        || code.len() > 128
        || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("a code is letters, digits, '-', '_' and '.', at most 128 of them".into());
    }
    let hash = state.secrets.code_hmac(&code);
    // A code names its conference, so it must be unique across all of them.
    if super::code_conference(state, &hash).await?.is_some() {
        return Err(format!("the code {code} is already in use"));
    }
    let mut record = json!({ "space": space.to_string(), "codeHash": hash, "personal": personal });
    if let Some(expires) = args.flag("expires") {
        index::parse_iso_ms(expires)
            .ok_or_else(|| format!("--expires isn't a date and time: {expires}"))?;
        record["expires"] = json!(expires);
    }
    if let Some(max) = args.flag("max-uses") {
        let max: u64 =
            max.parse().map_err(|_| format!("--max-uses must be a number, not {max}"))?;
        record["maxUses"] = json!(max);
    }
    acting
        .create_in(state, &SpaceUri::admin(&org.did).to_string(), index::CODE, None, record)
        .await?;
    done(format!("Code: {code}"), json!({ "codes": [code] }))
}

async fn list_import(state: &AppState, args: &Args) -> Result<Done, String> {
    let file = args.word(2, "the CSV file")?;
    let (space, org) = conference(state, args).await?;
    let (acting, role) = acting(state, args, &org).await?;
    only_owners(role, &acting, "import attendee lists")?;
    let csv = std::fs::read_to_string(file).map_err(|e| format!("couldn't read {file}: {e}"))?;
    let mut lines = csv_rows(&csv).into_iter();
    let header: Vec<String> =
        lines.next().ok_or("the list is empty")?.iter().map(|h| h.trim().to_lowercase()).collect();
    let column = |name: &str| header.iter().position(|h| h == name);
    let (handle_col, email_col, role_col) = (column("handle"), column("email"), column("role"));
    if handle_col.is_none() && email_col.is_none() {
        return Err("the list needs a handle or an email column".into());
    }
    let conference = org.conference(&space.to_string()).ok_or("no such conference")?;
    let admin_space = SpaceUri::admin(&org.did).to_string();
    let mut imported = 0;
    for cells in lines {
        let cell = |col: Option<usize>| {
            col.and_then(|i| cells.get(i)).map(|c| c.trim()).filter(|c| !c.is_empty())
        };
        let mut entry = json!({ "space": space.to_string() });
        let mut did = None;
        if let Some(handle) = cell(handle_col) {
            let handle = crate::identity::normalize_handle(handle)
                .ok_or_else(|| format!("{handle} isn't a handle"))?;
            // Resolved now, and bound to that DID: a handle that changes
            // hands later doesn't take the place with it.
            match state.resolver.resolve_handle(&handle).await {
                Ok(resolved) => {
                    entry["did"] = json!(resolved);
                    did = Some(resolved);
                }
                Err(_) => eprintln!(
                    "note: {handle} doesn't resolve yet; it's kept and tried again when someone joins"
                ),
            }
            entry["handle"] = json!(handle);
        }
        if let Some(email) = cell(email_col) {
            entry["emailHmac"] = json!(state.secrets.email_hmac(email));
        }
        let role = cell(role_col);
        if let Some(role) = role {
            if !ROLES.contains(&role) {
                return Err(format!("{role} isn't a role: use owner, staff or speaker"));
            }
            entry["role"] = json!(role);
        }
        if entry.get("did").is_none()
            && entry.get("handle").is_none()
            && entry.get("emailHmac").is_none()
        {
            continue;
        }
        acting.create_in(state, &admin_space, index::LIST_ENTRY, None, entry).await?;
        if let (Some(did), Some(role)) = (&did, role) {
            // A role from the list admits its subject only while the list
            // does, and only while the owner who imported it is one.
            let writer = conference_super_admin(state, &org, conference).await?;
            writer
                .put_in(
                    state,
                    &space.to_string(),
                    index::ROLE,
                    did,
                    json!({ "subject": did, "role": role, "assignedBy": acting.did, "via": "list", "since": index::iso(now_ms() as u64 * 1000) }),
                )
                .await?;
        }
        imported += 1;
    }
    done(format!("Imported {imported} attendees."), json!({ "imported": imported }))
}

async fn requests_list(state: &AppState, args: &Args) -> Result<Done, String> {
    let (space, org) = conference(state, args).await?;
    let conference = org.conference(&space.to_string()).ok_or("no such conference")?;
    let requests: Vec<Value> = conference
        .pending
        .iter()
        .map(|(did, us)| json!({ "did": did, "since": index::iso(*us) }))
        .collect();
    let text = if requests.is_empty() {
        "No requests waiting.".to_owned()
    } else {
        conference.pending.keys().cloned().collect::<Vec<_>>().join("\n")
    };
    done(text, json!({ "requests": requests }))
}

/// Approving, denying, admitting, removing and banning: an admin's decision,
/// written to their own repo.
async fn decide(state: &AppState, args: &Args, action: &str) -> Result<Done, String> {
    let who = args.word(2, "a handle")?;
    let subject = did_of(state, who).await?;
    let (space, org) = conference(state, args).await?;
    let (acting, role) = acting(state, args, &org).await?;
    let conference = org.conference(&space.to_string()).ok_or("no such conference")?;
    let admin_space = SpaceUri::admin(&org.did).to_string();
    let now = index::iso(now_ms() as u64 * 1000);
    let space_uri = space.to_string();
    let (collection, record, message) = match action {
        "approve" | "deny" if !conference.pending.contains_key(&subject) => {
            return Err(format!("{who} has no request waiting"));
        }
        "approve" => (
            index::MEMBER,
            json!({ "space": space_uri, "subject": subject, "via": "request", "since": now }),
            format!("{who} is in."),
        ),
        "deny" => (
            index::DENY,
            json!({ "space": space_uri, "subject": subject }),
            format!("{who} wasn't admitted."),
        ),
        "add" => (
            index::MEMBER,
            json!({ "space": space_uri, "subject": subject, "via": "admin", "since": now }),
            format!("{who} is in."),
        ),
        "remove" | "ban" if org.is_admin(&subject) => {
            return Err(format!(
                "{who} is an admin of {}: remove them as an admin with `org admin remove` instead",
                org.did
            ));
        }
        "remove"
            if role != Some(Role::Owner)
                && conference
                    .roles
                    .get(&subject)
                    .is_some_and(|r| matches!(r.as_str(), "owner" | "staff")) =>
        {
            return Err(format!(
                "only owners can remove someone with the owner or staff role; {} is staff",
                acting.handle
            ));
        }
        "remove" => (
            index::MEMBER,
            json!({ "space": space_uri, "subject": subject, "via": "removed", "until": now }),
            format!("{who} was removed."),
        ),
        "ban" => {
            only_owners(role, &acting, "ban people")?;
            (
                index::BAN,
                json!({ "space": space_uri, "subject": subject }),
                format!("{who} is banned."),
            )
        }
        _ => return Err(format!("unknown action {action}")),
    };
    acting.create_in(state, &admin_space, collection, None, record).await?;
    let mut after = after_change(state, &org).await?;
    // Another admin's decision doesn't undo the super admin's latest one.
    let mut member = after.conference(&space_uri).is_some_and(|c| c.is_member(&subject));
    // Out of the conference, out of their role: once they're out, so a
    // removal the super admin's decision overrides leaves the role be.
    if matches!(action, "remove" | "ban") && !member && conference.roles.contains_key(&subject) {
        super::keep_list_claim(state, &org, conference, &subject).await?;
        let writer = conference_super_admin(state, &org, conference).await?;
        writer.delete_in(state, &space_uri, index::ROLE, &subject).await?;
        after = after_change(state, &after).await?;
        member = after.conference(&space_uri).is_some_and(|c| c.is_member(&subject));
    }
    let now_in = after.conference(&space_uri);
    if matches!(action, "approve" | "add") && now_in.is_some_and(|c| c.banned.contains(&subject)) {
        return Err(format!("{who} is banned from this conference"));
    }
    if (matches!(action, "approve" | "add") && !member) || (action == "remove" && member) {
        return Err(format!(
            "{who}: the super admin's latest decision about them stands; only they can change it"
        ));
    }
    done(message, json!({ "did": subject, "action": action }))
}

async fn member_role(state: &AppState, args: &Args) -> Result<Done, String> {
    let who = args.word(2, "a handle")?;
    let subject = did_of(state, who).await?;
    let role = args.need("role")?;
    if !["owner", "staff", "speaker", "none"].contains(&role) {
        return Err(format!("--role must be owner, staff, speaker or none, not {role}"));
    }
    let (space, org) = conference(state, args).await?;
    let (acting, acting_role) = acting(state, args, &org).await?;
    let conference = org.conference(&space.to_string()).ok_or("no such conference")?;
    // Within the acting admin's role: owner and staff roles are given and
    // taken by owners; any admin can give or take the others.
    let privileged = |r: &str| matches!(r, "owner" | "staff");
    if acting_role != Some(Role::Owner)
        && (privileged(role) || conference.roles.get(&subject).is_some_and(|r| privileged(r)))
    {
        return Err(format!(
            "only owners can give or take the owner and staff roles; {} is staff",
            acting.handle
        ));
    }
    // Only the conference's super admin's role records count. Each names the
    // admin who decided it, and counts only while they could.
    let writer = conference_super_admin(state, &org, conference).await?;
    // A list row held by this role's claim stays theirs whatever the role
    // becomes.
    super::keep_list_claim(state, &org, conference, &subject).await?;
    if role == "none" {
        if conference.roles.contains_key(&subject) {
            writer.delete_in(state, &space.to_string(), index::ROLE, &subject).await?;
        }
    } else {
        writer
            .put_in(
                state,
                &space.to_string(),
                index::ROLE,
                &subject,
                json!({ "subject": subject, "role": role, "assignedBy": acting.did, "since": index::iso(now_ms() as u64 * 1000) }),
            )
            .await?;
    }
    after_change(state, &org).await?;
    done(format!("{who}'s role: {role}"), json!({ "did": subject, "role": role }))
}

async fn apps(state: &AppState, args: &Args, action: &str) -> Result<Done, String> {
    let (space, org) = match (args.flag("conference"), args.flag("org")) {
        (Some(_), _) => {
            let (space, org) = conference(state, args).await?;
            (space.to_string(), org)
        }
        (None, Some(did)) => {
            let org = org(state, did).await?;
            (SpaceUri::admin(&org.did).to_string(), org)
        }
        (None, None) => return Err(format!("give --conference <space> or --org <did>\n\n{USAGE}")),
    };
    let (acting, _) = acting(state, args, &org).await?;
    only_super_admin(&org, &acting, "change which apps can read a space")?;
    match action {
        "add" | "remove" => {
            let client = args.word(2, "the app's client ID")?.to_owned();
            url::Url::parse(&client).map_err(|_| format!("{client} isn't a client ID (a URL)"))?;
            let add = action == "add";
            let client_for_change = client.clone();
            settings_snapshot(state, &acting, &org, &space, move |v| {
                let mut list: Vec<String> = v
                    .get("allowList")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();
                list.retain(|c| c != &client_for_change);
                if add {
                    list.push(client_for_change);
                }
                v["allowList"] = json!(list);
            })
            .await?;
            after_change(state, &org).await?;
            done(
                if add {
                    format!("{client} can read {space}.")
                } else {
                    format!("{client} can no longer read {space}.")
                },
                json!({ "space": space, "client": client }),
            )
        }
        "open" => {
            if args.flag("yes").is_none() {
                return Err(format!(
                    "This lets any app a member uses read everything in {space}, not only the apps on its list.\n\
                     Run it again with --yes to go ahead."
                ));
            }
            settings_snapshot(state, &acting, &org, &space, |v| v["appAccess"] = json!("open"))
                .await?;
            done(
                format!("Any app a member uses can now read {space}."),
                json!({ "space": space, "appAccess": "open" }),
            )
        }
        _ => {
            settings_snapshot(state, &acting, &org, &space, |v| {
                v["appAccess"] = json!("allowList")
            })
            .await?;
            after_change(state, &org).await?;
            done(
                format!("Only the apps on its list can read {space}."),
                json!({ "space": space, "appAccess": "allowList" }),
            )
        }
    }
}

async fn rules_set(state: &AppState, args: &Args) -> Result<Done, String> {
    let file = args.word(2, "the rules file")?;
    let (space, org) = conference(state, args).await?;
    let (acting, role) = acting(state, args, &org).await?;
    only_owners(role, &acting, "set a conference's rules")?;
    let conference = org.conference(&space.to_string()).ok_or("no such conference")?;
    let rules: Value = serde_json::from_str(
        &std::fs::read_to_string(file).map_err(|e| format!("couldn't read {file}: {e}"))?,
    )
    .map_err(|e| format!("{file} isn't JSON: {e}"))?;
    let list = rules
        .get("rules")
        .and_then(Value::as_array)
        .ok_or("the rules file needs a \"rules\" array")?;
    for rule in list {
        let ok = rule.get("collection").and_then(Value::as_str).is_some()
            && matches!(rule.get("writers").and_then(Value::as_str), Some("admins" | "members"));
        if !ok {
            return Err(
                "each rule is {\"collection\": …, \"writers\": \"admins\" | \"members\"}".into()
            );
        }
    }
    let writer = conference_super_admin(state, &org, conference).await?;
    writer
        .put_in(state, &space.to_string(), index::RULES, "self", json!({ "rules": list }))
        .await?;
    done("Rules set.", json!({ "space": space.to_string() }))
}

/// A CSV file's rows (RFC 4180): fields separated by commas, optionally
/// quoted, with `""` for a quote inside quotes. Blank lines are skipped.
fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let mut end_row = |row: &mut Vec<String>, field: &mut String| {
        row.push(std::mem::take(field));
        let done = std::mem::take(row);
        if done.iter().any(|f| !f.trim().is_empty()) {
            rows.push(done);
        }
    };
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if field.trim().is_empty() => {
                field.clear();
                quoted = true;
            }
            (',', false) => row.push(std::mem::take(&mut field)),
            ('\r', false) if chars.peek() == Some(&'\n') => {}
            ('\n' | '\r', false) => end_row(&mut row, &mut field),
            (c, _) => field.push(c),
        }
    }
    end_row(&mut row, &mut field);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attendee_list_is_read_as_csv() {
        let rows = csv_rows(
            "handle,email,role\r\nana.test,\"Ana, de Vries\" <ana@example.com>,speaker\n\n\"bram.test\",,\n\"say \"\"hi\"\"\",x,\n",
        );
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0], ["handle", "email", "role"]);
        assert_eq!(rows[1], ["ana.test", "Ana, de Vries <ana@example.com>", "speaker"]);
        assert_eq!(rows[2], ["bram.test", "", ""]);
        assert_eq!(rows[3], ["say \"hi\"", "x", ""]);
    }
}
