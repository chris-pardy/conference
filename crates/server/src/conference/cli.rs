//! `conference-server admin …`: the operator's CLI. It runs with the server's
//! environment, against the same database, while the server runs. Every
//! action is written as an admin (`--as <handle>`, by default the super
//! admin) into their own repo, under the session they connected with, and
//! read back into the index before the command reports success.
//!
//! It exits non-zero with the reason on stderr when it refuses or fails. With
//! `--json`, its last line of stdout is one JSON object.

use std::collections::BTreeMap;
use std::process::ExitCode;
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

async fn org(state: &AppState, did: &str) -> Result<Org, String> {
    index::load(state, did).await?.ok_or_else(|| format!("{did} isn't an organization we host"))
}

/// A conference, by its space URI, with its organization.
async fn conference(state: &AppState, args: &Args) -> Result<(SpaceUri, Org), String> {
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
    let did = conference.super_admin().unwrap_or(&org.super_admin).to_owned();
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
async fn after_change(state: &AppState, before: &Org) -> Result<Org, String> {
    let after = org(state, &before.did).await?;
    super::revoke_lost(state, &super::readers(before), &after).await;
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
    for (did, admin) in &org.admins {
        let connected = connected(state, did).await?;
        lines.push(format!(
            "  {did} {} {}",
            admin.role.as_str(),
            if connected { "connected" } else { "not connected" }
        ));
        admins.push(json!({ "did": did, "role": admin.role.as_str(), "connected": connected }));
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
        acting
            .put_in(
                state,
                &admin_space,
                index::ADMIN,
                &subject,
                json!({ "subject": subject, "role": role.as_str() }),
            )
            .await?;
        // Admins are members of every conference, with their role.
        for conference in org.conferences.values() {
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
    let owners: Vec<&String> =
        org.admins.iter().filter(|(_, a)| a.role == Role::Owner).map(|(d, _)| d).collect();
    if owners.len() == 1 && owners[0] == &subject {
        return Err(format!("{who} is the last owner of {}: add another owner first", org.did));
    }
    if !org.admins.contains_key(&subject) {
        return Err(format!("{who} isn't an admin of {}", org.did));
    }
    acting.delete_in(state, &admin_space, index::ADMIN, &subject).await?;
    for conference in org.conferences.values() {
        if conference.roles.contains_key(&subject) {
            let writer = conference_super_admin(state, &org, conference).await?;
            writer.delete_in(state, conference.space(), index::ROLE, &subject).await?;
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
    let org = sync::reindex(state, did).await?;
    super::revoke_lost(state, &super::readers(&before), &org).await;
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
        Some(handle) => Acting::new(state, &did_of(state, handle).await?, handle).await?,
        None => Acting::new(state, &org.super_admin, &org_admin.handle).await?,
    };
    let invite_only = args.flag("invite-only").is_some();
    let theme: Option<Value> = match args.flag("theme") {
        Some(theme) => {
            Some(serde_json::from_str(theme).map_err(|e| format!("--theme isn't JSON: {e}"))?)
        }
        None => None,
    };
    let skey = tid_now();
    let space = SpaceUri::new(&org.did, CONFERENCE_TYPE, &skey).to_string();
    let intake = SpaceUri::new(&org.did, INTAKE_TYPE, &skey).to_string();

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

    // Its settings first, so the space is ours before anything is written into it.
    let admin_space = SpaceUri::admin(&org.did).to_string();
    let eventside = state.oauth.client_id_for("atproto");
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
        "visibility": if invite_only { "inviteOnly" } else { "public" },
    });

    // The entry point: public records in the super admin's repo, or inside the space.
    let mut sidecar = json!({
        "space": space,
        "intake": intake,
        "organization": org.did,
        "superAdmin": super_admin.did,
        "visibility": if invite_only { "inviteOnly" } else { "public" },
    });
    if let Some(theme) = &theme {
        sidecar["theme"] = theme.clone();
    }
    let event_uri = if invite_only {
        None
    } else {
        let uri = match &adopted {
            Some(uri) => uri.clone(),
            None => super_admin.create_public(state, EVENT, None, event_value.clone()).await?,
        };
        let rkey = uri.rsplit('/').next().unwrap_or_default().to_owned();
        sidecar["event"] = json!(uri);
        super_admin.create_public(state, SIDECAR, Some(&rkey), sidecar.clone()).await?;
        settings["event"] = json!(uri);
        Some(uri)
    };
    org_admin.create_in(state, &admin_space, index::SPACE, None, settings).await?;
    if invite_only {
        let rkey = tid_now();
        let event_uri = format!("{space}/{}/{EVENT}/{rkey}", super_admin.did);
        super_admin.create_in(state, &space, EVENT, Some(&rkey), event_value.clone()).await?;
        sidecar["event"] = json!(event_uri);
        super_admin.put_in(state, &space, SIDECAR, "self", sidecar).await?;
    }

    // Roles for the organization's admins, and the rules.
    for (did, admin) in &org.admins {
        super_admin
            .put_in(
                state,
                &space,
                index::ROLE,
                did,
                json!({ "subject": did, "role": admin.role.as_str() }),
            )
            .await?;
    }
    let rules: Vec<Value> = index::default_rules()
        .into_iter()
        .map(|(collection, writers)| json!({ "collection": collection, "writers": writers }))
        .collect();
    super_admin.put_in(state, &space, index::RULES, "self", json!({ "rules": rules })).await?;

    super::save(
        state,
        &Row {
            space: space.clone(),
            org: org.did.clone(),
            intake: intake.clone(),
            super_admin: super_admin.did.clone(),
            event: event_uri.clone(),
            invite_only: i64::from(invite_only),
            info: super::info_from_event(&event_value, theme.as_ref()).to_string(),
        },
    )
    .await?;
    let name = event_value.get("name").and_then(Value::as_str).unwrap_or("The conference");
    let mut out = json!({ "space": space, "intake": intake });
    if let Some(event) = &event_uri {
        out["event"] = json!(event);
    }
    done(format!("Created {name}: {space}"), out)
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
    for authority in authority::all(&state.db).await? {
        if let Some(other) = index::load(state, &authority.did).await?
            && other.conferences.values().any(|c| c.has_code(&hash))
        {
            return Err(format!("the code {code} is already in use"));
        }
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
    let mut lines = csv.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<String> = lines
        .next()
        .ok_or("the list is empty")?
        .split(',')
        .map(|h| h.trim().to_lowercase())
        .collect();
    let column = |name: &str| header.iter().position(|h| h == name);
    let (handle_col, email_col, role_col) = (column("handle"), column("email"), column("role"));
    if handle_col.is_none() && email_col.is_none() {
        return Err("the list needs a handle or an email column".into());
    }
    let conference = org.conference(&space.to_string()).ok_or("no such conference")?;
    let admin_space = SpaceUri::admin(&org.did).to_string();
    let mut imported = 0;
    for line in lines {
        let cells: Vec<&str> = line.split(',').map(str::trim).collect();
        let cell =
            |col: Option<usize>| col.and_then(|i| cells.get(i)).copied().filter(|c| !c.is_empty());
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
            let writer = conference_super_admin(state, &org, conference).await?;
            writer
                .put_in(
                    state,
                    &space.to_string(),
                    index::ROLE,
                    did,
                    json!({ "subject": did, "role": role }),
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
    // Out of the conference, out of their role.
    if matches!(action, "remove" | "ban") && conference.roles.contains_key(&subject) {
        let writer = conference_super_admin(state, &org, conference).await?;
        writer.delete_in(state, &space_uri, index::ROLE, &subject).await?;
    }
    after_change(state, &org).await?;
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
    let (acting, _) = acting(state, args, &org).await?;
    let conference = org.conference(&space.to_string()).ok_or("no such conference")?;
    // Only the conference's super admin's role records count.
    let writer = conference_super_admin(state, &org, conference).await?;
    let _ = acting;
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
                json!({ "subject": subject, "role": role }),
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
