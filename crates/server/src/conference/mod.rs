//! Conferences: one private space per conference, membership decided through
//! `decide`, and the admin CLI (see features/conference-space.md).
//!
//! Stubs for now, so the red tests fail on "not implemented" rather than on a
//! missing route or a CLI that starts the server instead.
//!
//! **Test hook (TC-40).** When the environment has
//! `EVENTSIDE_HALT_AFTER_DECISION=1`, the process that commits a decision (the
//! server or the CLI) exits with code 86 right after the decision's database
//! transaction commits, before any outbox entry is applied. The outbox worker
//! in the server must then apply it when the server starts again.

pub mod api;
pub mod cli;

/// The exit code a process uses when `EVENTSIDE_HALT_AFTER_DECISION` stops it.
pub const HALT_AFTER_DECISION_EXIT: u8 = 86;
