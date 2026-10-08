//! The index: permissions derived from records. Our database keeps a copy of
//! the records in an organization's admin space and intake spaces (and the
//! role and rules records in its conference spaces), each with the revision
//! of the commit that wrote it. Everything a host check or a feature asks
//! (who's an admin, a space's policies and app access, who's a member and
//! when, who's banned, which requests are waiting) is derived from them here,
//! the same way any app that can read those spaces could.
//!
//! Signed decisions (design review round 5): a permission record counts only
//! if our host's signature on it verifies against a key the authority's DID
//! document lists now ([`attest`]). Each decision was checked when it was
//! signed, so readers apply no precedence of their own: they replay each
//! person's signed entries in `seq` order.
//! - The organization is crawled from its super admin. Only the super
//!   admin's signed `admin` records name admins (`role: "none"` for a
//!   former one), and only their signed `space` records set a space's
//!   policies, app access and join methods.
//! - Each person in a conference has grounds for membership: an admin's
//!   admission, their own signed join, or being an admin of the
//!   organization. A removal or ban ends every ground at or below its rank;
//!   their own leave ends all of them. They're a member while any remains.
//! - An unsigned join is a request at most; an unsigned leave doesn't count.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{Value, json};

use super::attest::{self, Keys, Rank, Sig};
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
pub const CODE_REVOKE: &str = "app.eventside.admin.codeRevoke";
pub const LIST_ENTRY: &str = "app.eventside.admin.listEntry";
pub const JOIN: &str = "app.eventside.intake.join";
pub const LEAVE: &str = "app.eventside.intake.leave";
pub const ROLE: &str = "app.eventside.conference.role";
pub const RULES: &str = "app.eventside.conference.rules";

/// A `member` record's `via` when the super admin made someone an admin of
/// the organization (and so a member of each conference), and when they
/// stopped being one. These mark admin periods, not decisions about
/// someone's attendance.
pub const VIA_ADMIN: &str = "orgAdmin";
pub const VIA_ADMIN_REMOVED: &str = "orgAdminRemoved";
/// A `member` record's `via` for a removal.
pub const VIA_REMOVED: &str = "removed";

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
    pub value: Value,
}

impl Rec {
    fn str(&self, field: &str) -> Option<&str> {
        self.value.get(field).and_then(Value::as_str)
    }
}

/// How far before our host first saw a record in a conference space its
/// commit may be dated: enough for a write notification's ordinary delay.
/// Members' own records (plans, chat) only: permission records go by their
/// signatures.
pub const BACKDATE_SLACK_US: u64 = 60_000_000;

/// When a member's record in a conference space counts from: its commit, but
/// never more than [`BACKDATE_SLACK_US`] before our host first saw it. A
/// writer's PDS chooses its own revisions, so without this someone removed
/// could date a record back into a period when they were a member.
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
    /// When it was signed; 0 for the defaults.
    pub us: u64,
}

impl Settings {
    fn from_record(rec: &Rec, us: u64) -> Option<Self> {
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
            us,
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

/// An invite code, as its signed record made it.
#[derive(Debug, Clone)]
pub struct Code {
    pub seq: u64,
    /// The rank it was issued at: revoking it takes that rank or higher.
    pub rank: Rank,
    /// The admin who issued it.
    pub author: String,
    pub personal: bool,
    pub expires_us: Option<u64>,
    pub max_uses: Option<u64>,
    pub revoked: bool,
}

/// A row of an attendee list: a DID (a handle row is resolved when it's
/// imported) or an email's HMAC.
#[derive(Debug, Clone)]
pub struct ListEntry {
    /// The owner who imported it.
    pub by: String,
    pub did: Option<String>,
    pub email_hmac: Option<String>,
    pub role: Option<String>,
}

/// How a join was admitted: the `via` of a signed join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Role,
    List,
    Code,
    Open,
    Email,
}

impl Via {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Role => "role",
            Self::List => "list",
            Self::Code => "code",
            Self::Open => "open",
            Self::Email => "email",
        }
    }
}

/// A ground someone is a member on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ground {
    /// An admin's admission, at the rank it was signed with.
    Admission { by: String, rank: Rank },
    /// Their own signed join.
    Own,
    /// Being an admin of the organization: no removal or ban ends it.
    OrgAdmin,
}

impl Ground {
    fn rank(&self) -> Rank {
        match self {
            Self::Admission { rank, .. } => *rank,
            Self::Own => Rank::Person,
            Self::OrgAdmin => Rank::SuperAdmin,
        }
    }
}

/// Where someone stands in a conference, from replaying their entries.
#[derive(Debug, Clone, Default)]
pub struct Standing {
    pub grounds: Vec<Ground>,
    /// The rank of the ban in force, if any.
    pub ban: Option<Rank>,
    /// The decision that stands: its rank, and whether it admits. A
    /// decision contradicting it is signed only for someone of that rank or
    /// higher (or the person themselves joining or leaving).
    pub decision: Option<(Rank, bool)>,
}

impl Standing {
    fn member(&self) -> bool {
        self.grounds.contains(&Ground::OrgAdmin) || (self.ban.is_none() && !self.grounds.is_empty())
    }

    /// Ends every ground at or below `rank`, but not an admin's.
    fn end(&mut self, rank: Rank) {
        self.grounds.retain(|g| *g == Ground::OrgAdmin || g.rank() > rank);
    }

    /// A new decision: agreeing with the standing one keeps the higher rank.
    fn decide(&mut self, rank: Rank, admits: bool) {
        self.decision = Some(match self.decision {
            Some((above, says)) if says == admits => (above.max(rank), admits),
            _ => (rank, admits),
        });
    }
}

/// A conference, as its records make it.
#[derive(Debug, Clone)]
pub struct Conference {
    /// Its settings now.
    pub settings: Settings,
    history: Vec<Settings>,
    pub codes: BTreeMap<String, Code>,
    /// Who used each code, by its HMAC: distinct DIDs, first first.
    uses: BTreeMap<String, Vec<String>>,
    pub list: Vec<ListEntry>,
    /// The conference's super admin: who writes its roles and rules.
    pub super_admin_did: String,
    /// Roles, from the latest signed role record (or join) for each person.
    pub roles: BTreeMap<String, String>,
    /// Roles given by an attendee list, which admit their subject only while
    /// the list does.
    list_roles: BTreeSet<String>,
    /// Who assigned each role, when its record says.
    pub role_deciders: BTreeMap<String, String>,
    /// When each role took effect: its record's `since`, else when it was
    /// signed. A role counts for what its holder wrote from then.
    role_since: BTreeMap<String, u64>,
    /// When each person was an admin of the organization, from the super
    /// admin's `orgAdmin` and `orgAdminRemoved` member records.
    admin_periods: BTreeMap<String, Vec<Period>>,
    /// The rules, from the latest signed rules record.
    pub rules: Option<Value>,
    /// Membership periods by DID, past members included.
    pub members: BTreeMap<String, Vec<Period>>,
    pub banned: BTreeSet<String>,
    /// Requests waiting for an admin, with when they were made.
    pub pending: BTreeMap<String, u64>,
    /// People whose latest request was denied.
    pub denied: BTreeSet<String>,
    /// Where each person stands, for checking a decision before it's signed.
    people: BTreeMap<String, Standing>,
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

    /// Whether someone is on the list by their DID.
    pub fn on_list(&self, did: &str) -> bool {
        self.list.iter().any(|e| e.did.as_deref() == Some(did))
    }

    /// When `did`'s role took effect, if they have one.
    pub fn role_since(&self, did: &str) -> Option<u64> {
        self.roles.contains_key(did).then(|| self.role_since.get(did).copied()).flatten()
    }

    /// Whether `did`'s role came from an attendee list.
    pub fn has_list_role(&self, did: &str) -> bool {
        self.list_roles.contains(did)
    }

    /// Whether a code (by its HMAC) is one of this conference's.
    pub fn has_code(&self, code_hmac: &str) -> bool {
        self.codes.contains_key(code_hmac)
    }

    /// The codes an admin issued that haven't been revoked, with the rank
    /// each was issued at.
    pub fn codes_by(&self, author: &str) -> Vec<(String, Rank)> {
        self.codes
            .iter()
            .filter(|(_, c)| c.author == author && !c.revoked)
            .map(|(hash, c)| (hash.clone(), c.rank))
            .collect()
    }

    /// Everyone whose only grounds for membership are `admin`'s admissions:
    /// who undoing them takes out.
    pub fn only_admitted_by(&self, admin: &str) -> Vec<String> {
        self.people
            .iter()
            .filter(|(_, s)| {
                !s.grounds.is_empty()
                    && s.grounds
                        .iter()
                        .all(|g| matches!(g, Ground::Admission { by, .. } if by == admin))
            })
            .map(|(did, _)| did.clone())
            .collect()
    }

    /// Where `did` stands: their grounds, any ban, and the standing decision.
    pub fn standing(&self, did: &str) -> Standing {
        self.people.get(did).cloned().unwrap_or_default()
    }

    /// Who was the conference's super admin before the current one, by its
    /// settings snapshots (`org_super` when one names no one).
    pub fn past_super_admins(&self, org_super: &str) -> BTreeSet<String> {
        self.history
            .iter()
            .map(|s| s.super_admin.clone().unwrap_or_else(|| org_super.to_owned()))
            .filter(|did| *did != self.super_admin_did)
            .collect()
    }

    /// Whether a code admits `did` at `now_us`: it's valid, not revoked or
    /// expired, and within its limits, counting distinct DIDs (the records'
    /// uses, and `also`, the journal's). A personal code is bound to the
    /// first DID that used it.
    pub fn code_admits(&self, hash: &str, did: &str, now_us: u64, also: &BTreeSet<String>) -> bool {
        let Some(code) = self.codes.get(hash) else { return false };
        if code.revoked || code.expires_us.is_some_and(|exp| now_us >= exp) {
            return false;
        }
        let mut users: BTreeSet<&str> =
            self.uses.get(hash).into_iter().flatten().map(String::as_str).collect();
        users.extend(also.iter().map(String::as_str));
        if code.personal {
            users.iter().all(|user| *user == did)
        } else {
            users.contains(did) || code.max_uses.is_none_or(|max| (users.len() as u64) < max)
        }
    }

    /// The rule that admits a join by `did` now, if any: a pre-assigned
    /// role, the attendee list by DID, a valid code, or an open conference.
    pub fn admits(
        &self,
        did: &str,
        code_hash: Option<&str>,
        now_us: u64,
        also: &BTreeSet<String>,
    ) -> Option<Via> {
        let settings = &self.settings;
        if self.roles.contains_key(did) && (!self.list_roles.contains(did) || settings.has("list"))
        {
            Some(Via::Role)
        } else if settings.has("list") && self.on_list(did) {
            Some(Via::List)
        } else if settings.has("code")
            && code_hash.is_some_and(|h| self.code_admits(h, did, now_us, also))
        {
            Some(Via::Code)
        } else if settings.has("open") {
            Some(Via::Open)
        } else {
            None
        }
    }

    /// The role `did` has in the conference: the role records', else their
    /// admin role.
    pub fn role_of(&self, org: &Org, did: &str) -> Option<String> {
        self.roles.get(did).cloned().or_else(|| org.admin_role(did).map(|r| r.as_str().to_owned()))
    }

    /// The role `did` had at `us`, from the role records (not their admin
    /// role): the current one, once it took effect.
    pub fn role_at(&self, did: &str, us: u64) -> Option<&str> {
        self.roles
            .get(did)
            .filter(|_| self.role_since.get(did).is_none_or(|since| *since <= us))
            .map(String::as_str)
    }

    /// Whether `did` was an admin of the organization at `us`: the super
    /// admin always; anyone else within an admin period the super admin's
    /// records mark (an open one only while they're still an admin).
    pub fn was_admin_at(&self, org: &Org, did: &str, us: u64) -> bool {
        if did == org.super_admin {
            return true;
        }
        let periods = self.admin_periods.get(did).map(Vec::as_slice).unwrap_or_default();
        if periods.iter().any(|p| p.until.is_some() && p.covers(us)) {
            return true;
        }
        org.admins.get(did).is_some_and(|admin| {
            let open = periods.iter().find(|p| p.until.is_none()).map(|p| p.since);
            open.map_or(admin.since_us, |since| since.min(admin.since_us)) <= us
        })
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
    /// The admins the super admin's `admin` records name now.
    pub admins: BTreeMap<String, Admin>,
    /// Everyone an `admin` record names, former admins included: their
    /// decisions still stand, so the crawl still reads their repos.
    pub named_admins: BTreeSet<String>,
    pub admin_settings: Settings,
    pub conferences: BTreeMap<String, Conference>,
    /// The highest `seq` any record's signature carries.
    pub max_seq: u64,
}

impl Org {
    /// An admin's role. The super admin is always an owner, whatever an
    /// `admin` record names them.
    pub fn admin_role(&self, did: &str) -> Option<Role> {
        if did == self.super_admin {
            return Some(Role::Owner);
        }
        self.admins.get(did).map(|admin| admin.role)
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

/// The collections the index derives anything from in an intake space.
const INTAKE_DERIVED: [&str; 2] = [JOIN, LEAVE];
/// The collections the index derives anything from in a conference space:
/// the rest there (plans, chat) are members' own, and no one's access
/// depends on them.
const CONFERENCE_DERIVED: [&str; 2] = [ROLE, RULES];

/// Whether the index derives anything from a collection's records in a
/// space: everything in an organization's admin space, joins and leaves in
/// an intake space, roles and rules in a conference space. What [`records`]
/// reads, reindex rebuilds from, and a sync re-derives the view for.
pub fn derives_from(space: &SpaceUri, collection: &str) -> bool {
    match space.kind.as_str() {
        ADMIN_TYPE => space.skey == "self",
        INTAKE_TYPE => INTAKE_DERIVED.contains(&collection),
        CONFERENCE_TYPE => CONFERENCE_DERIVED.contains(&collection),
        _ => false,
    }
}

/// The index's records for an organization: those [`derives_from`] names.
pub async fn records(state: &AppState, org: &str) -> Result<Vec<Rec>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT space, repo, collection, rkey, rev, value FROM space_records \
         WHERE value IS NOT NULL AND \
         (space = $1 OR (space LIKE $2 AND collection IN ($6, $7)) \
         OR (space LIKE $3 AND collection IN ($4, $5)))",
    )
    .bind(SpaceUri::admin(org).to_string())
    .bind(format!("at://{org}/space/{INTAKE_TYPE}/%"))
    .bind(format!("at://{org}/space/{CONFERENCE_TYPE}/%"))
    .bind(CONFERENCE_DERIVED[0])
    .bind(CONFERENCE_DERIVED[1])
    .bind(INTAKE_DERIVED[0])
    .bind(INTAKE_DERIVED[1])
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("could not read the index: {e}"))?;
    Ok(rows
        .into_iter()
        .filter_map(|(space, repo, collection, rkey, rev, value)| {
            // The query narrows by prefix; this is the rule itself.
            if !SpaceUri::parse(&space).is_some_and(|s| derives_from(&s, &collection)) {
                return None;
            }
            Some(Rec {
                us: tid_micros(&rev)?,
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

/// How many times an organization's index has changed. Every write of a
/// record its view is derived from ([`derives_from`]) bumps it, from the
/// server or the admin CLI (another process), and so does a change to its
/// attestation keys, so a derived view is reused exactly until what's under
/// it changes.
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
    let keys = attest::keys(&state.db, org).await?;
    let eventside = state.oauth.client_id_for("atproto");
    let derived = Arc::new(derive(
        org,
        &authority.super_admin,
        authority.created_at as u64 * 1000,
        &recs,
        &state.secrets,
        &eventside,
        &keys,
        crate::db::now_ms() as u64 * 1000,
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

/// Derives an organization's permissions from its records: those whose
/// signature verifies against `keys` (at `now_us`), each `seq` once.
#[allow(clippy::too_many_arguments)]
pub fn derive(
    org: &str,
    super_admin: &str,
    created_us: u64,
    recs: &[Rec],
    secrets: &Secrets,
    eventside_client: &str,
    keys: &Keys,
    now_us: u64,
) -> Org {
    let admin_space = SpaceUri::admin(org).to_string();

    // Each `seq` counts once. Within a repo and space, the records that
    // claim the same `seq` are versions of one decision (a copy, or an old
    // version put back): the latest written is the one that stands, and it
    // counts only if it verifies. Across repos or spaces a signature can't be
    // copied (`repository` and `space` are signed), so a valid duplicate
    // there means a leaked key; the first, in a fixed order, counts.
    let mut latest: BTreeMap<(&str, &str, u64), &Rec> = BTreeMap::new();
    let mut unclaimed: Vec<&Rec> = Vec::new();
    for rec in recs {
        match attest::claimed_seq(&rec.value, org, &rec.space) {
            Some(seq) => {
                let slot =
                    latest.entry((rec.space.as_str(), rec.repo.as_str(), seq)).or_insert(rec);
                if (&rec.rev, &rec.rkey) > (&slot.rev, &slot.rkey) {
                    *slot = rec;
                }
            }
            None => unclaimed.push(rec),
        }
    }
    let mut ordered: Vec<&Rec> = latest.into_values().chain(unclaimed).collect();
    ordered.sort_by(|a, b| {
        (&a.space, &a.repo, &a.collection, &a.rkey).cmp(&(
            &b.space,
            &b.repo,
            &b.collection,
            &b.rkey,
        ))
    });
    let mut seqs = BTreeSet::new();
    let mut signed: Vec<(&Rec, Sig)> = Vec::new();
    let mut unsigned_joins: Vec<&Rec> = Vec::new();
    let mut max_seq = 0;
    for rec in ordered {
        // The collection is in the record's path, not in what's signed: a
        // record counts only in the collection its signed `$type` names, so
        // one moved into another collection (a deny into bans) doesn't.
        let in_place = rec.str("$type") == Some(rec.collection.as_str());
        match attest::verify(&rec.value, &rec.repo, &rec.space, keys, now_us).filter(|_| in_place) {
            Some(sig) => {
                max_seq = max_seq.max(sig.seq);
                if seqs.insert(sig.seq) {
                    signed.push((rec, sig));
                }
            }
            None if rec.collection == JOIN => unsigned_joins.push(rec),
            None => {}
        }
    }
    signed.sort_by_key(|(_, sig)| sig.seq);

    // Admins, as the super admin's latest signed `admin` record about each
    // names them.
    let mut named: BTreeMap<String, (&Rec, Sig)> = BTreeMap::new();
    for (rec, sig) in &signed {
        if rec.space == admin_space
            && rec.repo == super_admin
            && rec.collection == ADMIN
            && sig.rank == Rank::SuperAdmin
            && let Some(subject) = rec.str("subject")
        {
            named.insert(subject.to_owned(), (rec, *sig));
        }
    }
    let admins: BTreeMap<String, Admin> = named
        .iter()
        .filter_map(|(subject, (rec, sig))| {
            let role = rec.str("role").and_then(Role::parse)?;
            let since_us = rec.str("since").and_then(parse_iso_us).unwrap_or(sig.signed_us);
            Some((subject.clone(), Admin { role, since_us }))
        })
        .collect();

    // Space settings: the super admin's signed snapshots, latest last.
    let mut snapshots: BTreeMap<String, Vec<Settings>> = BTreeMap::new();
    for (rec, sig) in &signed {
        if rec.space == admin_space
            && rec.repo == super_admin
            && rec.collection == SPACE
            && sig.rank == Rank::SuperAdmin
            && let Some(settings) = Settings::from_record(rec, sig.signed_us)
            && SpaceUri::parse(&settings.space).is_some_and(|s| s.authority == org)
        {
            snapshots.entry(settings.space.clone()).or_default().push(settings);
        }
    }
    let admin_settings = snapshots
        .get(&admin_space)
        .and_then(|h| h.last().cloned())
        .unwrap_or_else(|| Settings::default_admin(org, eventside_client));

    let context = Context {
        admin_space: &admin_space,
        super_admin,
        admins: &admins,
        created_us,
        secrets,
        signed: &signed,
        unsigned_joins: &unsigned_joins,
    };
    let mut conferences = BTreeMap::new();
    for (space, history) in snapshots {
        let Some(current) = history.last().cloned() else { continue };
        if current.kind != CONFERENCE_TYPE {
            continue;
        }
        conferences.insert(space, derive_conference(current, history, &context));
    }

    Org {
        did: org.to_owned(),
        super_admin: super_admin.to_owned(),
        created_us,
        admins,
        named_admins: named.into_keys().collect(),
        admin_settings,
        conferences,
        max_seq,
    }
}

/// What every conference of an organization is derived from.
struct Context<'a> {
    admin_space: &'a str,
    super_admin: &'a str,
    admins: &'a BTreeMap<String, Admin>,
    created_us: u64,
    secrets: &'a Secrets,
    /// The signed records, in `seq` order.
    signed: &'a [(&'a Rec, Sig)],
    unsigned_joins: &'a [&'a Rec],
}

/// One entry in a person's timeline.
#[derive(Debug, Clone)]
enum Event {
    Admit { by: String },
    Remove,
    Ban,
    Deny,
    AdminStart,
    AdminEnd,
    Join,
    Request,
    Leave,
}

fn derive_conference(settings: Settings, history: Vec<Settings>, cx: &Context) -> Conference {
    let space = settings.space.clone();
    let intake = settings.intake.clone().unwrap_or_default();
    let conference_super =
        settings.super_admin.clone().unwrap_or_else(|| cx.super_admin.to_owned());
    let about = |rec: &Rec, sig: &Sig| {
        rec.space == cx.admin_space
            && rec.str("space") == Some(space.as_str())
            && sig.rank != Rank::Person
    };

    // Codes, their revocations, and the attendee list.
    let mut codes: BTreeMap<String, Code> = BTreeMap::new();
    let mut list = Vec::new();
    for (rec, sig) in cx.signed.iter().filter(|(r, s)| about(r, s)) {
        match rec.collection.as_str() {
            CODE => {
                if let Some(hash) = rec.str("codeHash") {
                    codes.insert(
                        hash.to_owned(),
                        Code {
                            seq: sig.seq,
                            rank: sig.rank,
                            author: rec.repo.clone(),
                            personal: rec
                                .value
                                .get("personal")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            expires_us: rec.str("expires").and_then(parse_iso_us),
                            max_uses: rec.value.get("maxUses").and_then(Value::as_u64),
                            revoked: false,
                        },
                    );
                }
            }
            CODE_REVOKE => {
                if let Some(code) = rec.str("codeHash").and_then(|h| codes.get_mut(h))
                    && code.seq < sig.seq
                {
                    code.revoked = true;
                }
            }
            LIST_ENTRY => list.push(ListEntry {
                by: rec.repo.clone(),
                did: rec.str("did").map(str::to_owned),
                email_hmac: rec.str("emailHmac").map(str::to_owned),
                role: rec.str("role").map(str::to_owned),
            }),
            _ => {}
        }
    }

    // Roles and rules: the conference super admin's signed records in its
    // space, the latest by `seq` for each person (or a join's role, if later).
    struct Given {
        seq: u64,
        role: String,
        since: u64,
        list: bool,
        by: Option<String>,
    }
    let mut given: BTreeMap<String, Given> = BTreeMap::new();
    let mut rules = None;
    for (rec, sig) in cx.signed {
        if rec.space != space || rec.repo != conference_super || sig.rank == Rank::Person {
            continue;
        }
        match rec.collection.as_str() {
            ROLE => {
                if let (Some(subject), Some(role)) = (rec.str("subject"), rec.str("role")) {
                    given.insert(
                        subject.to_owned(),
                        Given {
                            seq: sig.seq,
                            role: role.to_owned(),
                            since: rec.str("since").and_then(parse_iso_us).unwrap_or(sig.signed_us),
                            list: rec.str("via") == Some("list"),
                            by: rec.str("assignedBy").map(str::to_owned),
                        },
                    );
                }
            }
            RULES if rec.rkey == "self" => rules = Some(rec.value.clone()),
            _ => {}
        }
    }

    // Each person's signed entries, in `seq` order.
    let mut timelines: BTreeMap<String, Vec<(Sig, Event)>> = BTreeMap::new();
    let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (rec, sig) in cx.signed {
        if about(rec, sig) {
            let Some(subject) = rec.str("subject") else { continue };
            let event = match rec.collection.as_str() {
                MEMBER => match rec.str("via") {
                    Some(VIA_ADMIN) if sig.rank == Rank::SuperAdmin => Event::AdminStart,
                    Some(VIA_ADMIN_REMOVED) if sig.rank == Rank::SuperAdmin => Event::AdminEnd,
                    Some(VIA_ADMIN | VIA_ADMIN_REMOVED) => continue,
                    via if via == Some(VIA_REMOVED) || rec.value.get("until").is_some() => {
                        Event::Remove
                    }
                    _ => Event::Admit { by: rec.repo.clone() },
                },
                BAN => Event::Ban,
                DENY => Event::Deny,
                _ => continue,
            };
            timelines.entry(subject.to_owned()).or_default().push((*sig, event));
        } else if rec.space == intake && sig.rank == Rank::Person {
            let person = rec.repo.clone();
            let event = match rec.collection.as_str() {
                JOIN if rec.str("via") == Some("request") => Event::Request,
                JOIN => {
                    if let Some(code) = rec.str("code").filter(|_| rec.str("via") == Some("code")) {
                        let users = uses.entry(cx.secrets.code_hmac(code)).or_default();
                        if !users.contains(&person) {
                            users.push(person.clone());
                        }
                    }
                    if let Some(role) = rec.str("role") {
                        let later = given.get(&person).is_none_or(|g| g.seq < sig.seq);
                        if later {
                            given.insert(
                                person.clone(),
                                Given {
                                    seq: sig.seq,
                                    role: role.to_owned(),
                                    since: sig.signed_us,
                                    list: true,
                                    by: None,
                                },
                            );
                        }
                    }
                    Event::Join
                }
                LEAVE => Event::Leave,
                _ => continue,
            };
            timelines.entry(person).or_default().push((*sig, event));
        }
    }

    let mut roles = BTreeMap::new();
    let mut list_roles = BTreeSet::new();
    let mut role_deciders = BTreeMap::new();
    let mut role_since = BTreeMap::new();
    for (subject, g) in given {
        if !matches!(g.role.as_str(), "owner" | "staff" | "speaker") {
            continue;
        }
        if g.list {
            list_roles.insert(subject.clone());
        }
        if let Some(by) = g.by {
            role_deciders.insert(subject.clone(), by);
        }
        role_since.insert(subject.clone(), g.since);
        roles.insert(subject, g.role);
    }

    let is_admin = |did: &str| did == cx.super_admin || cx.admins.contains_key(did);
    let mut members = BTreeMap::new();
    let mut banned = BTreeSet::new();
    let mut pending = BTreeMap::new();
    let mut denied = BTreeSet::new();
    let mut people = BTreeMap::new();
    let mut admin_periods = BTreeMap::new();
    let mut last_decided: BTreeMap<String, u64> = BTreeMap::new();

    for (did, timeline) in timelines {
        let mut standing = Standing::default();
        let mut periods = Vec::new();
        let mut since: Option<u64> = None;
        let mut request: Option<u64> = None;
        let mut was_denied = false;
        let mut admin_marks = Vec::new();
        let mut admin_since: Option<u64> = None;
        for (sig, event) in &timeline {
            let rank = sig.rank;
            match event {
                Event::Admit { by } => {
                    match standing.ban {
                        Some(ban) if rank >= ban => standing.ban = None,
                        Some(_) => continue,
                        None => {}
                    }
                    standing.grounds.push(Ground::Admission { by: by.clone(), rank });
                    standing.decide(rank, true);
                    request = None;
                    was_denied = false;
                }
                Event::Remove => {
                    standing.end(rank);
                    standing.decide(rank, false);
                    request = None;
                }
                Event::Ban => {
                    standing.end(rank);
                    standing.ban = Some(standing.ban.map_or(rank, |b| b.max(rank)));
                    standing.decide(rank, false);
                    request = None;
                }
                Event::Deny => {
                    standing.decide(rank, false);
                    request = None;
                    was_denied = true;
                }
                Event::AdminStart => {
                    standing.grounds.retain(|g| *g != Ground::OrgAdmin);
                    standing.grounds.push(Ground::OrgAdmin);
                    admin_since.get_or_insert(sig.signed_us);
                }
                Event::AdminEnd => {
                    standing.grounds.retain(|g| *g != Ground::OrgAdmin);
                    if let Some(start) = admin_since.take() {
                        admin_marks.push(Period { since: start, until: Some(sig.signed_us) });
                    }
                }
                Event::Join => {
                    if standing.ban.is_some() {
                        continue;
                    }
                    standing.grounds.push(Ground::Own);
                    // The person's own join stands over an earlier removal
                    // or denial; it agrees with an admission.
                    standing.decide(Rank::Person, true);
                    request = None;
                    was_denied = false;
                }
                Event::Request => {
                    if standing.ban.is_some() {
                        continue;
                    }
                    request = Some(sig.signed_us);
                    was_denied = false;
                }
                Event::Leave => {
                    standing.grounds.retain(|g| *g == Ground::OrgAdmin);
                    standing.decision = Some((Rank::Person, false));
                    request = None;
                }
            }
            let member = standing.member();
            match (member, since) {
                (true, None) => since = Some(sig.signed_us),
                (false, Some(start)) => {
                    periods.push(Period { since: start, until: Some(sig.signed_us) });
                    since = None;
                }
                _ => {}
            }
        }
        if let Some(start) = since {
            periods.push(Period { since: start, until: None });
        }
        if let Some(start) = admin_since {
            admin_marks.push(Period { since: start, until: None });
        }
        if !admin_marks.is_empty() {
            admin_periods.insert(did.clone(), admin_marks);
        }
        if let Some((last, _)) = timeline.last() {
            last_decided.insert(did.clone(), last.signed_us);
        }
        let member = standing.member();
        if standing.ban.is_some() && !is_admin(&did) {
            banned.insert(did.clone());
        }
        if !member && standing.ban.is_none() {
            if let Some(at) = request {
                pending.insert(did.clone(), at);
            } else if was_denied {
                denied.insert(did.clone());
            }
        }
        if !periods.is_empty() {
            members.insert(did.clone(), periods);
        }
        people.insert(did, standing);
    }

    let mut conference = Conference {
        settings,
        history,
        codes,
        uses,
        list,
        super_admin_did: conference_super,
        roles,
        list_roles,
        role_deciders,
        role_since,
        admin_periods,
        rules,
        members,
        banned,
        pending,
        denied,
        people,
    };

    // Admins are members of every conference of theirs.
    let admin_since = cx
        .admins
        .iter()
        .map(|(did, a)| (did.clone(), a.since_us))
        .chain(std::iter::once((cx.super_admin.to_owned(), cx.created_us)));
    for (did, since) in admin_since {
        if conference.is_member(&did) {
            continue;
        }
        let periods = conference.members.entry(did).or_default();
        let since = periods.last().and_then(|p| p.until).map_or(since, |until| until.max(since));
        periods.push(Period { since, until: None });
    }

    // A join written by another app, without our signature, is a request at
    // most: pending while requests are on, unless something about the
    // person was signed after it.
    let mut unsigned: BTreeMap<&str, u64> = BTreeMap::new();
    for rec in cx.unsigned_joins.iter().filter(|r| r.space == intake) {
        let at = unsigned.entry(rec.repo.as_str()).or_default();
        *at = (*at).max(rec.us);
    }
    if conference.settings.has("request") {
        for (did, at) in unsigned {
            let decided = last_decided.get(did).is_some_and(|d| *d >= at);
            if decided || conference.is_member(did) || conference.banned.contains(did) {
                continue;
            }
            conference.denied.remove(did);
            let waiting = conference.pending.entry(did.to_owned()).or_insert(at);
            *waiting = (*waiting).max(at);
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
    use crate::keys::EcKey;
    use crate::spacehost::attest::tests::{signed, test_keys};

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
    const KEES: &str = "did:plc:keesaaaaaaaaaaaaaaaaaaaa";
    const ANA: &str = "did:plc:anaaaaaaaaaaaaaaaaaaaaaa";
    const BRAM: &str = "did:plc:bramaaaaaaaaaaaaaaaaaaaa";
    /// A time, in microseconds, all signings here are after.
    const T0: u64 = 1_800_000_000_000_000;

    fn space(kind: &str) -> String {
        SpaceUri::new(ORG, kind, "3conf").to_string()
    }

    /// Records, signed as our host would, one `seq` after another.
    struct World {
        key: EcKey,
        keys: Keys,
        seq: u64,
        recs: Vec<Rec>,
    }

    impl World {
        /// Atmosphere, with Pim as staff and Kees as an owner, and a
        /// conference whose join methods are `methods`.
        fn new(methods: &[&str]) -> Self {
            let (key, keys) = test_keys(ORG);
            let mut world = Self { key, keys, seq: 0, recs: Vec::new() };
            let admin = SpaceUri::admin(ORG).to_string();
            world.sign(
                &admin,
                OLGA,
                SPACE,
                "s1",
                Rank::SuperAdmin,
                json!({ "space": space(CONFERENCE_TYPE),
                "intake": space(INTAKE_TYPE), "superAdmin": OLGA, "join": { "methods": methods } }),
            );
            world.sign(
                &admin,
                OLGA,
                ADMIN,
                PIM,
                Rank::SuperAdmin,
                json!({ "subject": PIM, "role": "staff" }),
            );
            world.sign(
                &admin,
                OLGA,
                ADMIN,
                KEES,
                Rank::SuperAdmin,
                json!({ "subject": KEES, "role": "owner" }),
            );
            world
        }

        fn sign(
            &mut self,
            space: &str,
            repo: &str,
            collection: &str,
            rkey: &str,
            rank: Rank,
            value: Value,
        ) {
            self.seq += 1;
            let us = T0 + self.seq * 1000;
            let mut value = value;
            value["$type"] = json!(collection);
            let value = signed(value, &self.key, ORG, space, self.seq, rank, us, repo);
            self.recs.push(Rec {
                space: space.to_owned(),
                repo: repo.to_owned(),
                collection: collection.to_owned(),
                rkey: rkey.to_owned(),
                rev: String::new(),
                us,
                value,
            });
        }

        /// An admin's decision about someone in the conference.
        fn decide(&mut self, by: &str, rank: Rank, collection: &str, subject: &str, extra: Value) {
            let mut value = json!({ "space": space(CONFERENCE_TYPE), "subject": subject });
            value.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            let rkey = format!("d{}", self.seq + 1);
            self.sign(&SpaceUri::admin(ORG).to_string(), by, collection, &rkey, rank, value);
        }

        fn join(&mut self, who: &str, value: Value) {
            let rkey = format!("j{}", self.seq + 1);
            self.sign(&space(INTAKE_TYPE), who, JOIN, &rkey, Rank::Person, value);
        }

        fn org(&self) -> Org {
            derive(
                ORG,
                OLGA,
                T0,
                &self.recs,
                &Secrets::for_tests(),
                "https://app.example/client.json",
                &self.keys,
                T0 + 1_000_000_000,
            )
        }

        fn conference(&self) -> Conference {
            self.org().conference(&space(CONFERENCE_TYPE)).unwrap().clone()
        }
    }

    #[test]
    fn tc_59_an_admin_record_without_our_signature_doesnt_count() {
        let mut world = World::new(&["code"]);
        world.recs.push(Rec {
            space: SpaceUri::admin(ORG).to_string(),
            repo: PIM.into(),
            collection: MEMBER.into(),
            rkey: "x".into(),
            rev: String::new(),
            us: T0,
            value: json!({ "space": space(CONFERENCE_TYPE), "subject": BRAM, "via": "admin" }),
        });
        assert!(!world.conference().is_member(BRAM));
        world.decide(PIM, Rank::Staff, MEMBER, BRAM, json!({ "via": "admin" }));
        assert!(world.conference().is_member(BRAM));
    }

    #[test]
    fn tc_53_a_former_admins_decisions_keep_standing() {
        let mut world = World::new(&["code"]);
        world.decide(PIM, Rank::Staff, MEMBER, BRAM, json!({ "via": "admin" }));
        // Olga removes Pim as an admin.
        let admin = SpaceUri::admin(ORG).to_string();
        world.sign(
            &admin,
            OLGA,
            ADMIN,
            PIM,
            Rank::SuperAdmin,
            json!({ "subject": PIM, "role": "none" }),
        );
        let org = world.org();
        assert!(!org.is_admin(PIM));
        assert!(org.named_admins.contains(PIM), "the crawl still reads his repo");
        assert!(org.conference(&space(CONFERENCE_TYPE)).unwrap().is_member(BRAM));
    }

    #[test]
    fn tc_58_grounds_stand_until_a_removal_of_their_rank() {
        let mut world = World::new(&["code"]);
        world.join(ANA, json!({ "via": "open" }));
        world.decide(PIM, Rank::Staff, MEMBER, ANA, json!({ "via": "admin" }));
        world.decide(PIM, Rank::Staff, MEMBER, BRAM, json!({ "via": "admin" }));
        let conference = world.conference();
        assert_eq!(conference.standing(ANA).grounds.len(), 2, "her own join and Pim's admission");
        assert_eq!(
            conference.standing(BRAM).grounds,
            [Ground::Admission { by: PIM.into(), rank: Rank::Staff }]
        );
        // An owner's removal ends both of Ana's grounds.
        world.decide(KEES, Rank::Owner, MEMBER, ANA, json!({ "via": "removed", "until": iso(T0) }));
        let conference = world.conference();
        assert!(!conference.is_member(ANA));
        assert_eq!(conference.standing(ANA).decision, Some((Rank::Owner, false)));
        // Her own join stands over it.
        world.join(ANA, json!({ "via": "open" }));
        assert!(world.conference().is_member(ANA));
    }

    #[test]
    fn tc_60_a_ban_keeps_the_rank_it_was_signed_with() {
        let mut world = World::new(&["code"]);
        world.join(BRAM, json!({ "via": "open" }));
        world.decide(KEES, Rank::Owner, BAN, BRAM, json!({}));
        // Kees is made staff: the ban he signed as an owner keeps its rank.
        let admin = SpaceUri::admin(ORG).to_string();
        world.sign(
            &admin,
            OLGA,
            ADMIN,
            KEES,
            Rank::SuperAdmin,
            json!({ "subject": KEES, "role": "staff" }),
        );
        let conference = world.conference();
        assert!(conference.banned.contains(BRAM));
        assert_eq!(conference.standing(BRAM).ban, Some(Rank::Owner));
        // The super admin's admission lifts it.
        world.decide(OLGA, Rank::SuperAdmin, MEMBER, BRAM, json!({ "via": "admin" }));
        let conference = world.conference();
        assert!(conference.is_member(BRAM) && !conference.banned.contains(BRAM));
    }

    #[test]
    fn tc_64_an_unsigned_join_is_a_request_at_most() {
        let mut world = World::new(&["code", "request"]);
        world.recs.push(Rec {
            space: space(INTAKE_TYPE),
            repo: ANA.into(),
            collection: JOIN.into(),
            rkey: "j".into(),
            rev: String::new(),
            // After the world's first three signings, before the next.
            us: T0 + 3_500,
            value: json!({ "code": "atmosphere27" }),
        });
        let conference = world.conference();
        assert!(!conference.is_member(ANA));
        assert!(conference.pending.contains_key(ANA));
        // Denied after it, it's no longer waiting.
        world.decide(OLGA, Rank::SuperAdmin, DENY, ANA, json!({}));
        let conference = world.conference();
        assert!(!conference.pending.contains_key(ANA));
        // With requests off, it's nothing at all.
        let mut off = World::new(&["code"]);
        off.recs.push(Rec {
            repo: ANA.into(),
            ..world.recs.iter().find(|r| r.rkey == "j").unwrap().clone()
        });
        assert!(off.conference().pending.is_empty());
    }

    #[test]
    fn a_copy_with_the_same_seq_counts_once_and_leaves_dont_count_unsigned() {
        let mut world = World::new(&["open"]);
        world.join(ANA, json!({ "via": "open" }));
        let copy = Rec { rkey: "copy".into(), ..world.recs.last().unwrap().clone() };
        world.recs.push(copy);
        // An unsigned leave doesn't take her out.
        world.recs.push(Rec {
            space: space(INTAKE_TYPE),
            repo: ANA.into(),
            collection: LEAVE.into(),
            rkey: "l".into(),
            rev: String::new(),
            us: T0 + 900_000,
            value: json!({}),
        });
        let conference = world.conference();
        assert_eq!(conference.standing(ANA).grounds, [Ground::Own]);
        assert!(conference.is_member(ANA));
        // A signed one does.
        world.sign(&space(INTAKE_TYPE), ANA, LEAVE, "l2", Rank::Person, json!({}));
        assert!(!world.conference().is_member(ANA));
    }

    #[test]
    fn tc_63_the_latest_version_of_a_decision_in_its_repo_stands() {
        let mut world = World::new(&["code"]);
        world.decide(OLGA, Rank::SuperAdmin, MEMBER, BRAM, json!({ "via": "admin" }));
        assert!(world.conference().is_member(BRAM));
        // A later copy of it, in the same repo, whose signature no longer
        // verifies (here: tampered), is the version that stands.
        let mut copy = world.recs.last().unwrap().clone();
        copy.rkey = "copy".into();
        copy.rev = "3zzzzzzzzzzzz".into();
        copy.value["signatures"][0]["signature"]["$bytes"] =
            json!(crate::crypto::encode_b64(&[1; 64]));
        world.recs.push(copy.clone());
        assert!(!world.conference().is_member(BRAM));
        // Written earlier than the original, it's an old version: the original stands.
        world.recs.last_mut().unwrap().rev = String::new();
        world
            .recs
            .iter_mut()
            .filter(|r| r.rkey != "copy")
            .for_each(|r| r.rev = "3aaaaaaaaaaaa".into());
        assert!(world.conference().is_member(BRAM));
        // In another repo, a copy changes nothing.
        let elsewhere = Rec { repo: PIM.into(), rev: "3zzzzzzzzzzzz".into(), ..copy };
        world.recs.retain(|r| r.rkey != "copy");
        world.recs.push(elsewhere);
        assert!(world.conference().is_member(BRAM));
    }

    #[test]
    fn a_signed_record_moved_into_another_collection_doesnt_count() {
        let mut world = World::new(&["request"]);
        world.decide(PIM, Rank::Staff, DENY, BRAM, json!({ "$type": DENY }));
        let moved = Rec {
            collection: BAN.into(),
            rkey: "moved".into(),
            rev: "3zzzzzzzzzzzz".into(),
            ..world.recs.last().unwrap().clone()
        };
        world.recs.push(moved);
        let conference = world.conference();
        assert!(!conference.banned.contains(BRAM));
        assert_eq!(conference.standing(BRAM).ban, None);
    }

    #[test]
    fn code_limits_count_distinct_dids() {
        let secrets = Secrets::for_tests();
        let hash = secrets.code_hmac("crew");
        let mut world = World::new(&["code"]);
        world.decide(
            KEES,
            Rank::Owner,
            CODE,
            "",
            json!({ "codeHash": hash, "personal": false, "maxUses": 2 }),
        );
        world.join(ANA, json!({ "via": "code", "code": "crew" }));
        world.join(ANA, json!({ "via": "code", "code": "crew" }));
        let none = BTreeSet::new();
        let conference = world.conference();
        assert!(conference.code_admits(&hash, BRAM, T0, &none), "Ana's two joins are one use");
        assert!(!conference.code_admits(&hash, BRAM, T0, &BTreeSet::from([PIM.to_owned()])));
        assert!(
            conference.code_admits(&hash, ANA, T0, &BTreeSet::from([PIM.to_owned()])),
            "she can rejoin"
        );
        // Revoked by an admin of its rank, it admits no one.
        world.decide(OLGA, Rank::SuperAdmin, CODE_REVOKE, "", json!({ "codeHash": hash }));
        assert!(!world.conference().code_admits(&hash, ANA, T0, &none));
    }

    #[test]
    fn the_index_derives_from_admin_records_joins_leaves_roles_and_rules_only() {
        let conference = SpaceUri::new(ORG, CONFERENCE_TYPE, "3conf");
        let intake = SpaceUri::new(ORG, INTAKE_TYPE, "3conf");
        let admin = SpaceUri::admin(ORG);
        assert!(derives_from(&admin, "app.eventside.admin.anything"));
        assert!(derives_from(&intake, JOIN) && derives_from(&intake, LEAVE));
        assert!(!derives_from(&intake, "com.example.junk"));
        assert!(derives_from(&conference, ROLE) && derives_from(&conference, RULES));
        assert!(!derives_from(&conference, "community.lexicon.calendar.event"));
        assert!(!derives_from(&SpaceUri::new(ORG, ADMIN_TYPE, "other"), ADMIN));
    }

    #[test]
    fn a_role_is_the_latest_signed_one_and_counts_from_when_it_took_effect() {
        let mut world = World::new(&["code"]);
        let conf = space(CONFERENCE_TYPE);
        world.sign(
            &conf,
            OLGA,
            ROLE,
            ANA,
            Rank::Owner,
            json!({ "subject": ANA, "role": "speaker" }),
        );
        world.sign(
            &conf,
            OLGA,
            ROLE,
            ANA,
            Rank::Owner,
            json!({ "subject": ANA, "role": "staff", "since": iso(T0 + 40_000) }),
        );
        // Written by anyone else, or unsigned, a role doesn't count.
        world.sign(
            &conf,
            PIM,
            ROLE,
            BRAM,
            Rank::Staff,
            json!({ "subject": BRAM, "role": "owner" }),
        );
        let org = world.org();
        let conference = org.conference(&conf).unwrap();
        assert_eq!(conference.roles.get(ANA).map(String::as_str), Some("staff"));
        assert_eq!(conference.role_at(ANA, T0 + 30_000), None);
        assert_eq!(conference.role_at(ANA, T0 + 45_000), Some("staff"));
        assert!(!conference.roles.contains_key(BRAM));
        assert!(conference.was_admin_at(&org, OLGA, 1));
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
