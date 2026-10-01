//! Spike: encrypted local database for the Rust Core.
//!
//! Verifies on every platform CI runs on:
//! * SQLCipher 4 raw-key encryption, wrong-key rejection, file is not plaintext
//! * WAL + foreign keys + STRICT tables
//! * migration engine with checksums, transactional apply, backup-before-migrate
//! * integrity checks on startup
//! * key lifecycle: DPAPI-protected key file + Recovery-Key restore on a new PC

pub mod migrate;
pub mod store;

pub use migrate::{Migration, MIGRATIONS};
pub use store::{Database, KeyFiles, StoreError};
