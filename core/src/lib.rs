//! Artaveo Dental Rust Core (ADR-01): every business rule, every database
//! access, authentication, permissions and audit live here. Transports
//! (Tauri IPC, LAN, dev HTTP) only call [`Core::handle`].

pub mod api;
pub mod appointment;
pub mod attachment;
pub mod audit;
pub mod auth;
pub mod backup;
pub mod calendar;
pub mod catalog;
pub mod clinic;
pub mod clock;
pub mod config;
pub mod db;
pub mod documents;
pub mod draft;
pub mod error;
pub mod formulary;
pub mod ids;
pub mod import;
pub mod keys;
pub mod logging;
pub mod medical;
pub mod normalize;
pub mod patient;
pub mod recall;
pub mod scheduling;
pub mod seeds;
pub mod session;
pub mod settings;
pub mod specialty;
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

/// Bulk synthetic patients (and optionally appointments) for the seed tool and performance
/// tests (Phase 3/4 NFR: search under 200 ms and profile under 500 ms with 100 000 patients).
/// One transaction, so 100 000 rows take seconds instead of minutes of fsyncs.
#[doc(hidden)]
pub fn seeds_bulk_patients(core: &Core, patients: usize, appointments: usize) -> error::Result<()> {
    use artaveo_shared::CreatePatientParams;
    const NAMES: [&str; 6] = ["احمد خان", "زرغونه", "کریم داد", "فاطمه", "نجیب الله", "صدیقه"];
    core.with_db(|o| {
        let tx = o.conn.transaction()?;
        let actor = audit::Actor::system();
        let mut ids = Vec::with_capacity(patients);
        for i in 0..patients {
            let p = patient::create_patient(
                &tx,
                &actor,
                &CreatePatientParams {
                    full_name: format!("{} {i}", NAMES[i % NAMES.len()]),
                    phone: Some(format!("07{:08}", i % 100_000_000)),
                    allow_duplicate: true,
                    ..Default::default()
                },
            )?;
            ids.push(p.id);
        }
        if appointments > 0 && !ids.is_empty() {
            let now = clock::now_iso();
            let doctor = ids::new_id();
            let chair = ids::new_id();
            tx.execute(
                "INSERT INTO doctor(id, full_name, color, created_at, updated_at) VALUES (?1, 'Seed Doctor', '#0e7490', ?2, ?2)",
                rusqlite::params![doctor, now],
            )?;
            tx.execute(
                "INSERT INTO chair(id, name, created_at, updated_at) VALUES (?1, 'Seed Chair', ?2, ?2)",
                rusqlite::params![chair, now],
            )?;
            let mut st = tx.prepare(
                "INSERT INTO appointment(id, patient_id, doctor_id, chair_id, start_at, end_at, local_date, start_min, end_min,
                    status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'completed', ?10, ?10)",
            )?;
            let base = time::macros::date!(2024 - 01 - 01);
            for i in 0..appointments {
                // 16 half-hour slots a day, from 08:00.
                let day = base + time::Duration::days((i / 16) as i64);
                let start_min = 8 * 60 + (i % 16) as i64 * 30;
                let date = scheduling::format_date(day);
                let start = format!("{date}T{:02}:{:02}:00.000Z", start_min / 60, start_min % 60);
                let end = format!("{date}T{:02}:{:02}:00.000Z", (start_min + 30) / 60, (start_min + 30) % 60);
                st.execute(rusqlite::params![
                    ids::new_id(),
                    ids[i % ids.len()],
                    doctor,
                    chair,
                    start,
                    end,
                    date,
                    start_min,
                    start_min + 30,
                    now
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}
