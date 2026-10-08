//! Acting as an admin: writing records into an admin's own repo (public, or
//! in one of our spaces) under the cookieless session they connected with,
//! signed by our host when they're permission records, then reading the
//! write into the index at once.

use reqwest::Method;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::pds::{PdsClient, PdsError};
use crate::auth::session;
use crate::crypto::tid_now;
use crate::spacehost::attest::{self, Check, Claim, Signer};
use crate::spacehost::{SpaceUri, sync};

/// How a write is signed: who decided it (the writer, unless another admin
/// did, as with a role the conference's super admin writes for them), what
/// it decides about, and the check it must pass when it's signed.
#[derive(Default)]
pub struct Signing<'a> {
    pub by: Option<&'a str>,
    pub claim: Claim,
    pub check: Option<Check<'a>>,
}

/// The conference space a public record is signed for: a sidecar's.
fn public_attest_space(collection: &str, value: &Value) -> Option<String> {
    (collection == super::SIDECAR)
        .then(|| value.get("space").and_then(Value::as_str).map(str::to_owned))
        .flatten()
}

/// An admin, with the session they connected with.
pub struct Acting {
    pub did: String,
    pub handle: String,
    id_hash: String,
}

/// The command that connects (or reconnects) an admin.
pub fn connect_command(handle: &str) -> String {
    format!("conference-server admin connect {handle}")
}

impl Acting {
    /// The admin's live session, refusing one that's missing a permission
    /// admins are now asked for.
    pub async fn new(state: &AppState, did: &str, handle: &str) -> Result<Self, String> {
        let id_hash = sqlx::query_scalar::<_, String>(
            "SELECT id_hash FROM sessions WHERE did = $1 AND kind = 'admin' AND ended_at IS NULL \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(did)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "{handle} isn't connected as an admin. Run `{}` first.",
                connect_command(handle)
            )
        })?;
        let row = session::load(&state.db, &id_hash).await.map_err(|e| e.to_string())?.ok_or_else(
            || {
                format!(
                    "{handle} isn't connected as an admin. Run `{}` first.",
                    connect_command(handle)
                )
            },
        )?;
        if session::outdated(state, &row) {
            return Err(format!(
                "{handle} needs to reconnect: admins are now asked for more permissions. Run `{}`.",
                connect_command(handle)
            ));
        }
        Ok(Self { did: did.to_owned(), handle: handle.to_owned(), id_hash })
    }

    async fn client(&self, state: &AppState) -> Result<PdsClient, String> {
        PdsClient::for_session(state, &self.id_hash).await.map_err(|e| match e {
            PdsError::SessionExpired => format!(
                "{}'s admin session has ended. Run `{}` to reconnect.",
                self.handle,
                connect_command(&self.handle)
            ),
            PdsError::Unavailable(why) => {
                format!("{}'s PDS couldn't be reached: {why}", self.handle)
            }
        })
    }

    /// An XRPC procedure at the admin's PDS, as them.
    async fn procedure(&self, state: &AppState, nsid: &str, body: &Value) -> Result<Value, String> {
        let client = self.client(state).await?;
        let res =
            client.send(Method::POST, &format!("/xrpc/{nsid}"), Some(body)).await.map_err(|e| {
                match e {
                    PdsError::SessionExpired => {
                        format!("{}'s admin session has ended", self.handle)
                    }
                    PdsError::Unavailable(why) => {
                        format!("{}'s PDS couldn't be reached: {why}", self.handle)
                    }
                }
            })?;
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("{}'s PDS refused {nsid} ({status}): {text}", self.handle));
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// Writes a record (`$type` and `createdAt` added), signed first when
    /// it's one our host signs: into the admin's repo in `space`, or (with
    /// `space` `None`) their public repo, signed for `attest_space`. The
    /// signing is checked and reserved first, and committed only once the
    /// record is written and read back.
    #[allow(clippy::too_many_arguments)]
    async fn write(
        &self,
        state: &AppState,
        nsid: &str,
        space: Option<&str>,
        attest_space: Option<&str>,
        collection: &str,
        rkey: &str,
        value: Value,
        signing: Signing<'_>,
    ) -> Result<(), String> {
        let mut record = typed(collection, value);
        let ticket = match attest_space {
            Some(attest_space) => {
                // An edited record's old signatures don't fit it any more.
                if let Some(map) = record.as_object_mut() {
                    map.remove("signatures");
                }
                let authority = SpaceUri::parse(attest_space)
                    .ok_or_else(|| format!("{attest_space} isn't a space"))?
                    .authority;
                let by = signing.by.unwrap_or(&self.did);
                let ticket = attest::reserve(
                    state,
                    &authority,
                    Signer::Admin(by),
                    &signing.claim,
                    signing.check,
                    space.map(|space| (space, self.did.as_str())),
                )
                .await?;
                if let Err(why) = ticket.sign(&mut record, attest_space, &self.did) {
                    attest::finish(state, &ticket, false).await;
                    return Err(why);
                }
                Some(ticket)
            }
            None => None,
        };
        let mut body =
            json!({ "repo": self.did, "collection": collection, "rkey": rkey, "record": record });
        if let Some(space) = space {
            body["space"] = json!(space);
        }
        let written = self.procedure(state, nsid, &body).await;
        // Read back, or the next check wouldn't see it: until it's in the
        // index, its journal entry stays pending, blocking another decision
        // about the same person (until the write notification brings it in,
        // or the entry lapses).
        let indexed = match (written.is_ok(), space) {
            (true, Some(space)) => self.synced(state, space).await,
            _ => true,
        };
        if let Some(ticket) = &ticket
            && (written.is_err() || indexed)
        {
            attest::finish(state, ticket, written.is_ok()).await;
        }
        written.map(drop)
    }

    /// Creates a record in the admin's repo in a space (a new TID rkey unless
    /// given), signed if it's one our host signs, and reads it into the
    /// index. Returns its rkey.
    pub async fn create_in(
        &self,
        state: &AppState,
        space: &str,
        collection: &str,
        rkey: Option<&str>,
        value: Value,
    ) -> Result<String, String> {
        self.create_signed(state, space, collection, rkey, value, Signing::default()).await
    }

    /// [`Self::create_in`], with the decider, claim and check its signing
    /// takes.
    pub async fn create_signed(
        &self,
        state: &AppState,
        space: &str,
        collection: &str,
        rkey: Option<&str>,
        value: Value,
        signing: Signing<'_>,
    ) -> Result<String, String> {
        let rkey = rkey.map_or_else(tid_now, str::to_owned);
        let attest_space = attest::signed_in(space, collection).then_some(space);
        self.write(
            state,
            "com.atproto.space.createRecord",
            Some(space),
            attest_space,
            collection,
            &rkey,
            value,
            signing,
        )
        .await?;
        Ok(rkey)
    }

    /// Writes a record at a known rkey in the admin's repo in a space,
    /// signed if it's one our host signs.
    pub async fn put_in(
        &self,
        state: &AppState,
        space: &str,
        collection: &str,
        rkey: &str,
        value: Value,
    ) -> Result<(), String> {
        self.put_signed(state, space, collection, rkey, value, Signing::default()).await
    }

    /// [`Self::put_in`], with the decider, claim and check its signing takes.
    pub async fn put_signed(
        &self,
        state: &AppState,
        space: &str,
        collection: &str,
        rkey: &str,
        value: Value,
        signing: Signing<'_>,
    ) -> Result<(), String> {
        let attest_space = attest::signed_in(space, collection).then_some(space);
        self.write(
            state,
            "com.atproto.space.putRecord",
            Some(space),
            attest_space,
            collection,
            rkey,
            value,
            signing,
        )
        .await
    }

    /// Writes a record exactly as given (already signed), at a known rkey in
    /// the admin's repo in a space, or their public repo without one.
    pub async fn put_as_is(
        &self,
        state: &AppState,
        space: Option<&str>,
        collection: &str,
        rkey: &str,
        record: Value,
    ) -> Result<(), String> {
        let mut body =
            json!({ "repo": self.did, "collection": collection, "rkey": rkey, "record": record });
        let nsid = match space {
            Some(space) => {
                body["space"] = json!(space);
                "com.atproto.space.putRecord"
            }
            None => "com.atproto.repo.putRecord",
        };
        self.procedure(state, nsid, &body).await?;
        if let Some(space) = space {
            let _ = self.synced(state, space).await;
        }
        Ok(())
    }

    /// Deletes a record from the admin's repo in a space.
    pub async fn delete_in(
        &self,
        state: &AppState,
        space: &str,
        collection: &str,
        rkey: &str,
    ) -> Result<(), String> {
        let body =
            json!({ "space": space, "repo": self.did, "collection": collection, "rkey": rkey });
        self.procedure(state, "com.atproto.space.deleteRecord", &body).await?;
        let _ = self.synced(state, space).await;
        Ok(())
    }

    /// Deletes a public record from the admin's repo.
    pub async fn delete_public(
        &self,
        state: &AppState,
        collection: &str,
        rkey: &str,
    ) -> Result<(), String> {
        let body = json!({ "repo": self.did, "collection": collection, "rkey": rkey });
        self.procedure(state, "com.atproto.repo.deleteRecord", &body).await?;
        Ok(())
    }

    /// Writes a public record at a known rkey in the admin's repo. A
    /// conference sidecar is signed, for the conference space it names.
    pub async fn put_public(
        &self,
        state: &AppState,
        collection: &str,
        rkey: &str,
        value: Value,
    ) -> Result<(), String> {
        let attest_space = public_attest_space(collection, &value);
        self.write(
            state,
            "com.atproto.repo.putRecord",
            None,
            attest_space.as_deref(),
            collection,
            rkey,
            value,
            Signing::default(),
        )
        .await
    }

    /// Creates a public record at `rkey` in the admin's repo, returning its
    /// AT-URI. A conference sidecar is signed, for the conference space it
    /// names.
    pub async fn create_public(
        &self,
        state: &AppState,
        collection: &str,
        rkey: Option<&str>,
        value: Value,
    ) -> Result<String, String> {
        let rkey = rkey.map_or_else(tid_now, str::to_owned);
        let attest_space = public_attest_space(collection, &value);
        self.write(
            state,
            "com.atproto.repo.createRecord",
            None,
            attest_space.as_deref(),
            collection,
            &rkey,
            value,
            Signing::default(),
        )
        .await?;
        Ok(format!("at://{}/{collection}/{rkey}", self.did))
    }

    /// Reads the admin's repo in a space into the index, now that they've
    /// written to it. The PDS's write notification would too, a moment later.
    async fn synced(&self, state: &AppState, space: &str) -> bool {
        if let Err(why) = sync::sync_repo(state, space, &self.did).await {
            eprintln!(
                "warning: couldn't read {}'s records in {space} back yet: {why}",
                self.handle
            );
            return false;
        }
        true
    }
}

/// A record value with its `$type`.
fn typed(collection: &str, mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("$type".into(), json!(collection));
        object.entry("createdAt").or_insert_with(|| {
            json!(crate::spacehost::index::iso(crate::db::now_ms() as u64 * 1000))
        });
    }
    value
}
