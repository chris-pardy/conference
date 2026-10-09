//! Decisions (design review round 1): every admission, removal, ban, role
//! change, join and leave goes through [`decide_in`], inside one database
//! transaction that the CLI and the server serialize on ([`begin`]).
//!
//! 1. The subject's standing is read from the decisions log, in `seq` order.
//! 2. The actor's role now is checked, and the precedence table applied.
//! 3. The decision is appended with the next `seq`.
//! 4. An outbox entry is added, which writes (or deletes) the signed record
//!    in the organization's repo once the transaction commits.
//!
//! The log decides; the records are a signed, readable copy of it.
//!
//! **Precedence.** Ranks are owner > staff > self; a decision keeps the rank
//! it was made with, whatever happens to its maker's role later.
//!
//! | Actor | Subject's standing | Action | Allowed? |
//! |---|---|---|---|
//! | anyone | not a member | admit | yes, unless banned |
//! | self | member | leave | yes |
//! | self | removed by staff or self | rejoin | yes |
//! | self | removed by an owner | rejoin | no |
//! | self | banned | rejoin | no |
//! | staff | admitted by an owner | remove, ban, change role | yes: staff may tighten |
//! | staff | removed or banned by an owner | admit, unban | no: staff can't loosen |
//! | staff | an owner or staff member | any | no: only owners act on admins |
//! | owner | any | any | yes |
//! | anyone | the last owner | remove, ban, demote, leave | no |

use std::collections::BTreeMap;
use std::fmt;

use sqlx::{Any, Transaction};

use crate::AppState;
use crate::db::{Backend, Db, now_ms};

/// A role in a conference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Staff,
    Speaker,
    Attendee,
}

impl Role {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(Self::Owner),
            "staff" => Some(Self::Staff),
            "speaker" => Some(Self::Speaker),
            "attendee" => Some(Self::Attendee),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Staff => "staff",
            Self::Speaker => "speaker",
            Self::Attendee => "attendee",
        }
    }

    /// Owners and staff: the `#organizers` audience.
    pub fn is_admin(self) -> bool {
        matches!(self, Self::Owner | Self::Staff)
    }

    /// The weight an admin in this role decides with.
    pub fn rank(self) -> Option<Rank> {
        match self {
            Self::Owner => Some(Rank::Owner),
            Self::Staff => Some(Rank::Staff),
            _ => None,
        }
    }
}

/// A decision's weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    /// The person deciding about themselves: joining, leaving.
    Own,
    Staff,
    Owner,
}

impl Rank {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "self" => Some(Self::Own),
            "staff" => Some(Self::Staff),
            "owner" => Some(Self::Owner),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Own => "self",
            Self::Staff => "staff",
            Self::Owner => "owner",
        }
    }
}

/// What's being decided about the subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Admitted, by a join method (`code`, `list`, `open`) or by an admin
    /// (`admin`), with a role.
    Admit {
        role: Role,
        method: String,
    },
    /// Removed by an admin.
    Remove,
    /// Left of their own accord.
    Leave,
    Ban,
    Unban,
    /// A member's role changed.
    SetRole(Role),
    /// Made an owner or staff member: admitted with the role, or, for a
    /// member, their role changed (`admin add`).
    Appoint(Role),
    /// No longer an owner or staff member, but still a member (`admin remove`).
    Dismiss,
}

impl Action {
    fn name(&self) -> &'static str {
        match self {
            Self::Admit { .. } => "admit",
            Self::Remove => "remove",
            Self::Leave => "leave",
            Self::Ban => "ban",
            Self::Unban => "unban",
            Self::SetRole(_) | Self::Appoint(_) | Self::Dismiss => "role",
        }
    }
}

/// Who decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    /// The subject, about themselves (joining, leaving).
    Subject,
    /// An admin of the conference, by DID: their role now sets their rank.
    Admin(String),
    /// Whoever creates a conference becomes its first owner, at owner rank.
    Creator(String),
}

/// One decision from the log.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Decision {
    pub seq: i64,
    pub subject: String,
    pub action: String,
    pub role: Option<String>,
    pub method: Option<String>,
    pub actor: String,
    pub rank: String,
    pub decided_at: i64,
}

impl Decision {
    fn rank(&self) -> Rank {
        Rank::parse(&self.rank).unwrap_or(Rank::Own)
    }
}

/// Where a person stands, from their decisions.
#[derive(Debug, Clone, Default)]
pub struct Standing {
    /// Their role, while they're a member.
    pub role: Option<Role>,
    /// The rank of the ban in force, if any.
    pub banned: Option<Rank>,
    /// The rank of the removal (or leaving, or ban) that ended their last
    /// membership, until they're admitted again or unbanned.
    pub removed: Option<Rank>,
    /// The decision that set their membership and role as they stand.
    pub membership: Option<Decision>,
    /// The ban in force.
    pub ban: Option<Decision>,
    /// The removal in force when the ban in force was placed, which lifting
    /// the ban restores: a ban never loosens a removal.
    removed_before_ban: Option<Rank>,
}

impl Standing {
    pub fn is_member(&self) -> bool {
        self.role.is_some()
    }

    fn apply(&mut self, d: &Decision) {
        let role = d.role.as_deref().and_then(Role::parse);
        match d.action.as_str() {
            "admit" => {
                self.role = Some(role.unwrap_or(Role::Attendee));
                self.removed = None;
                self.membership = Some(d.clone());
            }
            "role" => {
                if self.role.is_some() {
                    self.role = Some(role.unwrap_or(Role::Attendee));
                    self.membership = Some(d.clone());
                }
            }
            "remove" | "leave" => {
                if self.role.is_some() {
                    self.removed = Some(d.rank());
                }
                self.role = None;
                self.membership = None;
            }
            "ban" => {
                if self.banned.is_none() {
                    self.removed_before_ban = self.removed;
                }
                self.role = None;
                self.membership = None;
                self.banned = Some(d.rank());
                // The higher of the removal's and the ban's weight.
                self.removed = self.removed.max(Some(d.rank()));
                self.ban = Some(d.clone());
            }
            "unban" => {
                self.banned = None;
                self.removed = self.removed_before_ban.take();
                self.ban = None;
            }
            _ => {}
        }
    }
}

/// Everyone's standing in a conference, from its decisions (in `seq` order).
pub fn standings(decisions: &[Decision]) -> BTreeMap<String, Standing> {
    let mut out: BTreeMap<String, Standing> = BTreeMap::new();
    for d in decisions {
        out.entry(d.subject.clone()).or_default().apply(d);
    }
    out
}

/// One person's standing, from their decisions up to and including the
/// moment `at_ms` (all of them without it).
pub fn standing(decisions: &[Decision], subject: &str, at_ms: Option<i64>) -> Standing {
    let mut standing = Standing::default();
    for d in decisions {
        if d.subject == subject && at_ms.is_none_or(|at| d.decided_at <= at) {
            standing.apply(d);
        }
    }
    standing
}

/// The decisions of a conference, in order.
pub async fn decisions(db: &Db, conference: &str) -> Result<Vec<Decision>, String> {
    sqlx::query_as::<_, Decision>(
        "SELECT seq, subject, action, role, method, actor, rank, decided_at FROM decisions \
         WHERE conference = $1 ORDER BY seq",
    )
    .bind(conference)
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())
}

/// The decisions about one person, in order.
pub async fn decisions_about(
    db: &Db,
    conference: &str,
    subject: &str,
) -> Result<Vec<Decision>, String> {
    sqlx::query_as::<_, Decision>(
        "SELECT seq, subject, action, role, method, actor, rank, decided_at FROM decisions \
         WHERE conference = $1 AND subject = $2 ORDER BY seq",
    )
    .bind(conference)
    .bind(subject)
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())
}

/// A person's role in a conference now, if they're a member: what every
/// request inside the conference checks, never from a cached session.
pub async fn role(db: &Db, conference: &str, did: &str) -> Result<Option<Role>, String> {
    let decisions = decisions_about(db, conference, did).await?;
    Ok(standing(&decisions, did, None).role)
}

/// Whether a person is a member now.
pub async fn is_member_now(db: &Db, conference: &str, did: &str) -> Result<bool, String> {
    Ok(role(db, conference, did).await?.is_some())
}

/// Whether a person was a member at `at_ms` (when a record they wrote was
/// dated by their PDS).
pub async fn is_member_at(
    db: &Db,
    conference: &str,
    did: &str,
    at_ms: i64,
) -> Result<bool, String> {
    let decisions = decisions_about(db, conference, did).await?;
    Ok(standing(&decisions, did, Some(at_ms)).is_member())
}

/// Why a decision wasn't made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    NotAnAdmin,
    OnlyOwners,
    LastOwner,
    Banned,
    /// Removed by someone whose decision this actor can't loosen.
    RemovedByOwner,
    NotAMember,
    NotBanned,
    BannedByOwner,
    NotOrganizer,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotAnAdmin => "only an owner or staff member of the conference can do that",
            Self::OnlyOwners => "only owners can act on owners and staff, or make someone one",
            Self::LastOwner => "the conference must keep at least one owner",
            Self::Banned => "they're banned from the conference",
            Self::RemovedByOwner => "an owner removed them; only an owner can let them back in",
            Self::NotAMember => "they aren't a member",
            Self::NotBanned => "they aren't banned",
            Self::BannedByOwner => "an owner banned them; only an owner can lift it",
            Self::NotOrganizer => "they aren't an owner or staff member",
        })
    }
}

/// What `decide_in` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Appended to the log, as `seq`.
    Decided(i64),
    /// Nothing to change: it already stands.
    Unchanged,
    Refused(Refusal),
}

/// The transaction decisions are made in: `BEGIN IMMEDIATE` on SQLite, so
/// the CLI and the server (separate processes) take turns; an exclusive
/// lock on the log on Postgres.
pub async fn begin(state: &AppState) -> Result<Transaction<'static, Any>, String> {
    let backend = Backend::of(&state.config.database_url)?;
    match backend {
        Backend::Sqlite => state.db.begin_with("BEGIN IMMEDIATE").await.map_err(|e| e.to_string()),
        Backend::Postgres => {
            let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;
            sqlx::query("LOCK TABLE decisions IN SHARE ROW EXCLUSIVE MODE")
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            Ok(tx)
        }
    }
}

/// The next number in a table's `seq` column, inside the locked transaction.
pub async fn next_seq(tx: &mut Transaction<'static, Any>, table: &str) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>(&format!("SELECT COALESCE(MAX(seq), 0) + 1 FROM {table}"))
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| e.to_string())
}

/// Adds an outbox entry, inside the locked transaction.
pub async fn enqueue(
    tx: &mut Transaction<'static, Any>,
    conference: &str,
    kind: &str,
    subject: Option<&str>,
) -> Result<(), String> {
    let seq = next_seq(tx, "outbox").await?;
    sqlx::query(
        "INSERT INTO outbox (seq, conference, kind, subject, created_at, attempts) \
         VALUES ($1, $2, $3, $4, $5, 0)",
    )
    .bind(seq)
    .bind(conference)
    .bind(kind)
    .bind(subject)
    .bind(now_ms())
    .execute(&mut **tx)
    .await
    .map(drop)
    .map_err(|e| e.to_string())
}

/// Checks a decision against the subject's standing and the precedence
/// table, and records it with its outbox entry. Runs inside [`begin`]'s
/// transaction; the caller commits and then calls [`committed`].
pub async fn decide_in(
    tx: &mut Transaction<'static, Any>,
    conference: &str,
    subject: &str,
    action: Action,
    actor: &Actor,
) -> Result<Outcome, String> {
    let actor_did = match actor {
        Actor::Subject => subject,
        Actor::Admin(did) | Actor::Creator(did) => did.as_str(),
    };
    // The subject's and the actor's decisions, and those of anyone who has
    // ever been made an owner (to count the owners now).
    let all = sqlx::query_as::<_, Decision>(
        "SELECT seq, subject, action, role, method, actor, rank, decided_at FROM decisions \
         WHERE conference = $1 AND (subject = $2 OR subject = $3 OR subject IN \
         (SELECT subject FROM decisions WHERE conference = $1 AND role = 'owner')) ORDER BY seq",
    )
    .bind(conference)
    .bind(subject)
    .bind(actor_did)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    let everyone = standings(&all);
    let owners = everyone.values().filter(|s| s.role == Some(Role::Owner)).count();
    let none = Standing::default();
    let current = everyone.get(subject).unwrap_or(&none);
    let (actor_did, rank) = match actor {
        Actor::Subject => (subject.to_owned(), Rank::Own),
        Actor::Creator(did) => (did.clone(), Rank::Owner),
        Actor::Admin(did) => {
            let role = everyone.get(did).and_then(|s| s.role);
            match role.and_then(Role::rank) {
                Some(rank) => (did.clone(), rank),
                None => return Ok(Outcome::Refused(Refusal::NotAnAdmin)),
            }
        }
    };
    let decision = match precedence(current, &action, rank, owners) {
        Ok(Some(decision)) => decision,
        Ok(None) => return Ok(Outcome::Unchanged),
        Err(refusal) => return Ok(Outcome::Refused(refusal)),
    };
    let seq = next_seq(tx, "decisions").await?;
    let (role, method) = match &decision {
        Action::Admit { role, method } => (Some(role.as_str()), Some(method.as_str())),
        Action::SetRole(role) => (Some(role.as_str()), None),
        _ => (None, None),
    };
    sqlx::query(
        "INSERT INTO decisions (seq, conference, subject, action, role, method, actor, rank, decided_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(seq)
    .bind(conference)
    .bind(subject)
    .bind(decision.name())
    .bind(role)
    .bind(method)
    .bind(&actor_did)
    .bind(rank.as_str())
    .bind(now_ms())
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    enqueue(tx, conference, super::outbox::MEMBER_ENTRY, Some(subject)).await?;
    Ok(Outcome::Decided(seq))
}

/// The precedence table: the decision to record (`None` when it already
/// stands), or why not.
fn precedence(
    current: &Standing,
    action: &Action,
    rank: Rank,
    owners: usize,
) -> Result<Option<Action>, Refusal> {
    let subject_admin = current.role.is_some_and(Role::is_admin);
    let last_owner = current.role == Some(Role::Owner) && owners <= 1;
    // Only owners act on owners and staff (people deciding about
    // themselves aside).
    let admins_only = |rank: Rank| {
        if subject_admin && rank != Rank::Owner && rank != Rank::Own {
            Err(Refusal::OnlyOwners)
        } else {
            Ok(())
        }
    };
    match action {
        Action::Admit { role, .. } => {
            if role.is_admin() && rank != Rank::Owner {
                return Err(Refusal::OnlyOwners);
            }
            if current.banned.is_some() {
                return Err(Refusal::Banned);
            }
            // Already in: nothing to admit (a role is changed with `SetRole`).
            if current.role.is_some() {
                return Ok(None);
            }
            // Staff and the person themselves can't loosen an owner's removal.
            if current.removed == Some(Rank::Owner) && rank != Rank::Owner {
                return Err(Refusal::RemovedByOwner);
            }
            Ok(Some(action.clone()))
        }
        Action::Remove => {
            if current.role.is_none() {
                return Err(Refusal::NotAMember);
            }
            admins_only(rank)?;
            if last_owner {
                return Err(Refusal::LastOwner);
            }
            Ok(Some(Action::Remove))
        }
        Action::Leave => {
            if current.role.is_none() {
                return Ok(None);
            }
            if last_owner {
                return Err(Refusal::LastOwner);
            }
            Ok(Some(Action::Leave))
        }
        Action::Ban => {
            admins_only(rank)?;
            if last_owner {
                return Err(Refusal::LastOwner);
            }
            match current.banned {
                // Already banned with as much weight.
                Some(by) if by >= rank => Ok(None),
                _ => Ok(Some(Action::Ban)),
            }
        }
        Action::Unban => match current.banned {
            None => Err(Refusal::NotBanned),
            Some(Rank::Owner) if rank != Rank::Owner => Err(Refusal::BannedByOwner),
            Some(_) => Ok(Some(Action::Unban)),
        },
        Action::Appoint(role) => {
            if current.role.is_some() {
                precedence(current, &Action::SetRole(*role), rank, owners)
            } else {
                precedence(
                    current,
                    &Action::Admit { role: *role, method: "admin".into() },
                    rank,
                    owners,
                )
            }
        }
        Action::Dismiss => {
            if !subject_admin {
                return Err(Refusal::NotOrganizer);
            }
            precedence(current, &Action::SetRole(Role::Attendee), rank, owners)
        }
        Action::SetRole(role) => {
            let Some(now) = current.role else { return Err(Refusal::NotAMember) };
            if (subject_admin || role.is_admin()) && rank != Rank::Owner {
                return Err(Refusal::OnlyOwners);
            }
            if now == *role {
                return Ok(None);
            }
            if last_owner && *role != Role::Owner {
                return Err(Refusal::LastOwner);
            }
            Ok(Some(Action::SetRole(*role)))
        }
    }
}

/// After a decision's transaction commits: the TC-40 test hook, then the
/// outbox, which writes the decision's record.
pub fn committed() {
    if std::env::var("EVENTSIDE_HALT_AFTER_DECISION").is_ok_and(|v| v == "1") {
        eprintln!("EVENTSIDE_HALT_AFTER_DECISION: stopping before the outbox is applied");
        std::process::exit(i32::from(super::HALT_AFTER_DECISION_EXIT));
    }
}

/// Makes one decision in a transaction of its own, and commits it.
pub async fn decide(
    state: &AppState,
    conference: &str,
    subject: &str,
    action: Action,
    actor: &Actor,
) -> Result<Outcome, String> {
    let mut tx = begin(state).await?;
    let outcome = decide_in(&mut tx, conference, subject, action, actor).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    if matches!(outcome, Outcome::Decided(_)) {
        committed();
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(seq: i64, subject: &str, action: &str, role: Option<&str>, rank: &str) -> Decision {
        Decision {
            seq,
            subject: subject.into(),
            action: action.into(),
            role: role.map(str::to_owned),
            method: None,
            actor: "did:plc:actor".into(),
            rank: rank.into(),
            decided_at: seq * 1000,
        }
    }

    fn admit(role: Role) -> Action {
        Action::Admit { role, method: "admin".into() }
    }

    fn join() -> Action {
        Action::Admit { role: Role::Attendee, method: "code".into() }
    }

    fn standing_of(log: &[Decision]) -> Standing {
        standing(log, "s", None)
    }

    #[test]
    fn tc_28_staff_can_tighten_an_owners_admission() {
        let s = standing_of(&[d(1, "s", "admit", Some("attendee"), "owner")]);
        assert_eq!(precedence(&s, &Action::Remove, Rank::Staff, 1), Ok(Some(Action::Remove)));
        assert_eq!(precedence(&s, &Action::Ban, Rank::Staff, 1), Ok(Some(Action::Ban)));
    }

    #[test]
    fn tc_29_staff_cant_loosen_an_owners_removal_or_ban() {
        let removed = standing_of(&[
            d(1, "s", "admit", Some("attendee"), "self"),
            d(2, "s", "remove", None, "owner"),
        ]);
        assert_eq!(
            precedence(&removed, &admit(Role::Attendee), Rank::Staff, 1),
            Err(Refusal::RemovedByOwner)
        );
        assert!(precedence(&removed, &admit(Role::Attendee), Rank::Owner, 1).is_ok());
        let banned = standing_of(&[d(1, "s", "ban", None, "owner")]);
        assert_eq!(
            precedence(&banned, &Action::Unban, Rank::Staff, 1),
            Err(Refusal::BannedByOwner)
        );
        assert_eq!(precedence(&banned, &Action::Unban, Rank::Owner, 1), Ok(Some(Action::Unban)));
    }

    #[test]
    fn tc_30_rejoining_depends_on_who_removed_you() {
        let by_staff = standing_of(&[
            d(1, "s", "admit", Some("attendee"), "self"),
            d(2, "s", "remove", None, "staff"),
        ]);
        assert_eq!(precedence(&by_staff, &join(), Rank::Own, 1), Ok(Some(join())));
        let by_owner = standing_of(&[
            d(1, "s", "admit", Some("attendee"), "self"),
            d(2, "s", "remove", None, "owner"),
        ]);
        assert_eq!(precedence(&by_owner, &join(), Rank::Own, 1), Err(Refusal::RemovedByOwner));
    }

    #[test]
    fn tc_31_only_owners_act_on_admins() {
        let staff = standing_of(&[d(1, "s", "admit", Some("staff"), "owner")]);
        assert_eq!(precedence(&staff, &Action::Remove, Rank::Staff, 1), Err(Refusal::OnlyOwners));
        assert_eq!(precedence(&staff, &Action::Ban, Rank::Staff, 1), Err(Refusal::OnlyOwners));
        assert_eq!(
            precedence(&staff, &Action::SetRole(Role::Owner), Rank::Staff, 1),
            Err(Refusal::OnlyOwners)
        );
        assert_eq!(precedence(&staff, &Action::Remove, Rank::Owner, 1), Ok(Some(Action::Remove)));
    }

    #[test]
    fn tc_5_there_is_always_an_owner() {
        let owner = standing_of(&[d(1, "s", "admit", Some("owner"), "owner")]);
        for action in [Action::Remove, Action::Leave, Action::Ban, Action::SetRole(Role::Staff)] {
            assert_eq!(
                precedence(&owner, &action, Rank::Owner, 1),
                Err(Refusal::LastOwner),
                "{action:?}"
            );
        }
        assert_eq!(
            precedence(&owner, &Action::SetRole(Role::Staff), Rank::Owner, 2),
            Ok(Some(Action::SetRole(Role::Staff)))
        );
    }

    #[test]
    fn tc_32_a_demoted_owner_keeps_the_weight_of_their_owner_decisions() {
        // The ban was made at owner rank; its maker is staff now.
        let banned = standing_of(&[d(1, "s", "ban", None, "owner")]);
        assert_eq!(
            precedence(&banned, &Action::Unban, Rank::Staff, 1),
            Err(Refusal::BannedByOwner)
        );
    }

    #[test]
    fn tc_27_lifting_a_ban_lets_someone_join_but_doesnt_join_them() {
        let log = [
            d(1, "s", "admit", Some("attendee"), "self"),
            d(2, "s", "ban", None, "owner"),
            d(3, "s", "unban", None, "owner"),
        ];
        let s = standing_of(&log);
        assert!(!s.is_member());
        assert_eq!(precedence(&s, &join(), Rank::Own, 1), Ok(Some(join())));
    }

    #[test]
    fn staff_cant_loosen_an_owners_removal_by_banning_and_unbanning() {
        let mut log = vec![
            d(1, "s", "admit", Some("attendee"), "self"),
            d(2, "s", "remove", None, "owner"),
            d(3, "s", "ban", None, "staff"),
        ];
        let banned = standing_of(&log);
        assert_eq!(banned.removed, Some(Rank::Owner));
        assert_eq!(precedence(&banned, &Action::Unban, Rank::Staff, 1), Ok(Some(Action::Unban)));
        log.push(d(4, "s", "unban", None, "staff"));
        let unbanned = standing_of(&log);
        assert_eq!(unbanned.removed, Some(Rank::Owner));
        assert_eq!(precedence(&unbanned, &join(), Rank::Own, 1), Err(Refusal::RemovedByOwner));
        assert_eq!(
            precedence(&unbanned, &admit(Role::Attendee), Rank::Staff, 1),
            Err(Refusal::RemovedByOwner)
        );
    }

    #[test]
    fn tc_26_someone_can_be_banned_before_they_join() {
        let s = standing_of(&[]);
        assert_eq!(precedence(&s, &Action::Ban, Rank::Owner, 1), Ok(Some(Action::Ban)));
        let banned = standing_of(&[d(1, "s", "ban", None, "owner")]);
        assert_eq!(precedence(&banned, &join(), Rank::Own, 1), Err(Refusal::Banned));
    }

    #[test]
    fn tc_41_membership_is_read_at_a_moment() {
        let log = [
            d(1, "s", "admit", Some("attendee"), "self"),
            d(2, "s", "remove", None, "staff"),
            d(4, "s", "admit", Some("attendee"), "staff"),
        ];
        assert!(standing(&log, "s", Some(1500)).is_member());
        assert!(!standing(&log, "s", Some(3000)).is_member());
        assert!(standing(&log, "s", Some(4500)).is_member());
        assert!(!standing(&log, "s", Some(500)).is_member());
    }
}
