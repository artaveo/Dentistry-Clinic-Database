//! Method router: the one place where the API contract meets the Core.
//! Every method is authorised here (session, lock state, permission) before
//! any business code runs, so no transport can bypass Permission or Audit.

use std::fs;
use std::sync::atomic::Ordering;
use std::time::Instant;

use artaveo_shared::{methods as m, *};
use rusqlite::{params, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::audit::{self, Actor};
use crate::auth::{self, perm, NewUser, OWNER};
use crate::backup;
use crate::clinic;
use crate::clock::now_iso;
use crate::core::Core;
use crate::db::{self, KeyFiles};
use crate::error::{CoreError, Result};
use crate::ids::new_id;
use crate::keys::RecoveryKey;
use crate::session::{state_of, Session};
use crate::settings;
use crate::sysinfo;
use crate::{APP_VERSION, GIT_COMMIT};

/// Methods still allowed while the screen is locked.
const ALLOWED_WHEN_LOCKED: &[&str] = &[m::SESSION_STATE, m::SESSION_UNLOCK, m::AUTH_LOGOUT];

fn params<P: DeserializeOwned>(v: Value) -> Result<P> {
    let v = if v.is_null() { json!({}) } else { v };
    serde_json::from_value(v).map_err(|e| CoreError::validation(format!("invalid params: {e}")))
}

fn ok<T: Serialize>(v: T) -> Result<Value> {
    Ok(serde_json::to_value(v)?)
}

fn actor(s: &Session) -> Actor {
    Actor::user(&s.user.id, &s.user.username)
}

/// Index into `seeds::LANGS` (`["fa", "ps", "en"]`) and `GeoRow::labels`.
fn lang_index(language: Language) -> usize {
    match language {
        Language::Fa => 0,
        Language::Ps => 1,
        Language::En => 2,
    }
}

/// Serves `geo.provinces`/`geo.districts` straight from the embedded seed
/// CSVs (no database): used only before Setup (see `call()`). Ids match
/// `ids::seed_id`, the same ones the database uses once it exists, so a
/// province/district picked during Setup still resolves after the clinic
/// database is created and seeded.
fn geo_from_seeds(method: &str, p: Value) -> Result<Value> {
    match method {
        m::GEO_PROVINCES => {
            let LanguageParams { language } = params(p)?;
            let idx = lang_index(language);
            let mut rows = crate::seeds::province_rows()?;
            rows.sort_by_key(|r| r.sort);
            let v: Vec<LabeledItem> = rows
                .into_iter()
                .map(|r| LabeledItem {
                    id: province_seed_id(&r.code),
                    code: r.code,
                    label: r.labels[idx].clone(),
                })
                .collect();
            ok(v)
        }
        m::GEO_DISTRICTS => {
            let DistrictListParams { province_id, language } = params(p)?;
            let idx = lang_index(language);
            let mut rows = crate::seeds::district_rows()?;
            rows.sort_by_key(|r| r.sort);
            let v: Vec<LabeledItem> = rows
                .into_iter()
                .filter(|r| province_seed_id(r.parent.as_deref().unwrap_or_default()) == province_id)
                .map(|r| LabeledItem {
                    id: crate::ids::seed_id("district", &r.code),
                    code: r.code,
                    label: r.labels[idx].clone(),
                })
                .collect();
            ok(v)
        }
        _ => unreachable!("geo_from_seeds is only called for the two geo methods"),
    }
}

fn province_seed_id(code: &str) -> String {
    crate::ids::seed_id("province", code)
}

const MAX_LOGO_BYTES: usize = 2 * 1024 * 1024;

/// Decodes and sanity-checks a wizard-uploaded logo. Returns the file
/// extension to store it under (never the client-supplied path) and the
/// raw bytes, or `None` if no logo was provided.
fn decode_logo(base64: Option<&str>, file_name: Option<&str>) -> Result<Option<(&'static str, Vec<u8>)>> {
    let Some(b64) = base64.filter(|s| !s.is_empty()) else { return Ok(None) };
    let ext = match file_name.and_then(|n| n.rsplit('.').next()).map(|e| e.to_ascii_lowercase()) {
        Some(e) if e == "png" => "png",
        Some(e) if e == "jpg" || e == "jpeg" => "jpg",
        Some(e) if e == "webp" => "webp",
        _ => {
            return Err(CoreError::invalid(
                "logo",
                ValidationRule::LogoType,
                "logo must be a .png, .jpg or .webp file",
            ))
        }
    };
    let bytes = data_encoding::BASE64.decode(b64.as_bytes()).map_err(|e| {
        CoreError::invalid("logo", ValidationRule::LogoType, format!("invalid logo data: {e}"))
    })?;
    if bytes.is_empty() || bytes.len() > MAX_LOGO_BYTES {
        return Err(CoreError::invalid("logo", ValidationRule::LogoSize, "logo must be 1 byte – 2 MiB"));
    }
    Ok(Some((ext, bytes)))
}

impl Core {
    pub(crate) fn call(&self, req: RpcRequest) -> Result<Value> {
        let RpcRequest { method, params: p, token } = req;
        let method = method.as_str();
        if PUBLIC_METHODS.contains(&method) {
            return self.call_public(method, p);
        }
        if !ALL_METHODS.contains(&method) {
            return Err(CoreError::api(ErrorCode::UnknownMethod, method.to_string()));
        }
        if !self.is_set_up() {
            // The Setup Wizard's Clinic Info step needs the province/district
            // list before the clinic database (which normally serves it)
            // exists; it is system seed data, not clinic data, so it can be
            // read straight from the embedded CSVs with no session either.
            if matches!(method, m::GEO_PROVINCES | m::GEO_DISTRICTS) {
                return geo_from_seeds(method, p);
            }
            return Err(CoreError::api(ErrorCode::NotSetUp, "clinic is not set up"));
        }
        let token = token.unwrap_or_default();
        let session = self.authorize(&token, method)?;
        self.call_private(method, p, &token, session)
    }

    /// Resolves the session, applies idle-lock and records activity.
    fn authorize(&self, token: &str, method: &str) -> Result<Session> {
        let timeout = self.session_timeout();
        let mut store = self.lock_sessions();
        let s = store.get(Some(token).filter(|t| !t.is_empty()), timeout)?;
        if s.locked && !ALLOWED_WHEN_LOCKED.contains(&method) {
            return Err(CoreError::api(ErrorCode::SessionLocked, "screen is locked"));
        }
        if !s.locked && method != m::SESSION_STATE {
            s.last_activity = Instant::now();
        }
        Ok(s.clone())
    }

    fn call_public(&self, method: &str, p: Value) -> Result<Value> {
        match method {
            m::APP_STATUS => ok(AppStatus {
                state: if self.is_set_up() { AppState::Ready } else { AppState::NeedsSetup },
                version: APP_VERSION.into(),
                environment: self.config.environment.code().into(),
                clinic_name: self.clinic_name(),
                default_language: self.default_language(),
            }),
            m::APP_SETUP => self.setup(params(p)?),
            m::APP_CLINIC_LOGO => {
                let data_url = match self.lock_db().as_ref() {
                    Some(o) => clinic::logo_data_url(&o.conn)?,
                    None => None,
                };
                ok(ClinicLogo { data_url })
            }
            m::AUTH_LOGIN => {
                let LoginParams { username, password } = params(p)?;
                let (user, permissions) = self.with_db(|o| {
                    let user = auth::login(&o.conn, &username, &password)?;
                    let perms = auth::permissions_of_role(&o.conn, &user.role)?;
                    Ok((user, perms))
                })?;
                let token = self.lock_sessions().create(user.clone(), permissions.clone());
                let mut perms: Vec<String> = permissions.into_iter().collect();
                perms.sort();
                ok(SessionInfo {
                    token,
                    user,
                    permissions: perms,
                    timeout_minutes: self.timeout_minutes.load(Ordering::Relaxed),
                    locked: false,
                })
            }
            m::AUTH_RECOVER_OWNER => {
                let RecoverOwnerParams { recovery_key, new_password } = params(p)?;
                let owner_id = self.with_db(|o| {
                    let owner = auth::owner(&o.conn)?;
                    let valid = RecoveryKey::parse(&recovery_key)
                        .ok()
                        .and_then(|rk| {
                            let blob = fs::read(KeyFiles::beside(&self.config.db_path()).recovery).ok()?;
                            rk.unwrap(&blob).ok()
                        })
                        .is_some_and(|k| k == o.key);
                    if !valid {
                        audit::record(
                            &o.conn,
                            &Actor::system(),
                            "auth.recover_owner_failed",
                            Some("app_user"),
                            Some(&owner.id),
                            None,
                            None,
                        )?;
                        return Err(CoreError::api(
                            ErrorCode::RecoveryKeyInvalid,
                            "recovery key does not match this clinic",
                        )
                        .on_field("recovery_key", ValidationRule::RecoveryKey));
                    }
                    let actor = Actor::user(&owner.id, &owner.username);
                    auth::set_password(&o.conn, &actor, &owner.id, &new_password)
                        .map_err(|e| e.rename_field("new_password"))?;
                    audit::record(
                        &o.conn,
                        &actor,
                        "auth.recover_owner",
                        Some("app_user"),
                        Some(&owner.id),
                        None,
                        None,
                    )?;
                    Ok(owner.id)
                })?;
                self.lock_sessions().remove_user(&owner_id);
                ok(Empty {})
            }
            _ => unreachable!("public method list and match are in sync"),
        }
    }

    fn call_private(&self, method: &str, p: Value, token: &str, s: Session) -> Result<Value> {
        let timeout = self.session_timeout();
        match method {
            m::AUTH_LOGOUT => {
                self.lock_sessions().remove(token);
                self.with_db(|o| {
                    audit::record(
                        &o.conn,
                        &actor(&s),
                        "auth.logout",
                        Some("app_user"),
                        Some(&s.user.id),
                        None,
                        None,
                    )
                })?;
                ok(Empty {})
            }
            m::AUTH_CHANGE_PASSWORD => {
                let ChangePasswordParams { current_password, new_password } = params(p)?;
                self.with_db(|o| {
                    if !auth::check_password(&o.conn, &s.user.id, &current_password)? {
                        return Err(CoreError::api(
                            ErrorCode::InvalidCredentials,
                            "current password is wrong",
                        )
                        .on_field("current_password", ValidationRule::WrongPassword));
                    }
                    auth::set_password(&o.conn, &actor(&s), &s.user.id, &new_password)
                        .map_err(|e| e.rename_field("new_password"))?;
                    audit::record(
                        &o.conn,
                        &actor(&s),
                        "auth.change_password",
                        Some("app_user"),
                        Some(&s.user.id),
                        None,
                        None,
                    )
                })?;
                ok(Empty {})
            }
            m::SESSION_STATE | m::SESSION_TOUCH => ok(state_of(&s, timeout)),
            m::SESSION_LOCK => {
                let mut store = self.lock_sessions();
                let live = store.get(Some(token), timeout)?;
                live.locked = true;
                let state = state_of(live, timeout);
                drop(store);
                self.with_db(|o| {
                    audit::record(
                        &o.conn,
                        &actor(&s),
                        "session.lock",
                        Some("app_user"),
                        Some(&s.user.id),
                        None,
                        None,
                    )
                })?;
                ok(state)
            }
            m::SESSION_UNLOCK => {
                let UnlockParams { password } = params(p)?;
                let good = self.with_db(|o| {
                    let good = auth::check_password(&o.conn, &s.user.id, &password)?;
                    let action = if good { "session.unlock" } else { "session.unlock_failed" };
                    audit::record(
                        &o.conn,
                        &actor(&s),
                        action,
                        Some("app_user"),
                        Some(&s.user.id),
                        None,
                        None,
                    )?;
                    Ok(good)
                })?;
                if !good {
                    return Err(CoreError::api(ErrorCode::InvalidCredentials, "wrong password")
                        .on_field("password", ValidationRule::WrongPassword));
                }
                let mut store = self.lock_sessions();
                let live = store.get(Some(token), timeout)?;
                live.locked = false;
                live.last_activity = Instant::now();
                ok(state_of(live, timeout))
            }
            m::USERS_LIST => {
                s.require(perm::USERS_MANAGE)?;
                ok(self.with_db(|o| auth::list_users(&o.conn))?)
            }
            m::USERS_CREATE => {
                s.require(perm::USERS_MANAGE)?;
                let p: CreateUserParams = params(p)?;
                ok(self.with_db(|o| {
                    auth::create_user(
                        &o.conn,
                        &actor(&s),
                        NewUser {
                            username: &p.username,
                            display_name: &p.display_name,
                            password: &p.password,
                            role: &p.role,
                        },
                        false,
                    )
                })?)
            }
            m::USERS_UPDATE => {
                s.require(perm::USERS_MANAGE)?;
                let p: UpdateUserParams = params(p)?;
                let (user, perms) = self.with_db(|o| {
                    let u = auth::update_user(
                        &o.conn,
                        &actor(&s),
                        &p.id,
                        p.version,
                        &p.display_name,
                        &p.role,
                        p.is_active,
                    )?;
                    let perms = auth::permissions_of_role(&o.conn, &u.role)?;
                    Ok((u, perms))
                })?;
                let mut store = self.lock_sessions();
                if user.is_active {
                    store.refresh_user(&user, &perms);
                } else {
                    store.remove_user(&user.id);
                }
                ok(user)
            }
            m::ROLES_LIST => {
                s.require(perm::USERS_MANAGE)?;
                ok(self.with_db(|o| auth::list_roles(&o.conn))?)
            }
            m::REFERENCE_LIST => {
                let ReferenceListParams { type_code, language } = params(p)?;
                ok(self.with_db(|o| {
                    let mut stmt = o.conn.prepare_cached(
                        "SELECT i.id, i.code, t.label FROM reference_item i
                         JOIN reference_type ty ON ty.id = i.type_id
                         JOIN reference_translation t ON t.reference_item_id = i.id AND t.language_code = ?2
                         WHERE ty.code = ?1 AND i.is_active = 1 AND i.deleted_at IS NULL
                         ORDER BY i.sort_order, i.code",
                    )?;
                    let v: Vec<LabeledItem> = stmt
                        .query_map(params![type_code, language.code()], |r| {
                            Ok(LabeledItem { id: r.get(0)?, code: r.get(1)?, label: r.get(2)? })
                        })?
                        .collect::<rusqlite::Result<_>>()?;
                    Ok(v)
                })?)
            }
            m::GEO_PROVINCES => {
                let LanguageParams { language } = params(p)?;
                ok(self.with_db(|o| {
                    let mut stmt = o.conn.prepare_cached(
                        "SELECT p.id, p.code, t.label FROM province p
                         JOIN geo_translation t ON t.entity_type = 'province' AND t.entity_id = p.id AND t.language_code = ?1
                         ORDER BY p.sort_order",
                    )?;
                    let v: Vec<LabeledItem> = stmt
                        .query_map([language.code()], |r| Ok(LabeledItem { id: r.get(0)?, code: r.get(1)?, label: r.get(2)? }))?
                        .collect::<rusqlite::Result<_>>()?;
                    Ok(v)
                })?)
            }
            m::GEO_DISTRICTS => {
                let DistrictListParams { province_id, language } = params(p)?;
                ok(self.with_db(|o| {
                    let mut stmt = o.conn.prepare_cached(
                        "SELECT d.id, d.code, t.label FROM district d
                         JOIN geo_translation t ON t.entity_type = 'district' AND t.entity_id = d.id AND t.language_code = ?2
                         WHERE d.province_id = ?1 ORDER BY d.sort_order",
                    )?;
                    let v: Vec<LabeledItem> = stmt
                        .query_map(params![province_id, language.code()], |r| Ok(LabeledItem { id: r.get(0)?, code: r.get(1)?, label: r.get(2)? }))?
                        .collect::<rusqlite::Result<_>>()?;
                    Ok(v)
                })?)
            }
            m::CLINIC_GET => ok(self.with_db(|o| clinic::get(&o.conn))?),
            m::CLINIC_UPDATE => {
                s.require(perm::SETTINGS_MANAGE)?;
                let new: ClinicProfile = params(p)?;
                ok(self.with_db(|o| clinic::update(&o.conn, &actor(&s), &new))?)
            }
            m::CLINIC_SET_LOGO => {
                s.require(perm::SETTINGS_MANAGE)?;
                let SetLogoParams { logo_base64, logo_file_name } = params(p)?;
                let logo = decode_logo(logo_base64.as_deref(), logo_file_name.as_deref())?;
                let dir = self.config.data_dir.join("branding");
                let data_url = self.with_db(|o| clinic::set_logo(&o.conn, &actor(&s), &dir, logo))?;
                ok(ClinicLogo { data_url })
            }
            m::SETTINGS_GET => ok(self.with_db(|o| settings::get(&o.conn))?),
            m::SETTINGS_UPDATE => {
                s.require(perm::SETTINGS_MANAGE)?;
                let new: Settings = params(p)?;
                let saved = self.with_db(|o| settings::update(&o.conn, &actor(&s), &new))?;
                self.timeout_minutes.store(saved.session_timeout_minutes, Ordering::Relaxed);
                ok(saved)
            }
            m::BACKUP_CREATE => {
                s.require(perm::BACKUP_CREATE)?;
                ok(self.with_db(|o| {
                    let info =
                        backup::create(&o.conn, &o.key, &self.config, backup::Kind::Manual, &actor(&s))?;
                    backup::apply_retention(
                        &o.conn,
                        &self.config.backup_dir(),
                        settings::get(&o.conn)?.backup_keep_daily,
                    )?;
                    Ok(info)
                })?)
            }
            m::BACKUP_LIST => {
                s.require(perm::BACKUP_VIEW)?;
                ok(self.with_db(|o| backup::list(&o.conn))?)
            }
            m::AUDIT_LIST => {
                s.require(perm::AUDIT_VIEW)?;
                let AuditListParams { limit, offset } = params(p)?;
                ok(self.with_db(|o| audit::list(&o.conn, limit, offset))?)
            }
            m::SYSTEM_INFO => ok(self.system_info()?),
            _ => Err(CoreError::api(ErrorCode::UnknownMethod, method.to_string())),
        }
    }

    fn default_language(&self) -> Language {
        let code: Option<String> = self.lock_db().as_ref().and_then(|o| {
            o.conn
                .query_row("SELECT value FROM db_meta WHERE key = 'default_language'", [], |r| r.get(0))
                .optional()
                .ok()
                .flatten()
        });
        match code.as_deref() {
            Some("ps") => Language::Ps,
            Some("en") => Language::En,
            _ => Language::Fa,
        }
    }

    /// First run: create the encrypted database, the clinic identity and the Owner.
    fn setup(&self, p: SetupParams) -> Result<Value> {
        if p.clinic_name.trim().is_empty() || p.clinic_name.chars().count() > 120 {
            return Err(CoreError::invalid(
                "clinic_name",
                ValidationRule::ClinicNameLength,
                "clinic name must be 1–120 characters",
            ));
        }
        auth::validate_username(&p.owner_username).map_err(|e| e.rename_field("owner_username"))?;
        auth::validate_display_name(&p.owner_display_name)
            .map_err(|e| e.rename_field("owner_display_name"))?;
        auth::validate_password(&p.owner_password).map_err(|e| e.rename_field("owner_password"))?;
        let logo_bytes = decode_logo(p.logo_base64.as_deref(), p.logo_file_name.as_deref())?;
        let mut guard = self.lock_db();
        if guard.is_some() || self.config.db_path().exists() {
            return Err(CoreError::api(ErrorCode::AlreadySetUp, "clinic database already exists"));
        }
        drop(guard);

        let recovery = RecoveryKey::generate();
        let path = self.config.db_path();
        let result = (|| -> Result<()> {
            let (conn, key) = db::create(&path, self.protector.as_ref(), &recovery)?;
            self.install(conn, key)?;
            let logo_path = match &logo_bytes {
                Some((ext, bytes)) => {
                    let dir = self.config.data_dir.join("branding");
                    fs::create_dir_all(&dir)?;
                    let file = dir.join(format!("logo.{ext}"));
                    fs::write(&file, bytes)?;
                    Some(file.display().to_string())
                }
                None => None,
            };
            self.with_db(|o| {
                let tx = o.conn.transaction()?;
                let now = now_iso();
                for (k, v) in [
                    ("clinic_id", new_id()),
                    ("clinic_name", p.clinic_name.trim().to_string()),
                    ("default_language", p.language.code().to_string()),
                    ("created_at", now.clone()),
                    ("created_with_version", APP_VERSION.to_string()),
                ] {
                    tx.execute("INSERT INTO db_meta(key, value) VALUES (?1, ?2)", params![k, v])?;
                }
                clinic::init_from_setup(&tx, &p, logo_path.as_deref())?;
                let owner = auth::create_user(
                    &tx,
                    &Actor::system(),
                    NewUser { username: &p.owner_username, display_name: &p.owner_display_name, password: &p.owner_password, role: OWNER },
                    true,
                )?;
                audit::record(
                    &tx,
                    &Actor::user(&owner.id, &owner.username),
                    "app.setup",
                    Some("db_meta"),
                    None,
                    None,
                    Some(&json!({ "clinic_name": p.clinic_name.trim(), "language": p.language.code(), "version": APP_VERSION })),
                )?;
                tx.commit()?;
                Ok(())
            })
        })();
        if let Err(e) = result {
            // Leave no half-created clinic behind: the next attempt starts clean.
            guard = self.lock_db();
            *guard = None;
            drop(guard);
            for ext in ["db", "db-wal", "db-shm", "key", "rkey"] {
                let _ = fs::remove_file(path.with_extension(ext));
            }
            return Err(e);
        }
        self.start_integrity_check();
        tracing::info!("clinic set up");
        ok(SetupResult { recovery_key: recovery.to_display() })
    }

    pub fn system_info(&self) -> Result<SystemInfo> {
        let (database, last_backup) = match self.lock_db().as_ref() {
            Some(o) => (
                Some(DatabaseInfo {
                    encrypted: !fs::read(self.config.db_path())
                        .map(|b| b.starts_with(b"SQLite format 3"))
                        .unwrap_or(true),
                    cipher_version: db::cipher_version(&o.conn)?,
                    sqlite_version: db::sqlite_version(&o.conn)?,
                    key_protection: self.protector.kind().code().into(),
                    schema_version: db::schema_version(&o.conn)?,
                    // Recent writes live in the WAL until checkpointed; count both.
                    size_bytes: ["db", "db-wal"]
                        .iter()
                        .filter_map(|ext| fs::metadata(self.config.db_path().with_extension(ext)).ok())
                        .map(|m| m.len() as i64)
                        .sum(),
                }),
                backup::latest(&o.conn)?,
            ),
            None => (None, None),
        };
        let build_arch = sysinfo::build_arch();
        let machine_arch = sysinfo::machine_arch();
        Ok(SystemInfo {
            app_version: APP_VERSION.into(),
            git_commit: GIT_COMMIT.into(),
            build_arch: build_arch.into(),
            emulated: machine_arch != build_arch,
            machine_arch,
            os: sysinfo::os_description(),
            environment: self.config.environment.code().into(),
            computer_name: audit::computer_name().into(),
            data_dir: self.config.data_dir.display().to_string(),
            log_dir: self.config.log_dir().display().to_string(),
            database,
            integrity: self.integrity(),
            last_backup,
        })
    }
}
