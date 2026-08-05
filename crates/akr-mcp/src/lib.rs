//! `akr-mcp`: the `knowledge.*` tools of `docs/08-mcp.md`, over stdio.
//!
//! # One implementation
//!
//! Every tool here calls a function in `akr-cli`, which is the same function the `akr`
//! binary calls. §1 states the invariant:
//!
//! > `knowledge.context` and `akr context` call the same function with the same arguments
//! > and produce the same bundle. There is no MCP-specific assembly, ranking, or
//! > filtering. A behaviour that cannot be reproduced from the command line is a bug.
//!
//! This crate therefore contains no ledger logic at all. What it contains is an adapter:
//! JSON-RPC framing, the tool schemas, argument translation, and the error-class mapping
//! of §5. Those are the only places drift can enter, and they are what
//! `tests/differential.rs` checks.
//!
//! # No privileged access
//!
//! The server has exactly the capabilities the command line has. It opens the same
//! workspace, runs the same pipeline, and writes through the same atomic pipeline of
//! `docs/07-cli.md` §4. It cannot skip validation, cannot write an unformatted record, and
//! cannot read anything an operator could not read by running a command. In particular it
//! never opens the SQLite cache: D-019, restated in §6.

pub mod errors;
pub mod protocol;
pub mod record;
pub mod schema;
pub mod tools;

pub use protocol::{Server, serve};
