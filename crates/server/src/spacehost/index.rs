//! The index: permissions derived from records. Our database keeps a copy of
//! the records in an organization's admin space and intake spaces (and the
//! role and rules records in its conference spaces), each with the revision
//! of the commit that wrote it. Everything a host check or a feature asks
//! (who's an admin, a space's policies and app access, who's a member and
//! when, who's banned, which requests are waiting) is derived from them here,
//! the same way any app that can read those spaces could.
//!
//! The precedence rules:
//! - The organization is crawled from its super admin. Only the super
//!   admin's `admin` records name admins, and only their `space` records set
//!   a space's policies, app access and join methods.
//! - A record counts only while its author is an admin, and only within
//!   their role: staff can admit, remove and deny; owners can also ban and
//!   issue codes and lists.
//! - The super admin's ban always stands. Among other admins, a ban beats an
//!   admission; otherwise the latest decision stands.
//! - Joining and leaving are the person's own records in the intake space.
//!   A join counts as an admission if a rule in force at its commit admits
//!   it (a pre-assigned role, the attendee list, a valid code, an open
//!   conference); otherwise it's a request, if requests were on.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use serde_json::{Value, json};

use super::authority::Secrets;
use super::{ADMIN_TYPE, CONFERENCE_TYPE, INTAKE_TYPE, SpaceUri};
use crate::AppState;
use crate::crypto::tid_micros;

pub const ADMIN: &str = "app.eventside.admin.admin";
pub const SPACE: &str = "app.eventside.admin.space";
pub const MEMBER: &str = "app.eventside.admin.member";
pub const BAN: &str = "app.eventside.admin.ban";
pub const DENY: &str = "app.eventside.admin.deny";
pub const CODE: &str = "app.eventside.admin.code";
pub const LIST_ENTRY: &str = "app.eventside.admin.listEntry";
pub const JOIN: &str = "app.eventside.intake.join";
pub const LEAVE: &str = "app.eventside.intake.leave";
pub const ROLE: &str = "app.eventside.conference.role";
pub const RULES: &str = "app.eventside.conference.rules";

/// A record from the index.
#[derive(Debug, Clone)]
pub struct Rec {
    pub space: String,
    pub repo: String,
    pub collection: String,
    pub rkey: String,
    pub rev: String,
    /// When its commit was made, in microseconds (from `rev`).
    pub us: u64,
    /// When our host first saw this version of it, in microseconds.
    pub seen_us: Option<u64>,
    pub value: Value,
}

impl Rec {
    fn str(&self, field: &str) -> Option<&str> {
        self.value.get(field).and_then(Value::as_str)
    }

    /// When a record in an intake space counts from: its commit, but never
    /// before our host first saw it. Anyone can write there, from any PDS,
    /// and a PDS chooses its own revisions, so a backdated join can't jump
    /// ahead of others, or back to before a code expired.
    fn intake_us(&self) -> u64 {
        self.seen_us.map_or(self.us, |seen| self.us.max(seen))
    }
}

/// How far before our host first saw a record in a conference space its
/// commit may be dated: enough for a write notification's ordinary delay.
pub const BACKDATE_SLACK_US: u64 = 60_000_000;

/// When a record in a conference space counts from: its commit, but never
/// more than [`BACKDATE_SLACK_US`] before our host first saw it. A writer's
/// PDS chooses its own revisions, so without this someone removed could date
/// a record back into a period when they were a member.
pub fn conference_us(us: u64, seen_us: Option<u64>) -> u64 {
    seen_us.map_or(us, |seen| us.max(seen.saturating_sub(BACKDATE_SLACK_US)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Staff,
}

impl Role {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(Self::Owner),
            "staff" => Some(Self::Staff),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Staff => "staff",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Admin {
    pub role: Role,
    pub since_us: u64,
}

/// Which apps may read a space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAccess {
    Open,
    AllowList(Vec<String>),
}

impl AppAccess {
    /// Whether a client may read: any, when open; else one on the list.
    /// Eventside itself is on a list by its metadata URL, whatever scope
    /// list its client ID carries.
    pub fn allows(&self, client_id: Option<&str>, eventside: &str) -> bool {
        match self {
            Self::Open => true,
            Self::AllowList(allowed) => client_id.is_some_and(|id| {
                allowed.iter().any(|a| {
                    a == id || {
                        let ours = format!("{eventside}/oauth-client-metadata.json");
                        a.split('?').next() == Some(ours.as_str())
                            && id.split('?').next() == Some(ours.as_str())
                    }
                })
            }),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::Open => json!({ "$type": "com.atproto.simplespace.defs#open" }),
            Self::AllowList(allowed) => {
                json!({ "$type": "com.atproto.simplespace.defs#allowList", "allowed": allowed })
            }
        }
    }
}

/// A space's settings, from one of the super admin's `space` records.
#[derive(Debug, Clone)]
pub struct Settings {
    pub space: String,
    pub kind: String,
    pub app_access: AppAccess,
    pub methods: BTreeSet<String>,
    pub intake: Option<String>,
    pub event: Option<String>,
    pub super_admin: Option<String>,
    pub invite_only: bool,
    pub us: u64,
}

impl Settings {
    fn from_record(rec: &Rec) -> Option<Self> {
        let space = rec.str("space")?.to_owned();
        let parsed = SpaceUri::parse(&space)?;
        let allowed: Vec<String> = rec
            .value
            .get("allowList")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let app_access = if rec.str("appAccess") == Some("open") {
            AppAccess::Open
        } else {
            AppAccess::AllowList(allowed)
        };
        let methods = rec
            .value
            .get("join")
            .and_then(|j| j.get("methods"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        Some(Self {
            kind: parsed.kind,
            space,
            app_access,
            methods,
            intake: rec.str("intake").map(str::to_owned),
            event: rec.str("event").map(str::to_owned),
            super_admin: rec.str("superAdmin").map(str::to_owned),
            invite_only: rec.str("visibility") == Some("inviteOnly"),
            us: rec.us,
        })
    }

    /// An admin space before its super admin has written anything:
    /// eventside alone may read it.
    fn default_admin(org: &str, eventside_client: &str) -> Self {
        Self {
            space: SpaceUri::admin(org).to_string(),
            kind: ADMIN_TYPE.to_owned(),
            app_access: AppAccess::AllowList(vec![eventside_client.to_owned()]),
            methods: BTreeSet::new(),
            intake: None,
            event: None,
            super_admin: None,
            invite_only: false,
            us: 0,
        }
    }

    pub fn has(&self, method: &str) -> bool {
        self.methods.contains(method)
    }
}

/// A membership period, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub since: u64,
    pub until: Option<u64>,
}

impl Period {
    pub fn covers(&self, us: u64) -> bool {
        self.since <= us && self.until.is_none_or(|until| us < until)
    }

    pub fn to_json(&self) -> Value {
        let mut out = json!({ "since": iso(self.since) });
        if let Some(until) = self.until {
            out["until"] = json!(iso(until));
        }
        out
    }
}

/// Microseconds as an ISO 8601 timestamp, to the millisecond.
pub fn iso(us: u64) -> String {
    let ms = (us / 1000) as i64;
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60,
        ms.rem_euclid(1000)
    )
}

#[derive(Debug, Clone)]
struct Code {
    us: u64,
    personal: bool,
    expires_us: Option<u64>,
    max_uses: Option<u64>,
}

#[derive(Debug, Clone, Default)]
struct CodeUse {
    count: u64,
    bound: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListEntry {
    /// The owner who imported it (or, for one bound when its handle first
    /// resolved, the owner who imported the handle).
    pub by: String,
    pub did: Option<String>,
    pub handle: Option<String>,
    pub email_hmac: Option<String>,
    pub role: Option<String>,
}

/// How a join was admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Role,
    List,
    Code,
    Open,
}

/// A conference, as its records make it.
#[derive(Debug, Clone)]
pub struct Conference {
    /// Its settings now.
    pub settings: Settings,
    history: Vec<Settings>,
    codes: BTreeMap<String, Code>,
    pub list: Vec<ListEntry>,
    /// The conference's super admin: who writes its roles and rules.
    pub super_admin_did: String,
    /// Roles, from the conference super admin's role records, each counting
    /// only while the admin who assigned it could.
    pub roles: BTreeMap<String, String>,
    /// Roles assigned by an attendee list import, which admit their subject
    /// only while the list does.
    list_roles: BTreeSet<String>,
    /// Who assigned each role, when another admin did.
    pub role_deciders: BTreeMap<String, String>,
    /// The rules, from the conference super admin's rules record.
    pub rules: Option<Value>,
    /// Membership periods by DID, past members included.
    pub members: BTreeMap<String, Vec<Period>>,
    pub banned: BTreeSet<String>,
    /// Requests waiting for an admin, with when they were made.
    pub pending: BTreeMap<String, u64>,
    /// People whose latest request was denied.
    pub denied: BTreeSet<String>,
    uses: HashMap<String, CodeUse>,
}

impl Conference {
    pub fn space(&self) -> &str {
        &self.settings.space
    }

    pub fn intake(&self) -> Option<&str> {
        self.settings.intake.as_deref()
    }

    /// The conference's super admin: the one its settings name, else the
    /// organization's.
    pub fn super_admin(&self) -> &str {
        &self.super_admin_did
    }

    pub fn is_member(&self, did: &str) -> bool {
        self.members.get(did).is_some_and(|periods| periods.iter().any(|p| p.until.is_none()))
    }

    /// Whether `did` was a member at `us`.
    pub fn was_member_at(&self, did: &str, us: u64) -> bool {
        self.members.get(did).is_some_and(|periods| periods.iter().any(|p| p.covers(us)))
    }

    pub fn current_members(&self) -> impl Iterator<Item = &String> {
        self.members.iter().filter(|(_, p)| p.iter().any(|p| p.until.is_none())).map(|(did, _)| did)
    }

    pub fn has_email_rows(&self) -> bool {
        self.list.iter().any(|e| e.email_hmac.is_some())
    }

    pub fn on_list(&self, did: &str) -> bool {
        self.list.iter().any(|e| e.did.as_deref() == Some(did))
    }

    /// List entries with only a handle that didn't resolve when imported,
    /// and hasn't been bound to a DID since (no entry with a DID has the
    /// same handle).
    pub fn unbound_handles(&self) -> impl Iterator<Item = &ListEntry> {
        self.list.iter().filter(|e| {
            e.did.is_none()
                && e.handle.as_deref().is_some_and(|handle| {
                    !self
                        .list
                        .iter()
                        .any(|b| b.did.is_some() && b.handle.as_deref() == Some(handle))
                })
        })
    }

    /// Whether a code (by its HMAC) is one of this conference's.
    pub fn has_code(&self, code_hmac: &str) -> bool {
        self.codes.contains_key(code_hmac)
    }

    fn settings_at(&self, us: u64) -> &Settings {
        self.history
            .iter()
            .rev()
            .find(|s| s.us <= us)
            .or(self.history.first())
            .unwrap_or(&self.settings)
    }

    /// Whether a code admits `did` at `us`, counting the uses so far.
    fn code_admits(&self, uses: &HashMap<String, CodeUse>, hash: &str, did: &str, us: u64) -> bool {
        let Some(code) = self.codes.get(hash) else { return false };
        let used = uses.get(hash).cloned().unwrap_or_default();
        code.us <= us
            && code.expires_us.is_none_or(|exp| us < exp)
            && if code.personal {
                used.bound.as_deref().is_none_or(|bound| bound == did)
            } else {
                code.max_uses.is_none_or(|max| used.count < max)
            }
    }

    /// The rule in force at `us` that admits a join by `did`, if any.
    fn admission(
        &self,
        uses: &HashMap<String, CodeUse>,
        did: &str,
        code_hash: Option<&str>,
        us: u64,
    ) -> Option<Via> {
        let settings = self.settings_at(us);
        if self.roles.contains_key(did) && (!self.list_roles.contains(did) || settings.has("list"))
        {
            Some(Via::Role)
        } else if settings.has("list") && self.on_list(did) {
            Some(Via::List)
        } else if settings.has("code")
            && code_hash.is_some_and(|h| self.code_admits(uses, h, did, us))
        {
            Some(Via::Code)
        } else if settings.has("open") {
            Some(Via::Open)
        } else {
            None
        }
    }

    /// What a join by `did` now would come to: admitted (and how), or not.
    /// Codes are checked against their uses so far.
    pub fn would_admit(&self, did: &str, code_hash: Option<&str>, us: u64) -> Option<Via> {
        self.admission(&self.uses, did, code_hash, us)
    }

    /// Whether a code admits `did` now.
    pub fn code_valid(&self, code_hash: &str, did: &str, us: u64) -> bool {
        self.settings.has("code") && self.code_admits(&self.uses, code_hash, did, us)
    }

    /// The role `did` has in the conference: the super admin's role record,
    /// else their admin role.
    pub fn role_of(&self, org: &Org, did: &str) -> Option<String> {
        self.roles.get(did).cloned().or_else(|| org.admin_role(did).map(|r| r.as_str().to_owned()))
    }

    /// Who may write a collection, by the rules: `admins` or `members`.
    pub fn writers_of(&self, collection: &str) -> &str {
        let from_rules =
            self.rules.as_ref().and_then(|r| r.get("rules")).and_then(Value::as_array).and_then(
                |rules| {
                    rules.iter().find_map(|r| {
                        (r.get("collection")?.as_str()? == collection)
                            .then(|| r.get("writers")?.as_str())
                            .flatten()
                    })
                },
            );
        match from_rules {
            Some("admins") => "admins",
            Some(_) => "members",
            None => default_writers(collection),
        }
    }
}

/// The rules a conference starts with: cards and announcements only by
/// owners and staff; plans, chat and anything else by any member.
pub fn default_rules() -> Vec<(&'static str, &'static str)> {
    vec![
        ("app.eventside.block.card", "admins"),
        ("app.eventside.conference.announcement", "admins"),
        ("community.lexicon.calendar.event", "members"),
        ("app.eventside.chat.message", "members"),
    ]
}

fn default_writers(collection: &str) -> &'static str {
    default_rules().into_iter().find(|(c, _)| *c == collection).map_or("members", |(_, w)| w)
}

/// An organization's permissions, derived from its records.
#[derive(Debug, Clone)]
pub struct Org {
    pub did: String,
    pub super_admin: String,
    pub created_us: u64,
    /// The admins the super admin's `admin` records name.
    pub admins: BTreeMap<String, Admin>,
    pub admin_settings: Settings,
    pub conferences: BTreeMap<String, Conference>,
}

impl Org {
    /// An admin's role. The super admin counts as an owner.
    pub fn admin_role(&self, did: &str) -> Option<Role> {
        match self.admins.get(did) {
            Some(admin) => Some(admin.role),
            None if did == self.super_admin => Some(Role::Owner),
            None => None,
        }
    }

    pub fn is_admin(&self, did: &str) -> bool {
        self.admin_role(did).is_some()
    }

    pub fn conference(&self, space: &str) -> Option<&Conference> {
        self.conferences.get(space)
    }

    /// The conference whose intake space this is.
    pub fn conference_of_intake(&self, intake: &str) -> Option<&Conference> {
        self.conferences.values().find(|c| c.intake() == Some(intake))
    }

    /// Whether this organization has the space.
    pub fn knows(&self, space: &SpaceUri) -> bool {
        let uri = space.to_string();
        space.authority == self.did
            && match space.kind.as_str() {
                ADMIN_TYPE => space.skey == "self",
                CONFERENCE_TYPE => self.conferences.contains_key(&uri),
                INTAKE_TYPE => self.conference_of_intake(&uri).is_some(),
                _ => false,
            }
    }

    /// Which apps may read a space. Intake spaces share the admin space's.
    pub fn app_access(&self, space: &SpaceUri) -> AppAccess {
        match space.kind.as_str() {
            CONFERENCE_TYPE => self
                .conferences
                .get(&space.to_string())
                .map_or(AppAccess::AllowList(vec![]), |c| c.settings.app_access.clone()),
            _ => self.admin_settings.app_access.clone(),
        }
    }

    /// Who may read a space: its members (admins, for the admin and intake spaces).
    pub fn can_read(&self, space: &SpaceUri, did: &str) -> bool {
        match space.kind.as_str() {
            CONFERENCE_TYPE => {
                self.conferences.get(&space.to_string()).is_some_and(|c| c.is_member(did))
            }
            ADMIN_TYPE | INTAKE_TYPE => self.is_admin(did),
            _ => false,
        }
    }

    /// Whose writes enter a space: members, admins, or (intake) anyone.
    pub fn can_write(&self, space: &SpaceUri, did: &str) -> bool {
        match space.kind.as_str() {
            CONFERENCE_TYPE => {
                self.conferences.get(&space.to_string()).is_some_and(|c| c.is_member(did))
            }
            ADMIN_TYPE => self.is_admin(did),
            INTAKE_TYPE => true,
            _ => false,
        }
    }

    /// The policies of a space, in the simplespace vocabulary.
    pub fn policies(&self, space: &SpaceUri) -> (Value, Value) {
        let members = json!({ "$type": "com.atproto.simplespace.defs#memberListPolicy" });
        let public = json!({ "$type": "com.atproto.simplespace.defs#publicPolicy" });
        match space.kind.as_str() {
            INTAKE_TYPE => (members, public),
            _ => (members.clone(), members),
        }
    }

    /// Everyone who may read a space now.
    pub fn readers(&self, space: &str) -> BTreeSet<String> {
        let Some(parsed) = SpaceUri::parse(space) else { return BTreeSet::new() };
        match parsed.kind.as_str() {
            CONFERENCE_TYPE => self
                .conferences
                .get(space)
                .map(|c| c.current_members().cloned().collect())
                .unwrap_or_default(),
            _ => self.admins.keys().cloned().chain([self.super_admin.clone()]).collect(),
        }
    }

    /// The spaces this organization hosts: its admin space, each conference
    /// space, and each intake space.
    pub fn spaces(&self) -> Vec<String> {
        let mut spaces = vec![SpaceUri::admin(&self.did).to_string()];
        for conference in self.conferences.values() {
            spaces.push(conference.space().to_owned());
            if let Some(intake) = conference.intake() {
                spaces.push(intake.to_owned());
            }
        }
        spaces
    }
}

/// The index's records for an organization: its admin and intake spaces,
/// and the role and rules records in its conference spaces, each with when
/// our host first saw it.
pub async fn records(state: &AppState, org: &str) -> Result<Vec<Rec>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, Option<i64>)>(
        "SELECT r.space, r.repo, r.collection, r.rkey, r.rev, r.value, s.seen_at FROM space_records r \
         LEFT JOIN space_record_seen s ON s.space = r.space AND s.repo = r.repo \
         AND s.collection = r.collection AND s.rkey = r.rkey AND s.rev = r.rev \
         WHERE r.value IS NOT NULL AND \
         (r.space = $1 OR r.space LIKE $2 OR (r.space LIKE $3 AND r.collection IN ($4, $5)))",
    )
    .bind(SpaceUri::admin(org).to_string())
    .bind(format!("at://{org}/space/{INTAKE_TYPE}/%"))
    .bind(format!("at://{org}/space/{CONFERENCE_TYPE}/%"))
    .bind(ROLE)
    .bind(RULES)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("could not read the index: {e}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|(space, repo, collection, rkey, rev, value, seen_at)| {
            Some(Rec {
                us: tid_micros(&rev)?,
                seen_us: seen_at.map(|ms| ms.max(0) as u64 * 1000),
                value: serde_json::from_str(&value).ok()?,
                space,
                repo,
                collection,
                rkey,
                rev,
            })
        })
        .collect())
}

/// How many times an organization's index has changed. Every write to the
/// index bumps it, from the server or the admin CLI (another process), so a
/// derived view is reused exactly until the records under it change.
pub async fn generation(state: &AppState, org: &str) -> Result<i64, String> {
    let generation =
        sqlx::query_scalar::<_, i64>("SELECT generation FROM index_generations WHERE org = $1")
            .bind(org)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| format!("could not read the index: {e}"))?;
    Ok(generation.unwrap_or(0))
}

/// Records that an organization's index changed. Called after the change is
/// written, so a view cached under an older generation is never reused.
pub async fn bump(state: &AppState, org: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO index_generations (org, generation) VALUES ($1, 1) \
         ON CONFLICT (org) DO UPDATE SET generation = index_generations.generation + 1",
    )
    .bind(org)
    .execute(&state.db)
    .await
    .map_err(|e| format!("could not update the index: {e}"))?;
    Ok(())
}

/// An organization's permissions, from the index: derived once per change
/// to its records, and shared until the next.
pub async fn load(state: &AppState, org: &str) -> Result<Option<Arc<Org>>, String> {
    // Read before the records: a change landing in between is then seen as
    // newer than what's cached, never older.
    let generation = generation(state, org).await?;
    if let Some(cached) = state.host.cached_org(org, generation) {
        return Ok(Some(cached));
    }
    let Some(authority) = super::authority::get(&state.db, org).await? else { return Ok(None) };
    let recs = records(state, org).await?;
    let eventside = state.oauth.client_id_for("atproto");
    let derived = Arc::new(derive(
        org,
        &authority.super_admin,
        authority.created_at as u64 * 1000,
        &recs,
        &state.secrets,
        &eventside,
    ));
    state.host.cache_org(org, generation, derived.clone());
    Ok(Some(derived))
}

/// The organization a space belongs to.
pub async fn load_for_space(
    state: &AppState,
    space: &SpaceUri,
) -> Result<Option<Arc<Org>>, String> {
    load(state, &space.authority).await
}

/// Derives an organization's permissions from its records.
pub fn derive(
    org: &str,
    super_admin: &str,
    created_us: u64,
    recs: &[Rec],
    secrets: &Secrets,
    eventside_client: &str,
) -> Org {
    let admin_space = SpaceUri::admin(org).to_string();
    let in_admin = |r: &&Rec| r.space == admin_space;

    // Admins, as the super admin's records name them.
    let mut admins = BTreeMap::new();
    for rec in
        recs.iter().filter(in_admin).filter(|r| r.repo == super_admin && r.collection == ADMIN)
    {
        if let (Some(subject), Some(role)) =
            (rec.str("subject"), rec.str("role").and_then(Role::parse))
        {
            admins.insert(subject.to_owned(), Admin { role, since_us: rec.us });
        }
    }
    let role_of = |did: &str| match admins.get(did) {
        Some(admin) => Some(admin.role),
        None if did == super_admin => Some(Role::Owner),
        None => None,
    };

    // Space settings: the super admin's snapshots, latest last.
    let mut snapshots: BTreeMap<String, Vec<Settings>> = BTreeMap::new();
    let mut settings_recs: Vec<&Rec> = recs
        .iter()
        .filter(in_admin)
        .filter(|r| r.repo == super_admin && r.collection == SPACE)
        .collect();
    settings_recs.sort_by(|a, b| a.us.cmp(&b.us).then_with(|| a.rkey.cmp(&b.rkey)));
    for rec in settings_recs {
        if let Some(settings) = Settings::from_record(rec)
            && SpaceUri::parse(&settings.space).is_some_and(|s| s.authority == org)
        {
            snapshots.entry(settings.space.clone()).or_default().push(settings);
        }
    }
    let admin_settings = snapshots
        .get(&admin_space)
        .and_then(|h| h.last().cloned())
        .unwrap_or_else(|| Settings::default_admin(org, eventside_client));

    // Admin decisions, from current admins, within their roles.
    let decisions: Vec<&Rec> = recs
        .iter()
        .filter(in_admin)
        .filter(|r| match (r.collection.as_str(), role_of(&r.repo)) {
            (MEMBER | DENY, Some(_)) | (BAN | CODE, Some(Role::Owner)) => true,
            // A list entry bound for an owner who imported its handle counts
            // only while they're still one.
            (LIST_ENTRY, Some(Role::Owner)) => {
                r.str("onBehalfOf").is_none_or(|owner| role_of(owner) == Some(Role::Owner))
            }
            _ => false,
        })
        .collect();

    let mut conferences = BTreeMap::new();
    for (space, history) in snapshots {
        let Some(current) = history.last().cloned() else { continue };
        if current.kind != CONFERENCE_TYPE {
            continue;
        }
        let conference = derive_conference(
            current,
            history,
            &decisions,
            recs,
            super_admin,
            &admins,
            created_us,
            secrets,
        );
        conferences.insert(space, conference);
    }

    Org {
        did: org.to_owned(),
        super_admin: super_admin.to_owned(),
        created_us,
        admins,
        admin_settings,
        conferences,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Admit,
    Remove,
    Leave,
    Ban,
}

#[allow(clippy::too_many_arguments)]
fn derive_conference(
    settings: Settings,
    history: Vec<Settings>,
    decisions: &[&Rec],
    recs: &[Rec],
    super_admin: &str,
    admins: &BTreeMap<String, Admin>,
    created_us: u64,
    secrets: &Secrets,
) -> Conference {
    let space = settings.space.clone();
    let about = |r: &&&Rec| r.str("space") == Some(space.as_str());
    let conference_super = settings.super_admin.clone().unwrap_or_else(|| super_admin.to_owned());

    let mut codes = BTreeMap::new();
    let mut list = Vec::new();
    for rec in decisions.iter().filter(about) {
        match rec.collection.as_str() {
            CODE => {
                if let Some(hash) = rec.str("codeHash") {
                    codes.insert(
                        hash.to_owned(),
                        Code {
                            us: rec.us,
                            personal: rec
                                .value
                                .get("personal")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            expires_us: rec.str("expires").and_then(parse_iso_us),
                            max_uses: rec.value.get("maxUses").and_then(Value::as_u64),
                        },
                    );
                }
            }
            LIST_ENTRY => list.push(ListEntry {
                by: rec.str("onBehalfOf").unwrap_or(&rec.repo).to_owned(),
                did: rec.str("did").map(str::to_owned),
                handle: rec.str("handle").map(str::to_owned),
                email_hmac: rec.str("emailHmac").map(str::to_owned),
                role: rec.str("role").map(str::to_owned),
            }),
            _ => {}
        }
    }

    // Roles and rules: only from the conference's super admin, in its space,
    // and only while they're an admin. A role written for another admin
    // (`assignedBy`) counts only while that admin may assign it: owner and
    // staff roles by owners, others by any admin. That's what lets a role
    // admit the person it names.
    let super_counts = conference_super == super_admin || admins.contains_key(&conference_super);
    let may_assign = |decider: &str, role: &str| {
        if decider == super_admin || (super_counts && decider == conference_super) {
            return true;
        }
        match admins.get(decider).map(|a| a.role) {
            Some(Role::Owner) => true,
            Some(Role::Staff) => !matches!(role, "owner" | "staff"),
            None => false,
        }
    };
    let mut roles = BTreeMap::new();
    let mut list_roles = BTreeSet::new();
    let mut role_deciders = BTreeMap::new();
    let mut rules = None;
    for rec in
        recs.iter().filter(|r| super_counts && r.space == space && r.repo == conference_super)
    {
        match rec.collection.as_str() {
            ROLE => {
                if let (Some(subject), Some(role)) = (rec.str("subject"), rec.str("role"))
                    && may_assign(rec.str("assignedBy").unwrap_or(&conference_super), role)
                {
                    roles.insert(subject.to_owned(), role.to_owned());
                    if let Some(decider) = rec.str("assignedBy") {
                        role_deciders.insert(subject.to_owned(), decider.to_owned());
                    }
                    if rec.str("via") == Some("list") {
                        list_roles.insert(subject.to_owned());
                    }
                }
            }
            RULES if rec.rkey == "self" => rules = Some(rec.value.clone()),
            _ => {}
        }
    }

    let mut conference = Conference {
        settings,
        history,
        codes,
        list,
        super_admin_did: conference_super.clone(),
        roles,
        list_roles,
        role_deciders,
        rules,
        members: BTreeMap::new(),
        banned: BTreeSet::new(),
        pending: BTreeMap::new(),
        denied: BTreeSet::new(),
        uses: HashMap::new(),
    };

    // Bans: the super admin's always stand; another owner's stands unless the
    // super admin admitted the person after it.
    let mut events: BTreeMap<String, Vec<(u64, Event)>> = BTreeMap::new();
    let mut ban_at: BTreeMap<String, u64> = BTreeMap::new();
    let mut denied_at: BTreeMap<String, u64> = BTreeMap::new();
    let mut decided_at: BTreeMap<String, u64> = BTreeMap::new();
    let super_admits: Vec<(&str, u64)> = decisions
        .iter()
        .filter(about)
        .filter(|r| {
            r.repo == super_admin && r.collection == MEMBER && r.value.get("until").is_none()
        })
        .filter_map(|r| Some((r.str("subject")?, r.us)))
        .collect();
    // Admins can't be banned: they're members because they're admins, and
    // the super admin's say always stands.
    let is_admin = |did: &str| did == super_admin || admins.contains_key(did);
    for rec in decisions.iter().filter(about) {
        let Some(subject) = rec.str("subject") else { continue };
        match rec.collection.as_str() {
            BAN if is_admin(subject) => {}
            BAN => {
                let overridden = rec.repo != super_admin
                    && super_admits.iter().any(|(s, us)| *s == subject && *us > rec.us);
                if !overridden {
                    let at = ban_at.entry(subject.to_owned()).or_insert(rec.us);
                    *at = (*at).min(rec.us);
                }
            }
            MEMBER => {
                let event =
                    if rec.value.get("until").is_some() { Event::Remove } else { Event::Admit };
                events.entry(subject.to_owned()).or_default().push((rec.us, event));
                let at = decided_at.entry(subject.to_owned()).or_default();
                *at = (*at).max(rec.us);
            }
            DENY => {
                let at = denied_at.entry(subject.to_owned()).or_default();
                *at = (*at).max(rec.us);
            }
            _ => {}
        }
    }
    for (subject, at) in &ban_at {
        events.entry(subject.clone()).or_default().push((*at, Event::Ban));
    }

    // Joins and leaves, in commit order, against the rules in force then.
    let intake = conference.settings.intake.clone().unwrap_or_default();
    let mut intake_recs: Vec<&Rec> = recs.iter().filter(|r| r.space == intake).collect();
    intake_recs.sort_by(|a, b| {
        a.intake_us()
            .cmp(&b.intake_us())
            .then_with(|| a.repo.cmp(&b.repo))
            .then_with(|| a.rkey.cmp(&b.rkey))
    });
    let mut requested_at: BTreeMap<String, u64> = BTreeMap::new();
    let mut uses = HashMap::new();
    for rec in intake_recs {
        let subject = rec.repo.clone();
        let us = rec.intake_us();
        match rec.collection.as_str() {
            JOIN => {
                if ban_at.get(&subject).is_some_and(|at| *at <= us) {
                    continue;
                }
                let hash = rec.str("code").map(|c| secrets.code_hmac(c));
                match conference.admission(&uses, &subject, hash.as_deref(), us) {
                    Some(via) => {
                        if via == Via::Code
                            && let Some(hash) = hash
                        {
                            let used: &mut CodeUse = uses.entry(hash).or_default();
                            used.count += 1;
                            used.bound.get_or_insert_with(|| subject.clone());
                        }
                        events.entry(subject).or_default().push((us, Event::Admit));
                    }
                    None if conference.settings_at(us).has("request") => {
                        requested_at.insert(subject, us);
                    }
                    None => {}
                }
            }
            LEAVE => events.entry(subject).or_default().push((us, Event::Leave)),
            _ => {}
        }
    }
    conference.uses = uses;

    // Each person's periods.
    for (subject, mut timeline) in events {
        timeline.sort_by_key(|(us, _)| *us);
        let mut periods = Vec::new();
        let mut since: Option<u64> = None;
        let mut banned = false;
        for (us, event) in timeline {
            match event {
                Event::Admit if !banned && since.is_none() => since = Some(us),
                Event::Remove | Event::Leave | Event::Ban => {
                    if let Some(start) = since.take() {
                        periods.push(Period { since: start, until: Some(us) });
                    }
                    banned |= event == Event::Ban;
                }
                Event::Admit => {}
            }
        }
        if let Some(start) = since {
            periods.push(Period { since: start, until: None });
        }
        if banned {
            conference.banned.insert(subject.clone());
        }
        if !periods.is_empty() {
            conference.members.insert(subject, periods);
        }
    }

    // Admins are members of every conference of theirs.
    let admin_since = admins
        .iter()
        .map(|(did, a)| (did.clone(), a.since_us))
        .chain(std::iter::once((super_admin.to_owned(), created_us)));
    for (did, since) in admin_since {
        if conference.banned.contains(&did) || conference.is_member(&did) {
            continue;
        }
        let periods = conference.members.entry(did).or_default();
        let since = periods.last().and_then(|p| p.until).map_or(since, |until| until.max(since));
        periods.push(Period { since, until: None });
    }

    // Requests: the latest join no rule admitted, not since decided, denied
    // or superseded by membership.
    for (subject, at) in requested_at {
        if conference.banned.contains(&subject) || conference.is_member(&subject) {
            continue;
        }
        let decided = decided_at.get(&subject).is_some_and(|d| *d > at);
        let denied = denied_at.get(&subject).is_some_and(|d| *d > at);
        if denied {
            conference.denied.insert(subject);
        } else if !decided {
            conference.pending.insert(subject, at);
        }
    }
    conference
}

/// An ISO 8601 timestamp as microseconds.
pub fn parse_iso_us(s: &str) -> Option<u64> {
    parse_iso_ms(s).map(|ms| ms as u64 * 1000)
}

/// `YYYY-MM-DDTHH:MM:SS[.fff](Z|±HH:MM)` as epoch milliseconds.
pub fn parse_iso_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-');
    let (year, month, day): (i64, i64, i64) =
        (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    let (clock, offset_secs) = if let Some(clock) = time.strip_suffix('Z') {
        (clock, 0)
    } else {
        let at = time.rfind(['+', '-'])?;
        let (clock, offset) = time.split_at(at);
        let sign = if offset.starts_with('-') { -1 } else { 1 };
        let (h, m) = offset[1..].split_once(':').unwrap_or((&offset[1..3.min(offset.len())], "0"));
        (clock, sign * (h.parse::<i64>().ok()? * 3600 + m.parse::<i64>().ok()? * 60))
    };
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, "0"));
    let mut t = hms.split(':');
    let (hour, minute, second): (i64, i64, i64) =
        (t.next()?.parse().ok()?, t.next()?.parse().ok()?, t.next().unwrap_or("0").parse().ok()?);
    let millis: i64 = format!("{:0<3}", &frac[..frac.len().min(3)]).parse().ok()?;
    // Days from civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + hour * 3600 + minute * 60 + second - offset_secs) * 1000 + millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_round_trip() {
        let ms = parse_iso_ms("2027-04-29T09:00:00+02:00").unwrap();
        assert_eq!(iso(ms as u64 * 1000), "2027-04-29T07:00:00.000Z");
        assert_eq!(parse_iso_ms("1970-01-01T00:00:01.5Z"), Some(1500));
        assert_eq!(
            parse_iso_ms("2024-02-29T12:30:00.123Z").map(|ms| iso(ms as u64 * 1000)).unwrap(),
            "2024-02-29T12:30:00.123Z"
        );
    }

    const ORG: &str = "did:plc:atmosphereorgaaaaaaaaaaa";
    const OLGA: &str = "did:plc:olgaaaaaaaaaaaaaaaaaaaaa";
    const PIM: &str = "did:plc:pimaaaaaaaaaaaaaaaaaaaaa";

    fn space(kind: &str) -> String {
        SpaceUri::new(ORG, kind, "3conf").to_string()
    }

    fn rec(space: &str, repo: &str, collection: &str, rkey: &str, us: u64, value: Value) -> Rec {
        Rec {
            space: space.to_owned(),
            repo: repo.to_owned(),
            collection: collection.to_owned(),
            rkey: rkey.to_owned(),
            rev: String::new(),
            us,
            seen_us: None,
            value,
        }
    }

    /// Atmosphere, with Pim as staff (unless `pim_is_admin` is false) and a
    /// conference whose join methods are `methods`, plus `more` records.
    fn org_with(methods: &[&str], pim_is_admin: bool, more: Vec<Rec>) -> Org {
        let admin = SpaceUri::admin(ORG).to_string();
        let mut recs = vec![rec(
            &admin,
            OLGA,
            SPACE,
            "s1",
            1,
            json!({ "space": space(CONFERENCE_TYPE), "intake": space(INTAKE_TYPE),
                    "superAdmin": OLGA, "join": { "methods": methods } }),
        )];
        if pim_is_admin {
            recs.push(rec(&admin, OLGA, ADMIN, PIM, 1, json!({ "subject": PIM, "role": "staff" })));
        }
        recs.extend(more);
        derive(ORG, OLGA, 0, &recs, &Secrets::for_tests(), "https://app.example/client.json")
    }

    fn role(subject: &str, role: &str, extra: Value) -> Rec {
        let mut value = json!({ "subject": subject, "role": role });
        value.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        rec(&space(CONFERENCE_TYPE), OLGA, ROLE, subject, 2, value)
    }

    fn join(who: &str, us: u64, seen_us: Option<u64>, value: Value) -> Rec {
        Rec { seen_us, ..rec(&space(INTAKE_TYPE), who, JOIN, "j", us, value) }
    }

    #[test]
    fn a_role_admits_only_while_whoever_assigned_it_could() {
        let bram = "did:plc:bramaaaaaaaaaaaaaaaaaaaa";
        let mallory = "did:plc:malloryaaaaaaaaaaaaaaaaa";
        let records = || {
            vec![
                role(bram, "speaker", json!({ "assignedBy": PIM })),
                // Staff can't make anyone an owner.
                role(mallory, "owner", json!({ "assignedBy": PIM })),
                join(bram, 20, None, json!({})),
                join(mallory, 21, None, json!({})),
            ]
        };
        let org = org_with(&["code"], true, records());
        let conference = org.conference(&space(CONFERENCE_TYPE)).unwrap();
        assert!(conference.is_member(bram));
        assert_eq!(conference.role_of(&org, bram).as_deref(), Some("speaker"));
        assert!(!conference.is_member(mallory));
        assert_eq!(conference.role_of(&org, mallory), None);

        // Once Pim isn't an admin, the role Pim assigned stops counting.
        let org = org_with(&["code"], false, records());
        let conference = org.conference(&space(CONFERENCE_TYPE)).unwrap();
        assert!(!conference.is_member(bram));
        assert_eq!(conference.role_of(&org, bram), None);
    }

    #[test]
    fn a_role_from_the_list_admits_only_while_the_list_does() {
        let ana = "did:plc:anaaaaaaaaaaaaaaaaaaaaaa";
        let records = || {
            vec![
                role(ana, "speaker", json!({ "assignedBy": OLGA, "via": "list" })),
                join(ana, 20, None, json!({})),
            ]
        };
        let off = org_with(&["code"], true, records());
        assert!(!off.conference(&space(CONFERENCE_TYPE)).unwrap().is_member(ana));
        let on = org_with(&["list"], true, records());
        assert!(on.conference(&space(CONFERENCE_TYPE)).unwrap().is_member(ana));
    }

    #[test]
    fn a_join_counts_from_when_it_was_first_seen_not_before() {
        let ana = "did:plc:anaaaaaaaaaaaaaaaaaaaaaa";
        let code = rec(
            &SpaceUri::admin(ORG).to_string(),
            OLGA,
            CODE,
            "c1",
            1_000_000,
            json!({ "space": space(CONFERENCE_TYPE), "codeHash": Secrets::for_tests().code_hmac("tulips"),
                    "expires": iso(5_000_000) }),
        );
        let joined = |seen_us| {
            let org = org_with(
                &["code"],
                true,
                vec![code.clone(), join(ana, 2_000_000, seen_us, json!({ "code": "tulips" }))],
            );
            org.conference(&space(CONFERENCE_TYPE)).unwrap().is_member(ana)
        };
        assert!(joined(None), "committed before the code expired");
        assert!(joined(Some(3_000_000)), "and seen before it did");
        assert!(!joined(Some(6_000_000)), "seen only after it expired: a backdated join");
    }

    fn admin_rec(subject: &str, role: &str, us: u64) -> Rec {
        rec(
            &SpaceUri::admin(ORG).to_string(),
            OLGA,
            ADMIN,
            subject,
            us,
            json!({ "subject": subject, "role": role }),
        )
    }

    fn member(by: &str, subject: &str, rkey: &str, us: u64, removed: bool) -> Rec {
        let mut value = json!({ "space": space(CONFERENCE_TYPE), "subject": subject });
        if removed {
            value["until"] = json!(iso(us));
        }
        rec(&SpaceUri::admin(ORG).to_string(), by, MEMBER, rkey, us, value)
    }

    #[test]
    fn an_admins_membership_keeps_its_start_and_its_end() {
        let admitted = member(OLGA, PIM, "m1", 10, false);
        // Promoted to owner later: the admin record is rewritten, but the
        // membership still starts when Pim became an admin.
        let org = org_with(&["code"], false, vec![admin_rec(PIM, "owner", 50), admitted.clone()]);
        let conference = org.conference(&space(CONFERENCE_TYPE)).unwrap();
        assert!(conference.was_member_at(PIM, 20));
        // Removed as an admin: what Pim wrote as a member still counts.
        let org = org_with(&["code"], false, vec![admitted, member(OLGA, PIM, "m2", 80, true)]);
        let conference = org.conference(&space(CONFERENCE_TYPE)).unwrap();
        assert!(!conference.is_member(PIM));
        assert!(conference.was_member_at(PIM, 20));
        assert!(!conference.was_member_at(PIM, 90));
    }

    #[test]
    fn a_conference_super_admin_counts_only_while_an_admin() {
        let mallory = "did:plc:malloryaaaaaaaaaaaaaaaaa";
        let conference_space = space(CONFERENCE_TYPE);
        let settings = rec(
            &SpaceUri::admin(ORG).to_string(),
            OLGA,
            SPACE,
            "s2",
            3,
            json!({ "space": conference_space, "intake": space(INTAKE_TYPE),
                    "superAdmin": PIM, "join": { "methods": ["code"] } }),
        );
        let made_owner = rec(
            &conference_space,
            PIM,
            ROLE,
            mallory,
            4,
            json!({ "subject": mallory, "role": "owner", "assignedBy": PIM }),
        );
        let org = org_with(&["code"], true, vec![settings.clone(), made_owner.clone()]);
        let conference = org.conference(&conference_space).unwrap();
        assert_eq!(conference.roles.get(mallory).map(String::as_str), Some("owner"));
        let org = org_with(&["code"], false, vec![settings, made_owner]);
        let conference = org.conference(&conference_space).unwrap();
        assert!(conference.roles.is_empty(), "Pim isn't an admin any more");
    }

    #[test]
    fn admins_cant_be_banned() {
        let kees = "did:plc:keesaaaaaaaaaaaaaaaaaaaa";
        let ban = |subject: &str| {
            rec(
                &SpaceUri::admin(ORG).to_string(),
                kees,
                BAN,
                subject,
                5,
                json!({ "space": space(CONFERENCE_TYPE), "subject": subject }),
            )
        };
        let org = org_with(&["code"], true, vec![admin_rec(kees, "owner", 1), ban(OLGA), ban(PIM)]);
        let conference = org.conference(&space(CONFERENCE_TYPE)).unwrap();
        assert!(conference.banned.is_empty());
        assert!(conference.is_member(OLGA) && conference.is_member(PIM));
    }

    #[test]
    fn a_handle_on_the_list_is_bound_once_and_only_for_its_importer() {
        let kees = "did:plc:keesaaaaaaaaaaaaaaaaaaaa";
        let ana = "did:plc:anaaaaaaaaaaaaaaaaaaaaaa";
        let entry = |by: &str, rkey: &str, value: Value| {
            let mut value = value;
            value["space"] = json!(space(CONFERENCE_TYPE));
            rec(&SpaceUri::admin(ORG).to_string(), by, LIST_ENTRY, rkey, 5, value)
        };
        let imported = entry(kees, "l1", json!({ "handle": "ana.test" }));
        let bound =
            entry(OLGA, "l2", json!({ "handle": "ana.test", "did": ana, "onBehalfOf": kees }));
        let org = org_with(&["list"], true, vec![admin_rec(kees, "owner", 1), imported.clone()]);
        let conference = org.conference(&space(CONFERENCE_TYPE)).unwrap();
        assert_eq!(conference.unbound_handles().count(), 1);
        assert_eq!(conference.unbound_handles().next().unwrap().by, kees);

        let org = org_with(
            &["list"],
            true,
            vec![admin_rec(kees, "owner", 1), imported.clone(), bound.clone()],
        );
        let conference = org.conference(&space(CONFERENCE_TYPE)).unwrap();
        assert_eq!(conference.unbound_handles().count(), 0, "bound to Ana for good");
        assert!(conference.on_list(ana));

        // Once Kees isn't an owner, the entry bound for Kees stops counting.
        let org = org_with(&["list"], true, vec![imported, bound]);
        assert!(!org.conference(&space(CONFERENCE_TYPE)).unwrap().on_list(ana));
    }

    #[test]
    fn a_conference_record_cant_be_dated_long_before_it_was_seen() {
        assert_eq!(conference_us(5_000_000, None), 5_000_000);
        // A notification's ordinary delay: dated by its commit.
        assert_eq!(conference_us(5_000_000, Some(30_000_000)), 5_000_000);
        // Backdated by more than that: dated by when it was seen.
        let seen = 500_000_000;
        assert_eq!(conference_us(5_000_000, Some(seen)), seen - BACKDATE_SLACK_US);
    }

    #[test]
    fn eventside_is_on_a_list_whatever_its_scopes() {
        let list =
            AppAccess::AllowList(vec!["https://app.example/oauth-client-metadata.json".into()]);
        assert!(list.allows(
            Some("https://app.example/oauth-client-metadata.json?scope=atproto%20x"),
            "https://app.example"
        ));
        assert!(!list.allows(Some("https://other.example/client.json"), "https://app.example"));
        assert!(!list.allows(None, "https://app.example"));
        assert!(AppAccess::Open.allows(None, "https://app.example"));
    }
}
