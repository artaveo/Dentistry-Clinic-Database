//! Method router: the one place where the API contract meets the Core.
//! Every method is authorised here (session, lock state, permission) before
//! any business code runs, so no transport can bypass Permission or Audit.

use std::fs;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use artaveo_shared::{methods as m, *};
use rusqlite::{params, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::attachment;
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
use crate::medical;
use crate::patient;
use crate::session::{state_of, Session};
use crate::settings;
use crate::sysinfo;
use crate::{appointment, recall, scheduling};
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

/// Decodes an attachment upload's base64 body, checking size up front
/// (`attachment::store` enforces it again on the actual plaintext).
fn decode_attachment(base64: &str) -> Result<Vec<u8>> {
    let bytes = data_encoding::BASE64.decode(base64.as_bytes()).map_err(|e| {
        CoreError::invalid("data_base64", ValidationRule::AttachmentType, format!("invalid file data: {e}"))
    })?;
    if bytes.is_empty() || bytes.len() > attachment::MAX_BYTES {
        return Err(CoreError::invalid(
            "data_base64",
            ValidationRule::AttachmentSize,
            "file must be 1 byte - 20 MiB",
        ));
    }
    Ok(bytes)
}

fn guess_mime(file_name: &str) -> &'static str {
    match file_name.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
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

    /// Resolves the session and applies the idle lock. Requests are *not*
    /// activity: background refreshes would otherwise keep an unattended
    /// screen unlocked (OF-012). Only `session.touch` (real mouse/keyboard
    /// input reported by the UI), login and unlock reset the idle timer.
    fn authorize(&self, token: &str, method: &str) -> Result<Session> {
        let timeout = self.session_timeout();
        let mut store = self.lock_sessions();
        let s = store.get(Some(token).filter(|t| !t.is_empty()), timeout)?;
        if s.locked && !ALLOWED_WHEN_LOCKED.contains(&method) {
            return Err(CoreError::api(ErrorCode::SessionLocked, "screen is locked"));
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
            m::SESSION_STATE => ok(state_of(&s, timeout)),
            m::SESSION_TOUCH => {
                let TouchParams { idle_ms } = params(p)?;
                let mut store = self.lock_sessions();
                let live = store.get(Some(token), timeout)?;
                // The user was last active `idle_ms` ago (never later than now,
                // never earlier than what the Core already knows).
                let idle = Duration::from_millis(idle_ms.into()).min(timeout);
                let at = Instant::now().checked_sub(idle).unwrap_or_else(Instant::now);
                if !live.locked && at > live.last_activity {
                    live.last_activity = at;
                }
                ok(state_of(live, timeout))
            }
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
            m::USERS_RESET_PASSWORD => {
                s.require(perm::USERS_MANAGE)?;
                let ResetPasswordParams { id, new_password } = params(p)?;
                let user = self.with_tx(|c| auth::reset_password(c, &actor(&s), &id, &new_password))?;
                // Whoever knew the old password must sign in again.
                self.lock_sessions().remove_user(&user.id);
                ok(user)
            }
            m::USERS_UNLOCK => {
                s.require(perm::USERS_MANAGE)?;
                let UserIdParams { id } = params(p)?;
                ok(self.with_tx(|c| auth::unlock_user(c, &actor(&s), &id))?)
            }
            m::ROLES_LIST => {
                s.require(perm::USERS_MANAGE)?;
                ok(self.with_db(|o| auth::list_roles(&o.conn))?)
            }
            m::PERMISSIONS_LIST => {
                s.require(perm::USERS_MANAGE)?;
                ok(auth::list_permissions())
            }
            m::ROLES_CREATE => {
                s.require(perm::USERS_MANAGE)?;
                let p: CreateRoleParams = params(p)?;
                ok(self.with_tx(|c| auth::create_role(c, &actor(&s), &p))?)
            }
            m::ROLES_UPDATE => {
                s.require(perm::USERS_MANAGE)?;
                let p: UpdateRoleParams = params(p)?;
                let (role, holders) = self.with_tx(|c| {
                    let role = auth::update_role(c, &actor(&s), &p)?;
                    Ok((role, auth::users_with_role(c, &p.code)?))
                })?;
                // Signed-in holders get the new permission set at once.
                let perms: std::collections::HashSet<String> = role.permissions.iter().cloned().collect();
                let mut store = self.lock_sessions();
                for u in &holders {
                    store.refresh_user(u, &perms);
                }
                ok(role)
            }
            m::ROLES_RESET => {
                s.require(perm::USERS_MANAGE)?;
                let DeleteRoleParams { code, version } = params(p)?;
                ok(self.with_tx(|c| auth::reset_role(c, &actor(&s), &code, version))?)
            }
            m::ROLES_DELETE => {
                s.require(perm::USERS_MANAGE)?;
                let DeleteRoleParams { code, version } = params(p)?;
                self.with_tx(|c| auth::delete_role(c, &actor(&s), &code, version))?;
                ok(Empty {})
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
                let AuditListParams { limit, offset, entity_id } = params(p)?;
                // Scoped to one record (e.g. a patient's Audit History tab, 3.6): anyone who
                // can view that record can see its history. The full log needs AUDIT_VIEW.
                s.require(if entity_id.is_some() { perm::PATIENTS_VIEW } else { perm::AUDIT_VIEW })?;
                ok(self.with_db(|o| audit::list(&o.conn, limit, offset, entity_id.as_deref()))?)
            }
            m::PATIENTS_LIST => {
                s.require(perm::PATIENTS_VIEW)?;
                let p: PatientListParams = params(p)?;
                ok(self.with_db(|o| patient::list_patients(&o.conn, &p))?)
            }
            m::PATIENTS_GET => {
                s.require(perm::PATIENTS_VIEW)?;
                let PatientIdParams { patient_id } = params(p)?;
                ok(self.with_db(|o| patient::get_patient(&o.conn, &patient_id))?)
            }
            m::PATIENTS_CREATE => {
                s.require(perm::PATIENTS_EDIT)?;
                let p: CreatePatientParams = params(p)?;
                ok(self.with_db(|o| patient::create_patient(&o.conn, &actor(&s), &p))?)
            }
            m::PATIENTS_UPDATE => {
                s.require(perm::PATIENTS_EDIT)?;
                let p: UpdatePatientParams = params(p)?;
                ok(self.with_db(|o| patient::update_patient(&o.conn, &actor(&s), &p))?)
            }
            m::PATIENTS_DELETE => {
                s.require(perm::PATIENTS_EDIT)?;
                let IdVersionParams { id, version } = params(p)?;
                self.with_db(|o| patient::delete_patient(&o.conn, &actor(&s), &id, version))?;
                ok(Empty {})
            }
            m::PATIENTS_CHECK_DUPLICATE => {
                s.require(perm::PATIENTS_EDIT)?;
                let DuplicateCheckParams { full_name, phone } = params(p)?;
                ok(self.with_db(|o| patient::check_duplicate(&o.conn, &full_name, phone.as_deref()))?)
            }
            m::PATIENTS_MERGE => {
                s.require(perm::PATIENTS_EDIT)?;
                let p: MergePatientsParams = params(p)?;
                ok(self.with_db(|o| patient::merge_patients(&o.conn, &actor(&s), &p))?)
            }
            m::PATIENTS_IMPORT => {
                s.require(perm::PATIENTS_EDIT)?;
                let p: ImportPatientsParams = params(p)?;
                ok(self.with_db(|o| crate::import::import_patients(&o.conn, &actor(&s), &p))?)
            }
            m::PATIENTS_EXPORT => {
                s.require(perm::PATIENTS_VIEW)?;
                ok(self.with_db(|o| {
                    crate::import::export_patients(&o.conn, &actor(&s), &self.config.exports_dir())
                })?)
            }
            m::MEDICAL_HISTORY_GET => {
                s.require(perm::CLINICAL_VIEW)?;
                let PatientIdParams { patient_id } = params(p)?;
                ok(self.with_db(|o| medical::get_history(&o.conn, &patient_id))?)
            }
            m::MEDICAL_HISTORY_UPDATE => {
                s.require(perm::CLINICAL_EDIT)?;
                let p: UpdateMedicalHistoryParams = params(p)?;
                ok(self.with_tx(|c| medical::update_history(c, &actor(&s), &p))?)
            }
            m::MEDICAL_HISTORY_REVIEW => {
                s.require(perm::CLINICAL_EDIT)?;
                let p: ReviewMedicalHistoryParams = params(p)?;
                ok(self.with_tx(|c| medical::review_history(c, &actor(&s), &p))?)
            }
            m::MEDICAL_QUESTIONS_LIST => {
                // Whoever sees a patient's alert banner needs the questions' labels.
                s.require(perm::PATIENTS_VIEW)?;
                let MedicalQuestionListParams { include_inactive } = params(p)?;
                ok(self.with_db(|o| medical::list_questions(&o.conn, include_inactive))?)
            }
            m::MEDICAL_QUESTIONS_CREATE => {
                s.require(perm::SETTINGS_MANAGE)?;
                let p: CreateMedicalQuestionParams = params(p)?;
                ok(self.with_tx(|c| medical::create_question(c, &actor(&s), &p))?)
            }
            m::MEDICAL_QUESTIONS_UPDATE => {
                s.require(perm::SETTINGS_MANAGE)?;
                let p: UpdateMedicalQuestionParams = params(p)?;
                ok(self.with_tx(|c| medical::update_question(c, &actor(&s), &p))?)
            }
            m::ATTACHMENTS_LIST => {
                s.require(perm::PATIENTS_VIEW)?;
                let PatientIdParams { patient_id } = params(p)?;
                ok(self.with_db(|o| patient::list_attachments(&o.conn, &patient_id))?)
            }
            m::ATTACHMENTS_UPLOAD => {
                s.require(perm::PATIENTS_EDIT)?;
                let p: UploadAttachmentParams = params(p)?;
                let bytes = decode_attachment(&p.data_base64)?;
                let mime = guess_mime(&p.file_name).to_string();
                let dir = self.config.attachments_dir();
                let captured_at = p.captured_at.clone().unwrap_or_else(|| now_iso()[..10].to_string());
                ok(self.with_tx_keyed(|c, key| {
                    let (sha256, size) = attachment::store(&dir, key, &bytes)?;
                    // OF-018: the thumbnail is made here, once, so lists never decode full images.
                    let thumb = match attachment::make_thumbnail(&bytes) {
                        Some(t) => Some(attachment::store(&dir, key, &t)?.0),
                        None => None,
                    };
                    patient::create_attachment(
                        c,
                        &actor(&s),
                        &p.patient_id,
                        p.kind,
                        &p.file_name,
                        &mime,
                        &sha256,
                        size,
                        p.tooth.as_deref(),
                        p.description.as_deref(),
                        thumb.as_deref(),
                        &captured_at,
                    )
                })?)
            }
            m::ATTACHMENTS_DELETE => {
                s.require(perm::PATIENTS_EDIT)?;
                let IdVersionParams { id, version } = params(p)?;
                ok(self
                    .with_db(|o| patient::delete_attachment(&o.conn, &actor(&s), &id, version))
                    .map(|_| Empty {})?)
            }
            m::ATTACHMENTS_FILE => {
                s.require(perm::PATIENTS_VIEW)?;
                let AttachmentFileParams { id, thumbnail } = params(p)?;
                let dir = self.config.attachments_dir();
                let (bytes, mime) = self.with_db(|o| {
                    let a = patient::get_attachment(&o.conn, &id)?;
                    let (sha256, thumb) = patient::attachment_hashes(&o.conn, &id)?;
                    match thumb.filter(|_| thumbnail) {
                        Some(t) => Ok((attachment::read(&dir, &o.key, &t)?, "image/jpeg".to_string())),
                        None => Ok((attachment::read(&dir, &o.key, &sha256)?, a.mime_type)),
                    }
                })?;
                ok(AttachmentData {
                    data_url: format!("data:{mime};base64,{}", data_encoding::BASE64.encode(&bytes)),
                })
            }
            m::ATTACHMENTS_THUMBNAILS => {
                s.require(perm::PATIENTS_VIEW)?;
                let PatientIdParams { patient_id } = params(p)?;
                let dir = self.config.attachments_dir();
                ok(self.with_tx_keyed(|c, key| {
                    let mut out = Vec::new();
                    for a in patient::list_attachments(c, &patient_id)? {
                        if !a.mime_type.starts_with("image/") {
                            out.push(AttachmentThumbnail { id: a.id, data_url: None });
                            continue;
                        }
                        let (sha256, thumb) = patient::attachment_hashes(c, &a.id)?;
                        let thumb = match thumb {
                            Some(t) => Some(t),
                            // A file saved before thumbnails existed: make its thumbnail once, now.
                            None => {
                                match attachment::make_thumbnail(&attachment::read(&dir, key, &sha256)?) {
                                    Some(bytes) => {
                                        let t = attachment::store(&dir, key, &bytes)?.0;
                                        patient::set_attachment_thumbnail(c, &a.id, &t)?;
                                        Some(t)
                                    }
                                    None => None,
                                }
                            }
                        };
                        let data_url = match thumb {
                            Some(t) => Some(format!(
                                "data:image/jpeg;base64,{}",
                                data_encoding::BASE64.encode(&attachment::read(&dir, key, &t)?)
                            )),
                            None => None,
                        };
                        out.push(AttachmentThumbnail { id: a.id, data_url });
                    }
                    Ok(out)
                })?)
            }
            m::PATIENTS_IMPORT_INSPECT => {
                s.require(perm::PATIENTS_EDIT)?;
                let p: ImportInspectParams = params(p)?;
                ok(crate::import::inspect_import(&p)?)
            }
            m::DOCTORS_LIST => {
                s.require(perm::APPOINTMENTS_VIEW)?;
                let DoctorListParams { include_inactive } = params(p)?;
                ok(self.with_db(|o| scheduling::list_doctors(&o.conn, include_inactive))?)
            }
            m::DOCTORS_CREATE => {
                s.require(perm::DOCTORS_MANAGE)?;
                let p: CreateDoctorParams = params(p)?;
                ok(self.with_tx(|c| scheduling::create_doctor(c, &actor(&s), &p))?)
            }
            m::DOCTORS_UPDATE => {
                s.require(perm::DOCTORS_MANAGE)?;
                let p: UpdateDoctorParams = params(p)?;
                ok(self.with_tx(|c| scheduling::update_doctor(c, &actor(&s), &p))?)
            }
            m::DOCTORS_SET_SCHEDULE => {
                s.require(perm::DOCTORS_MANAGE)?;
                let p: SetScheduleParams = params(p)?;
                ok(self.with_tx(|c| scheduling::set_schedule(c, &actor(&s), &p))?)
            }
            m::DOCTORS_ADD_LEAVE => {
                s.require(perm::DOCTORS_MANAGE)?;
                let p: AddLeaveParams = params(p)?;
                ok(self.with_tx(|c| scheduling::add_leave(c, &actor(&s), &p))?)
            }
            m::DOCTORS_DELETE_LEAVE => {
                s.require(perm::DOCTORS_MANAGE)?;
                let IdVersionParams { id, version } = params(p)?;
                self.with_tx(|c| scheduling::delete_leave(c, &actor(&s), &id, version))?;
                ok(Empty {})
            }
            m::CHAIRS_LIST => {
                s.require(perm::APPOINTMENTS_VIEW)?;
                let ChairListParams { include_inactive } = params(p)?;
                ok(self.with_db(|o| scheduling::list_chairs(&o.conn, include_inactive))?)
            }
            m::CHAIRS_CREATE => {
                s.require(perm::DOCTORS_MANAGE)?;
                let p: CreateChairParams = params(p)?;
                ok(self.with_tx(|c| scheduling::create_chair(c, &actor(&s), &p))?)
            }
            m::CHAIRS_UPDATE => {
                s.require(perm::DOCTORS_MANAGE)?;
                let p: UpdateChairParams = params(p)?;
                ok(self.with_tx(|c| scheduling::update_chair(c, &actor(&s), &p))?)
            }
            m::APPOINTMENTS_LIST => {
                s.require(perm::APPOINTMENTS_VIEW)?;
                let p: AppointmentListParams = params(p)?;
                ok(self.with_db(|o| appointment::list_appointments(&o.conn, &p))?)
            }
            m::APPOINTMENTS_COUNTS => {
                s.require(perm::APPOINTMENTS_VIEW)?;
                let p: AppointmentCountsParams = params(p)?;
                ok(self.with_db(|o| appointment::appointment_counts(&o.conn, &p))?)
            }
            m::APPOINTMENTS_GET => {
                s.require(perm::APPOINTMENTS_VIEW)?;
                let IdParams { id } = params(p)?;
                ok(self.with_db(|o| appointment::get_appointment(&o.conn, &id))?)
            }
            m::APPOINTMENTS_CREATE => {
                s.require(perm::APPOINTMENTS_EDIT)?;
                let p: CreateAppointmentParams = params(p)?;
                ok(self.with_tx(|c| appointment::create_appointment(c, &actor(&s), &p))?)
            }
            m::APPOINTMENTS_UPDATE => {
                s.require(perm::APPOINTMENTS_EDIT)?;
                let p: UpdateAppointmentParams = params(p)?;
                ok(self.with_tx(|c| appointment::update_appointment(c, &actor(&s), &p))?)
            }
            m::APPOINTMENTS_RESCHEDULE => {
                s.require(perm::APPOINTMENTS_EDIT)?;
                let p: RescheduleAppointmentParams = params(p)?;
                ok(self.with_tx(|c| appointment::reschedule_appointment(c, &actor(&s), &p))?)
            }
            m::APPOINTMENTS_SET_STATUS => {
                let p: SetAppointmentStatusParams = params(p)?;
                // Starting and finishing treatment is clinical work (doctor, assistant);
                // arrival, confirmation, cancellation and no-show are reception work.
                let treatment =
                    matches!(p.status, AppointmentStatus::InTreatment | AppointmentStatus::Completed);
                s.require(if treatment { perm::APPOINTMENTS_TREAT } else { perm::APPOINTMENTS_EDIT })?;
                ok(self.with_tx(|c| appointment::set_status(c, &actor(&s), &p))?)
            }
            m::APPOINTMENTS_WALK_IN => {
                s.require(perm::APPOINTMENTS_EDIT)?;
                let p: WalkInParams = params(p)?;
                ok(self.with_tx(|c| appointment::walk_in(c, &actor(&s), &p))?)
            }
            m::RECALLS_LIST => {
                s.require(perm::APPOINTMENTS_VIEW)?;
                let p: RecallListParams = params(p)?;
                ok(self.with_db(|o| recall::list_recalls(&o.conn, &p))?)
            }
            m::RECALLS_CREATE => {
                s.require(perm::APPOINTMENTS_EDIT)?;
                let p: CreateRecallParams = params(p)?;
                ok(self.with_tx(|c| recall::create_recall(c, &actor(&s), &p))?)
            }
            m::RECALLS_UPDATE => {
                s.require(perm::APPOINTMENTS_EDIT)?;
                let p: UpdateRecallParams = params(p)?;
                ok(self.with_tx(|c| recall::update_recall(c, &actor(&s), &p))?)
            }
            m::RECALLS_SET_STATUS => {
                s.require(perm::APPOINTMENTS_EDIT)?;
                let p: SetRecallStatusParams = params(p)?;
                ok(self.with_tx(|c| recall::set_recall_status(c, &actor(&s), &p))?)
            }
            m::SYSTEM_INFO => ok(self.system_info()?),
            m::DRAFTS_GET => {
                let p: DraftParams = params(p)?;
                ok(self.with_db(|o| crate::draft::get(&o.conn, &s.user.id, &p))?)
            }
            m::DRAFTS_SAVE => {
                let p: SaveDraftParams = params(p)?;
                self.with_db(|o| crate::draft::save(&o.conn, &s.user.id, &p))?;
                ok(Empty {})
            }
            m::DRAFTS_DELETE => {
                let p: DraftParams = params(p)?;
                self.with_db(|o| crate::draft::delete(&o.conn, &s.user.id, &p))?;
                ok(Empty {})
            }
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
                    // OF-024: the size of the data itself. The write-ahead log (WAL) is temporary and is
                    // not counted, so the figure does not jump up and down with recent writes.
                    size_bytes: db::data_size_bytes(&self.config.db_path())?,
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
