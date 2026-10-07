//! Which records in a conference space count: the one rule `listRecords`
//! serves by, and later features (block actions, plans) judge ingest by.
//!
//! A record counts when its author was a member at its dated time, the
//! conference's rules let them write its collection then, and, for role and
//! rules records, when it's the conference's super admin's.
//!
//! The dated time is the record's commit, but never more than
//! [`index::BACKDATE_SLACK_US`] before our host first saw it. That first-seen
//! time is ours: another app judging by commit revisions alone agrees except
//! for a record whose commit claims to be more than that much older than
//! when it reached us.

use crate::spacehost::index::{self, Conference, Org};

/// Whether `did` may write `collection` at `us`, by the conference's rules:
/// anyone who was a member then, or for a collection kept for admins, an
/// admin of the organization then, or someone whose owner or staff role had
/// taken effect by then.
pub fn may_write(org: &Org, conference: &Conference, did: &str, collection: &str, us: u64) -> bool {
    if !conference.was_member_at(did, us) {
        return false;
    }
    match conference.writers_of(collection) {
        "admins" => {
            conference.was_admin_at(org, did, us)
                || matches!(conference.role_at(did, us), Some("owner" | "staff"))
        }
        _ => true,
    }
}

/// When a record in the conference space counts from, if it counts at all:
/// written by `repo` into `collection`, in a commit at `rev_us`, first seen
/// by our host at `seen_us`.
pub fn record_counts(
    org: &Org,
    conference: &Conference,
    repo: &str,
    collection: &str,
    rev_us: u64,
    seen_us: Option<u64>,
) -> Option<u64> {
    let us = index::conference_us(rev_us, seen_us);
    // Role and rules records count only from the conference's super admin,
    // and only while they're an admin, as the index has it.
    if matches!(collection, index::ROLE | index::RULES)
        && (repo != conference.super_admin() || !org.is_admin(repo))
    {
        return None;
    }
    may_write(org, conference, repo, collection, us).then_some(us)
}
