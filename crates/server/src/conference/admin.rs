//! Acting as an admin: writing records into an admin's own repo (public, or
//! in one of our spaces) under the cookieless session they connected with,
//! then reading the write into the index at once.

use reqwest::Method;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::pds::{PdsClient, PdsError};
use crate::auth::session;
use crate::crypto::tid_now;
use crate::spacehost::sync;

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

    /// Creates a record in the admin's repo in a space (a new TID rkey unless
    /// given), and reads it into the index. Returns its rkey.
    pub async fn create_in(
        &self,
        state: &AppState,
        space: &str,
        collection: &str,
        rkey: Option<&str>,
        value: Value,
    ) -> Result<String, String> {
        let rkey = rkey.map_or_else(tid_now, str::to_owned);
        let body = json!({
            "space": space,
            "repo": self.did,
            "collection": collection,
            "rkey": rkey,
            "record": typed(collection, value),
        });
        self.procedure(state, "com.atproto.space.createRecord", &body).await?;
        self.synced(state, space).await;
        Ok(rkey)
    }

    /// Writes a record at a known rkey in the admin's repo in a space.
    pub async fn put_in(
        &self,
        state: &AppState,
        space: &str,
        collection: &str,
        rkey: &str,
        value: Value,
    ) -> Result<(), String> {
        let body = json!({
            "space": space,
            "repo": self.did,
            "collection": collection,
            "rkey": rkey,
            "record": typed(collection, value),
        });
        self.procedure(state, "com.atproto.space.putRecord", &body).await?;
        self.synced(state, space).await;
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
        self.synced(state, space).await;
        Ok(())
    }

    /// Creates a public record in the admin's repo, returning its AT-URI.
    pub async fn create_public(
        &self,
        state: &AppState,
        collection: &str,
        rkey: Option<&str>,
        value: Value,
    ) -> Result<String, String> {
        let mut body = json!({ "repo": self.did, "collection": collection, "record": typed(collection, value) });
        if let Some(rkey) = rkey {
            body["rkey"] = json!(rkey);
        }
        let out = self.procedure(state, "com.atproto.repo.createRecord", &body).await?;
        out.get("uri")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "createRecord returned no uri".into())
    }

    /// Reads the admin's repo in a space into the index, now that they've
    /// written to it. The PDS's write notification would too, a moment later.
    async fn synced(&self, state: &AppState, space: &str) {
        if let Err(why) = sync::sync_repo(state, space, &self.did).await {
            eprintln!(
                "warning: couldn't read {}'s records in {space} back yet: {why}",
                self.handle
            );
        }
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
