//! Artaveo Dental Rust Core (ADR-01): every business rule, every database
//! access, authentication, permissions and audit live here. Transports
//! (Tauri IPC, LAN, dev HTTP) only call [`Core::handle`].

pub mod api;
pub mod audit;
pub mod auth;
pub mod backup;
pub mod calendar;
pub mod clinic;
pub mod clock;
pub mod config;
pub mod db;
pub mod error;
pub mod ids;
pub mod keys;
pub mod logging;
pub mod normalize;
pub mod seeds;
pub mod session;
pub mod settings;
pub mod sysinfo;

mod core;
#[cfg(test)]
mod tests;

pub use crate::config::{Config, Environment};
pub use crate::core::Core;
pub use crate::error::CoreError;

/// SemVer of the whole application (workspace version, ADR: Application Versioning).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_COMMIT: &str = env!("ARTAVEO_GIT_COMMIT");

/// Bulk synthetic audit rows for the seed tool and performance tests.
#[doc(hidden)]
pub fn seeds_bulk_audit(core: &Core, n: usize) -> error::Result<()> {
    core.with_db(|o| {
        let tx = o.conn.transaction()?;
        {
            let mut st = tx.prepare(
                "INSERT INTO audit_log(at, user_id, username, action, entity, entity_id, new_value, computer)
                 VALUES (?1, NULL, 'seed', 'seed.synthetic', 'seed', ?2, '{}', 'seed')",
            )?;
            let at = clock::now_iso();
            for _ in 0..n {
                st.execute(rusqlite::params![at, ids::new_id()])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}
