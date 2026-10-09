//! `conference-server admin …`: the operator's CLI. It runs with the server's
//! environment (DATABASE_URL, PUBLIC_URL, ATPROTO_URL, …) against the same
//! database as the running server.
//!
//! Stub: every command prints "not implemented" and exits 1. The contract the
//! tests use is below; `--json` makes a command print one JSON object as the
//! last line of stdout. Commands that decide something take `--as <handle>`,
//! a connected admin, and exit non-zero when refused.

use std::process::ExitCode;

/// The commands and flags the tests rely on.
pub const USAGE: &str = "\
usage: conference-server admin <command> [--json]

  org connect <handle> [--plc-token <token>]
      Prints a URL to open. After the organization account signs in there,
      adds #atproto_space_host (this server), #atproto_space and
      #eventside_attest to its DID document by PLC operation, using the
      email token from requestPlcOperationSignature. --json: {did, handle}
  org key add --org <did> [--plc-token <token>]
      Adds another #eventside_attest… key. --json: {key}
  connect <handle>
      Connects an admin over OAuth: prints a URL to open, waits for sign-in.
  conference create --org <did> --as <handle> --name <name> --starts <iso>
      --ends <iso> --city <city> [--description <text>] [--theme <json>]
      The acting admin becomes the first owner. --json: {space, event}
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

pub async fn main(args: Vec<String>) -> ExitCode {
    if args.is_empty() || args.iter().any(|a| a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    eprintln!("not implemented: admin {}", args.join(" "));
    ExitCode::FAILURE
}
