//! Candidate-oriented ingest (experimental, deprecated).
//!
//! The full review-manifest workflow is preserved in `akr-core::ingest` for
//! reference, but the CLI surface is deprecated in favor of `akr source *`
//! (immutable source library). These stubs keep the crate compiling and
//! return a helpful error directing users to `akr source add`.

use crate::commands::Output;
use crate::session::{EnvError, Session};
use akr_core::ingest::TableMode;
use std::path::Path;

fn deprecated() -> EnvError {
    EnvError::new(
        "AKR-C004",
        "akr ingest is experimental and deprecated; use `akr source add <path> [--id <id>]` to register immutable sources in sources/",
    )
}

/// `akr ingest preview ...` — deprecated stub.
pub fn preview(
    _session: &Session,
    _path: &Path,
    _source_kind: &str,
    _tables: TableMode,
) -> Result<Output, EnvError> {
    Err(deprecated())
}

/// `akr ingest start ...` — deprecated stub.
pub fn start(
    _session: &Session,
    _path: &Path,
    _source_kind: &str,
    _tables: TableMode,
) -> Result<Output, EnvError> {
    Err(deprecated())
}

/// `akr ingest show ...` — deprecated stub.
pub fn show(
    _session: &Session,
    _ingest_id: &str,
    _pending_only: bool,
    _limit: Option<usize>,
) -> Result<Output, EnvError> {
    Err(deprecated())
}

/// `akr ingest mark ...` — deprecated stub.
#[allow(clippy::too_many_arguments)]
pub fn mark(
    _session: &Session,
    _ingest_id: &str,
    _candidate_id: &str,
    _disposition: &str,
    _basis: &[String],
    _target: Option<&str>,
    _promote_kind: &Option<String>,
    _promote_target: Option<&String>,
    _promote_attach_source: bool,
    _relations: &[String],
    _note: Option<&str>,
    _base_version: Option<usize>,
) -> Result<Output, EnvError> {
    Err(deprecated())
}

/// `akr ingest apply ...` — deprecated stub.
pub fn apply(
    _session: &Session,
    _ingest_id: &str,
    _base_version: Option<usize>,
    _dry_run: bool,
) -> Result<Output, EnvError> {
    Err(deprecated())
}

/// `akr ingest close ...` — deprecated stub.
pub fn close(
    _session: &Session,
    _ingest_id: &str,
    _base_version: Option<usize>,
) -> Result<Output, EnvError> {
    Err(deprecated())
}
