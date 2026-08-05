//! The one way a schema change could go wrong quietly.
//!
//! `meta.schema_version` drives invalidation: a cache whose recorded version differs from
//! the tool's is dropped and rebuilt. That works only if the version actually moves when
//! the schema does. Nothing in the compiler enforces that — the DDL is a string — so this
//! pins the DDL's hash and fails when it changes, which is the moment to decide whether
//! the version needs bumping.
//!
//! **When this fails**: read the diff to `spec/schema/index.sql`, bump
//! [`akr_core::store::SCHEMA_VERSION`] if the shape changed at all, and paste the new hash
//! below. A comment-only edit needs the hash updated and the version left alone.

use akr_core::store::{SCHEMA_SQL, SCHEMA_VERSION};

/// SHA-256 of `spec/schema/index.sql`, as `akr_core::hash::sha256` renders it.
const DDL_HASH: &str = "131768f63fba2f6251bb60614f880125dfe223353ec7dffc3afc4eb677068d10";

#[test]
fn the_ddl_has_not_changed_without_a_decision_about_the_schema_version() {
    let actual = akr_core::hash::sha256(SCHEMA_SQL.as_bytes()).to_hex();
    assert_eq!(
        actual, DDL_HASH,
        "spec/schema/index.sql changed. Decide whether SCHEMA_VERSION (currently \
         {SCHEMA_VERSION}) needs a bump, then update DDL_HASH in this test to {actual}"
    );
}
