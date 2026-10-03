//! Local authentication (Argon2id), users, roles and granular permissions
//! (roadmap 1.6). Permissions are defined here in code; roles are named
//! permission sets, synchronised into the database on every start.

use std::collections::HashSet;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use artaveo_shared::{
    CreateRoleParams, ErrorCode, PermissionInfo, RoleInfo, UpdateRoleParams, UserInfo, ValidationRule,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use time::Duration;

use crate::audit::{self, Actor};
use crate::clock::{iso, now, now_iso, parse};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::ids::{new_id, seed_id};

/// Granular permissions. Later phases add theirs here; roles pick them up.
pub mod perm {
    pub const PATIENTS_VIEW: &str = "patients.view";
    pub const PATIENTS_EDIT: &str = "patients.edit";
    pub const APPOINTMENTS_VIEW: &str = "appointments.view";
    pub const APPOINTMENTS_EDIT: &str = "appointments.edit";
    /// Start and finish a treatment in the queue (doctor/assistant work, 4.5).
    pub const APPOINTMENTS_TREAT: &str = "appointments.treat";
    /// Doctors, chairs, schedules and leave (4.2).
    pub const DOCTORS_MANAGE: &str = "doctors.manage";
    pub const CLINICAL_VIEW: &str = "clinical.view";
    pub const CLINICAL_EDIT: &str = "clinical.edit";
    pub const BILLING_VIEW: &str = "billing.view";
    pub const BILLING_EDIT: &str = "billing.edit";
    pub const BILLING_VOID: &str = "billing.void";
    pub const REPORTS_VIEW: &str = "reports.view";
    pub const INVENTORY_MANAGE: &str = "inventory.manage";
    pub const USERS_MANAGE: &str = "users.manage";
    pub const SETTINGS_MANAGE: &str = "settings.manage";
    pub const BACKUP_VIEW: &str = "backup.view";
    pub const BACKUP_CREATE: &str = "backup.create";
    pub const BACKUP_RESTORE: &str = "backup.restore";
    pub const AUDIT_VIEW: &str = "audit.view";
}

use perm::*;

pub const ALL_PERMISSIONS: &[&str] = &[
    PATIENTS_VIEW,
    PATIENTS_EDIT,
    APPOINTMENTS_VIEW,
    APPOINTMENTS_EDIT,
    APPOINTMENTS_TREAT,
    DOCTORS_MANAGE,
    CLINICAL_VIEW,
    CLINICAL_EDIT,
    BILLING_VIEW,
    BILLING_EDIT,
    BILLING_VOID,
    REPORTS_VIEW,
    INVENTORY_MANAGE,
    USERS_MANAGE,
    SETTINGS_MANAGE,
    BACKUP_VIEW,
    BACKUP_CREATE,
    BACKUP_RESTORE,
    AUDIT_VIEW,
];

/// Permissions only the built-in Owner role carries; a custom role cannot include them.
pub const OWNER_ONLY: &[&str] = &[BACKUP_RESTORE];

pub const OWNER: &str = "owner";

/// Default roles (roadmap 1.6). Owner has everything; restore is Owner-only (10.7).
pub fn system_roles() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (OWNER, ALL_PERMISSIONS.to_vec()),
        ("administrator", ALL_PERMISSIONS.iter().copied().filter(|p| *p != BACKUP_RESTORE).collect()),
        (
            "receptionist",
            vec![
                PATIENTS_VIEW,
                PATIENTS_EDIT,
                APPOINTMENTS_VIEW,
                APPOINTMENTS_EDIT,
                BILLING_VIEW,
                BILLING_EDIT,
            ],
        ),
        (
            "doctor",
            vec![
                PATIENTS_VIEW,
                PATIENTS_EDIT,
                APPOINTMENTS_VIEW,
                APPOINTMENTS_TREAT,
                CLINICAL_VIEW,
                CLINICAL_EDIT,
                BILLING_VIEW,
            ],
        ),
        ("accountant", vec![PATIENTS_VIEW, BILLING_VIEW, BILLING_EDIT, BILLING_VOID, REPORTS_VIEW]),
        ("assistant", vec![PATIENTS_VIEW, APPOINTMENTS_VIEW, APPOINTMENTS_TREAT, CLINICAL_VIEW]),
    ]
}

pub fn role_id(code: &str) -> String {
    seed_id("role", code)
}

/// The row id of any role by code: system roles have a deterministic id, a clinic's own roles a random one.
fn resolve_role_id(conn: &Connection, code: &str) -> Result<String> {
    conn.query_row("SELECT id FROM role WHERE code = ?1 AND deleted_at IS NULL", [code], |r| r.get(0))
        .optional()?
        .ok_or_else(|| {
            CoreError::api(ErrorCode::NotFound, format!("role {code}"))
                .on_field("role", ValidationRule::RoleNotFound)
        })
}

/// Makes permissions and system roles in the DB match the code.
pub fn sync_roles(conn: &mut Connection) -> Result<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    for p in ALL_PERMISSIONS {
        tx.execute("INSERT INTO permission(code) VALUES (?1) ON CONFLICT DO NOTHING", [p])?;
    }
    for (code, perms) in system_roles() {
        let id = role_id(code);
        tx.execute(
            "INSERT INTO role(id, code, is_system, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?3)
             ON CONFLICT(id) DO NOTHING",
            params![id, code, now],
        )?;
        tx.execute("DELETE FROM role_permission WHERE role_id = ?1", [&id])?;
        for p in perms {
            tx.execute(
                "INSERT INTO role_permission(role_id, permission_code) VALUES (?1, ?2)",
                params![id, p],
            )?;
        }
    }
    // Permissions removed from code disappear from the database too.
    let known: Vec<String> = ALL_PERMISSIONS.iter().map(|s| format!("'{s}'")).collect();
    tx.execute_batch(&format!(
        "DELETE FROM role_permission WHERE permission_code NOT IN ({0});
         DELETE FROM permission WHERE code NOT IN ({0});",
        known.join(",")
    ))?;
    tx.commit()?;
    Ok(())
}

const ROLE_SELECT: &str = "SELECT r.id, r.code, r.label, r.is_system, r.version,
        (SELECT COUNT(*) FROM app_user u WHERE u.role_id = r.id AND u.deleted_at IS NULL AND u.is_active = 1)
     FROM role r WHERE r.deleted_at IS NULL";

fn read_roles(conn: &Connection, tail: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<RoleInfo>> {
    let mut stmt = conn.prepare(&format!("{ROLE_SELECT} {tail}"))?;
    let rows: Vec<(String, RoleInfo)> = stmt
        .query_map(args, |r| {
            Ok((
                r.get::<_, String>(0)?,
                RoleInfo {
                    code: r.get(1)?,
                    label: r.get(2)?,
                    is_system: r.get::<_, i64>(3)? == 1,
                    permissions: Vec::new(),
                    user_count: r.get(5)?,
                    version: r.get(4)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter()
        .map(|(id, mut info)| {
            info.permissions = role_permissions_by_id(conn, &id)?;
            Ok(info)
        })
        .collect()
}

/// System roles first, then the clinic's own roles by name.
pub fn list_roles(conn: &Connection) -> Result<Vec<RoleInfo>> {
    read_roles(conn, "ORDER BY r.is_system DESC, COALESCE(r.label, r.code) COLLATE NOCASE", &[])
}

pub fn get_role(conn: &Connection, code: &str) -> Result<RoleInfo> {
    read_roles(conn, "AND r.code = ?1", &[&code])?.into_iter().next().ok_or_else(|| {
        CoreError::api(ErrorCode::NotFound, format!("role {code}"))
            .on_field("code", ValidationRule::RoleNotFound)
    })
}

pub fn list_permissions() -> Vec<PermissionInfo> {
    ALL_PERMISSIONS
        .iter()
        .map(|p| PermissionInfo { code: (*p).into(), assignable: !OWNER_ONLY.contains(p) })
        .collect()
}

fn validate_role(
    conn: &Connection,
    label: &str,
    permissions: &[String],
    except: Option<&str>,
) -> Result<Vec<String>> {
    let len = label.trim().chars().count();
    if !(1..=60).contains(&len) {
        return Err(CoreError::invalid(
            "label",
            ValidationRule::RoleLabelLength,
            "role name must be 1-60 characters",
        ));
    }
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM role WHERE label = ?1 COLLATE NOCASE AND deleted_at IS NULL AND code IS NOT ?2)",
        params![label.trim(), except],
        |r| r.get(0),
    )?;
    if taken {
        return Err(CoreError::invalid(
            "label",
            ValidationRule::RoleLabelTaken,
            "a role with this name exists",
        ));
    }
    let mut perms: Vec<String> = permissions.to_vec();
    perms.sort();
    perms.dedup();
    if perms.is_empty() {
        return Err(CoreError::invalid(
            "permissions",
            ValidationRule::PermissionsEmpty,
            "pick at least one permission",
        ));
    }
    for p in &perms {
        if !ALL_PERMISSIONS.contains(&p.as_str()) {
            return Err(CoreError::invalid(
                "permissions",
                ValidationRule::PermissionUnknown,
                format!("unknown permission {p}"),
            ));
        }
        if OWNER_ONLY.contains(&p.as_str()) {
            return Err(CoreError::invalid(
                "permissions",
                ValidationRule::PermissionNotAllowed,
                format!("{p} is reserved for the owner"),
            ));
        }
    }
    Ok(perms)
}

/// A clinic's own role (4.1): a name plus any set of permissions except the Owner-only ones.
pub fn create_role(conn: &Connection, actor: &Actor, p: &CreateRoleParams) -> Result<RoleInfo> {
    let perms = validate_role(conn, &p.label, &p.permissions, None)?;
    let id = new_id();
    let code =
        format!("custom_{}", id.rsplit('-').next().unwrap_or("role").chars().take(8).collect::<String>());
    let now = now_iso();
    conn.execute(
        "INSERT INTO role(id, code, is_system, label, created_at, created_by, updated_at, updated_by)
         VALUES (?1, ?2, 0, ?3, ?4, ?5, ?4, ?5)",
        params![id, code, p.label.trim(), now, actor.user_id],
    )?;
    for perm in &perms {
        conn.execute(
            "INSERT INTO role_permission(role_id, permission_code) VALUES (?1, ?2)",
            params![id, perm],
        )?;
    }
    let info = get_role(conn, &code)?;
    audit::record(conn, actor, "role.create", Some("role"), Some(&id), None, Some(&json!(info)))?;
    Ok(info)
}

fn custom_role(conn: &Connection, code: &str) -> Result<RoleInfo> {
    let role = get_role(conn, code)?;
    if role.is_system {
        return Err(CoreError::invalid(
            "code",
            ValidationRule::RoleSystem,
            "built-in roles cannot be changed",
        ));
    }
    Ok(role)
}

pub fn update_role(conn: &Connection, actor: &Actor, p: &UpdateRoleParams) -> Result<RoleInfo> {
    let before = custom_role(conn, &p.code)?;
    let perms = validate_role(conn, &p.label, &p.permissions, Some(&p.code))?;
    let id = role_row_id(conn, &p.code)?;
    let changed = conn.execute(
        "UPDATE role SET label = ?1, updated_at = ?2, updated_by = ?3, version = version + 1
         WHERE id = ?4 AND version = ?5 AND deleted_at IS NULL",
        params![p.label.trim(), now_iso(), actor.user_id, id, p.version],
    )?;
    expect_one_row(changed, "role")?;
    conn.execute("DELETE FROM role_permission WHERE role_id = ?1", [&id])?;
    for perm in &perms {
        conn.execute(
            "INSERT INTO role_permission(role_id, permission_code) VALUES (?1, ?2)",
            params![id, perm],
        )?;
    }
    let after = get_role(conn, &p.code)?;
    audit::record(
        conn,
        actor,
        "role.update",
        Some("role"),
        Some(&id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

pub fn delete_role(conn: &Connection, actor: &Actor, code: &str, version: i64) -> Result<()> {
    let before = custom_role(conn, code)?;
    let id = role_row_id(conn, code)?;
    let in_use: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM app_user WHERE role_id = ?1 AND deleted_at IS NULL)",
        [&id],
        |r| r.get(0),
    )?;
    if in_use {
        return Err(CoreError::invalid("code", ValidationRule::RoleInUse, "users still have this role"));
    }
    let changed = conn.execute(
        "UPDATE role SET deleted_at = ?1, updated_at = ?1, updated_by = ?2, version = version + 1
         WHERE id = ?3 AND version = ?4 AND deleted_at IS NULL",
        params![now_iso(), actor.user_id, id, version],
    )?;
    expect_one_row(changed, "role")?;
    audit::record(conn, actor, "role.delete", Some("role"), Some(&id), Some(&json!(before)), None)?;
    Ok(())
}

fn role_row_id(conn: &Connection, code: &str) -> Result<String> {
    Ok(conn.query_row("SELECT id FROM role WHERE code = ?1 AND deleted_at IS NULL", [code], |r| r.get(0))?)
}

/// Everyone holding `role_code`: their sessions must pick up changed permissions.
pub fn users_with_role(conn: &Connection, role_code: &str) -> Result<Vec<UserInfo>> {
    let mut stmt = conn.prepare(&format!("{USER_SELECT} AND r.code = ?1"))?;
    let v = stmt.query_map([role_code], map_user)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

fn role_permissions_by_id(conn: &Connection, role_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT permission_code FROM role_permission WHERE role_id = ?1 ORDER BY permission_code",
    )?;
    let v = stmt.query_map([role_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn permissions_of_role(conn: &Connection, role_code: &str) -> Result<HashSet<String>> {
    Ok(role_permissions_by_id(conn, &resolve_role_id(conn, role_code)?)?.into_iter().collect())
}

// ───────────────────────────── passwords ─────────────────────────────

pub const MIN_PASSWORD_LEN: usize = 8;
pub const MAX_FAILED_ATTEMPTS: i64 = 5;
pub const LOCKOUT_MINUTES: i64 = 15;

pub fn hash_password(password: &str) -> Result<String> {
    // Argon2id, OWASP-recommended defaults (19 MiB, t=2, p=1).
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| CoreError::api(ErrorCode::Internal, format!("argon2: {e}")))
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
}

pub fn validate_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(CoreError::invalid(
            "password",
            ValidationRule::PasswordTooShort,
            format!("password must be at least {MIN_PASSWORD_LEN} characters"),
        ));
    }
    Ok(())
}

pub fn validate_username(username: &str) -> Result<()> {
    let ok = (3..=32).contains(&username.len())
        && username.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !ok {
        return Err(CoreError::invalid(
            "username",
            ValidationRule::UsernameFormat,
            "username must be 3–32 characters: letters, digits, . _ -",
        ));
    }
    Ok(())
}

pub fn validate_display_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > 100 {
        return Err(CoreError::invalid(
            "display_name",
            ValidationRule::DisplayNameLength,
            "display name must be 1–100 characters",
        ));
    }
    Ok(())
}

// ───────────────────────────── users ─────────────────────────────

const USER_SELECT: &str = "SELECT u.id, u.username, u.display_name, r.code, u.is_active, u.version, r.label
     FROM app_user u JOIN role r ON r.id = u.role_id WHERE u.deleted_at IS NULL";

fn map_user(r: &rusqlite::Row<'_>) -> rusqlite::Result<UserInfo> {
    Ok(UserInfo {
        id: r.get(0)?,
        username: r.get(1)?,
        display_name: r.get(2)?,
        role: r.get(3)?,
        is_active: r.get::<_, i64>(4)? == 1,
        version: r.get(5)?,
        role_label: r.get(6)?,
    })
}

pub fn get_user(conn: &Connection, id: &str) -> Result<UserInfo> {
    conn.query_row(&format!("{USER_SELECT} AND u.id = ?1"), [id], map_user)
        .optional()?
        .ok_or_else(|| CoreError::api(ErrorCode::NotFound, format!("user {id}")))
}

pub fn list_users(conn: &Connection) -> Result<Vec<UserInfo>> {
    let mut stmt = conn.prepare(&format!("{USER_SELECT} ORDER BY u.username COLLATE NOCASE"))?;
    let v = stmt.query_map([], map_user)?.collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn owner(conn: &Connection) -> Result<UserInfo> {
    conn.query_row(
        &format!("{USER_SELECT} AND r.code = '{OWNER}' ORDER BY u.created_at LIMIT 1"),
        [],
        map_user,
    )
    .optional()?
    .ok_or_else(|| CoreError::api(ErrorCode::NotFound, "owner"))
}

fn ensure_assignable_role(conn: &Connection, role: &str) -> Result<()> {
    if role == OWNER {
        return Err(CoreError::invalid(
            "role",
            ValidationRule::RoleNotAssignable,
            "the owner role cannot be assigned; a clinic has exactly one owner",
        ));
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM role WHERE code = ?1 AND deleted_at IS NULL)",
        [role],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(CoreError::invalid(
            "role",
            ValidationRule::RoleNotAssignable,
            format!("unknown role {role}"),
        ));
    }
    Ok(())
}

pub struct NewUser<'a> {
    pub username: &'a str,
    pub display_name: &'a str,
    pub password: &'a str,
    pub role: &'a str,
}

/// `allow_owner` is only true during first-run setup.
pub fn create_user(conn: &Connection, actor: &Actor, u: NewUser<'_>, allow_owner: bool) -> Result<UserInfo> {
    validate_username(u.username)?;
    validate_display_name(u.display_name)?;
    validate_password(u.password)?;
    if !(allow_owner && u.role == OWNER) {
        ensure_assignable_role(conn, u.role)?;
    }
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM app_user WHERE username = ?1 COLLATE NOCASE AND deleted_at IS NULL)",
        [u.username],
        |r| r.get(0),
    )?;
    if taken {
        return Err(CoreError::invalid("username", ValidationRule::UsernameTaken, "username already exists"));
    }
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO app_user(id, username, display_name, password_hash, role_id, password_changed_at,
                              created_at, created_by, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?6, ?7)",
        params![
            id,
            u.username,
            u.display_name.trim(),
            hash_password(u.password)?,
            resolve_role_id(conn, u.role)?,
            now,
            actor.user_id
        ],
    )?;
    let info = get_user(conn, &id)?;
    audit::record(conn, actor, "user.create", Some("app_user"), Some(&id), None, Some(&json!(info)))?;
    Ok(info)
}

pub fn update_user(
    conn: &Connection,
    actor: &Actor,
    id: &str,
    expected_version: i64,
    display_name: &str,
    role: &str,
    is_active: bool,
) -> Result<UserInfo> {
    validate_display_name(display_name)?;
    let before = get_user(conn, id)?;
    if !is_active && actor.user_id.as_deref() == Some(id) {
        return Err(CoreError::invalid(
            "is_active",
            ValidationRule::SelfLockout,
            "you cannot deactivate your own account",
        ));
    }
    if before.role == OWNER {
        if role != OWNER || !is_active {
            return Err(CoreError::invalid(
                "role",
                ValidationRule::OwnerImmutable,
                "the owner cannot be demoted or deactivated",
            ));
        }
    } else {
        ensure_assignable_role(conn, role)?;
    }
    let changed = conn.execute(
        "UPDATE app_user SET display_name = ?1, role_id = ?2, is_active = ?3,
                updated_at = ?4, updated_by = ?5, version = version + 1
         WHERE id = ?6 AND version = ?7 AND deleted_at IS NULL",
        params![
            display_name.trim(),
            resolve_role_id(conn, role)?,
            is_active as i64,
            now_iso(),
            actor.user_id,
            id,
            expected_version
        ],
    )?;
    expect_one_row(changed, "user")?;
    let after = get_user(conn, id)?;
    audit::record(
        conn,
        actor,
        "user.update",
        Some("app_user"),
        Some(id),
        Some(&json!(before)),
        Some(&json!(after)),
    )?;
    Ok(after)
}

struct Credentials {
    id: String,
    username: String,
    hash: String,
    is_active: bool,
    failed: i64,
    locked_until: Option<String>,
}

fn credentials(conn: &Connection, username: &str) -> Result<Option<Credentials>> {
    Ok(conn
        .query_row(
            "SELECT id, username, password_hash, is_active, failed_attempts, locked_until
             FROM app_user WHERE username = ?1 COLLATE NOCASE AND deleted_at IS NULL",
            [username],
            |r| {
                Ok(Credentials {
                    id: r.get(0)?,
                    username: r.get(1)?,
                    hash: r.get(2)?,
                    is_active: r.get::<_, i64>(3)? == 1,
                    failed: r.get(4)?,
                    locked_until: r.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Verifies credentials with lockout after repeated failures. Every attempt is audited.
pub fn login(conn: &Connection, username: &str, password: &str) -> Result<UserInfo> {
    let Some(c) = credentials(conn, username)? else {
        // Same work and same error as a wrong password: no username probing.
        static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let dummy = DUMMY.get_or_init(|| hash_password("artaveo-timing-equaliser").unwrap_or_default());
        let _ = verify_password(dummy, password);
        audit::record(
            conn,
            &Actor { user_id: None, username: Some(username.into()) },
            "auth.login_failed",
            None,
            None,
            None,
            None,
        )?;
        return Err(CoreError::api(ErrorCode::InvalidCredentials, "invalid username or password"));
    };
    let actor = Actor::user(&c.id, &c.username);
    if let Some(until) = c.locked_until.as_deref().and_then(parse) {
        if until > now() {
            audit::record(conn, &actor, "auth.login_blocked", Some("app_user"), Some(&c.id), None, None)?;
            return Err(CoreError::api(ErrorCode::AccountLocked, format!("locked until {}", iso(until))));
        }
    }
    if !c.is_active || !verify_password(&c.hash, password) {
        let failed = c.failed + 1;
        let lock = (failed >= MAX_FAILED_ATTEMPTS).then(|| iso(now() + Duration::minutes(LOCKOUT_MINUTES)));
        conn.execute(
            "UPDATE app_user SET failed_attempts = ?1, locked_until = ?2 WHERE id = ?3",
            params![if lock.is_some() { 0 } else { failed }, lock, c.id],
        )?;
        audit::record(
            conn,
            &actor,
            "auth.login_failed",
            Some("app_user"),
            Some(&c.id),
            None,
            Some(&json!({"failed_attempts": failed, "locked_until": lock})),
        )?;
        return Err(CoreError::api(ErrorCode::InvalidCredentials, "invalid username or password"));
    }
    conn.execute("UPDATE app_user SET failed_attempts = 0, locked_until = NULL WHERE id = ?1", [&c.id])?;
    audit::record(conn, &actor, "auth.login", Some("app_user"), Some(&c.id), None, None)?;
    get_user(conn, &c.id)
}

pub fn check_password(conn: &Connection, user_id: &str, password: &str) -> Result<bool> {
    let hash: String =
        conn.query_row("SELECT password_hash FROM app_user WHERE id = ?1", [user_id], |r| r.get(0))?;
    Ok(verify_password(&hash, password))
}

/// Sets a new password, clears lockout, bumps version. Caller audits the reason.
pub fn set_password(conn: &Connection, actor: &Actor, user_id: &str, new_password: &str) -> Result<()> {
    validate_password(new_password)?;
    let now = now_iso();
    let changed = conn.execute(
        "UPDATE app_user SET password_hash = ?1, password_changed_at = ?2, failed_attempts = 0, locked_until = NULL,
                updated_at = ?2, updated_by = ?3, version = version + 1
         WHERE id = ?4 AND deleted_at IS NULL",
        params![hash_password(new_password)?, now, actor.user_id, user_id],
    )?;
    expect_one_row(changed, "user")
}

/// An administrator sets a new password for another user (4.1). Clears any lockout; the caller
/// ends the user's sessions.
pub fn reset_password(conn: &Connection, actor: &Actor, id: &str, new_password: &str) -> Result<UserInfo> {
    let user = get_user(conn, id)?;
    set_password(conn, actor, id, new_password).map_err(|e| e.rename_field("new_password"))?;
    audit::record(conn, actor, "user.reset_password", Some("app_user"), Some(id), None, None)?;
    get_user(conn, &user.id)
}

/// Clears a lockout caused by repeated wrong passwords without changing the password.
pub fn unlock_user(conn: &Connection, actor: &Actor, id: &str) -> Result<UserInfo> {
    get_user(conn, id)?;
    conn.execute(
        "UPDATE app_user SET failed_attempts = 0, locked_until = NULL, updated_at = ?1, updated_by = ?2,
                version = version + 1 WHERE id = ?3",
        params![now_iso(), actor.user_id, id],
    )?;
    audit::record(conn, actor, "user.unlock", Some("app_user"), Some(id), None, None)?;
    get_user(conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2id_hash_and_verify() {
        let h = hash_password("correct horse").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify_password(&h, "correct horse"));
        assert!(!verify_password(&h, "wrong horse"));
        assert!(!verify_password("garbage", "x"));
    }

    #[test]
    fn roles_are_subsets_and_owner_has_all() {
        let roles = system_roles();
        assert_eq!(roles.len(), 6);
        for (_, perms) in &roles {
            assert!(perms.iter().all(|p| ALL_PERMISSIONS.contains(p)));
        }
        assert_eq!(roles[0].1.len(), ALL_PERMISSIONS.len());
        assert!(!roles[1].1.contains(&BACKUP_RESTORE), "restore is Owner-only");
    }

    #[test]
    fn username_and_password_rules() {
        assert!(validate_username("dr.ahmad").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("احمد").is_err());
        assert!(validate_password("1234567").is_err());
        assert!(validate_password("۱۲۳۴۵۶۷۸").is_ok());
    }
}
