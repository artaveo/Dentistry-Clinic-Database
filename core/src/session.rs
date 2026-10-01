//! Sessions with an inactivity timeout that **locks** the screen (roadmap 1.6):
//! after `timeout` without activity the session stays alive but only
//! `session.unlock` (password) or logout are accepted.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use artaveo_shared::{ErrorCode, SessionState, UserInfo};
use rand::RngCore;

use crate::error::{CoreError, Result};

#[derive(Debug, Clone)]
pub struct Session {
    pub user: UserInfo,
    pub permissions: HashSet<String>,
    pub last_activity: Instant,
    pub locked: bool,
}

impl Session {
    pub fn require(&self, permission: &str) -> Result<()> {
        if self.permissions.contains(permission) {
            Ok(())
        } else {
            Err(CoreError::forbidden(permission))
        }
    }
}

#[derive(Default)]
pub struct SessionStore {
    sessions: HashMap<String, Session>,
}

impl SessionStore {
    pub fn create(&mut self, user: UserInfo, permissions: HashSet<String>) -> String {
        let mut raw = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let token = hex::encode(raw);
        self.sessions.insert(
            token.clone(),
            Session { user, permissions, last_activity: Instant::now(), locked: false },
        );
        token
    }

    /// Looks up a session and applies the idle-lock rule.
    pub fn get(&mut self, token: Option<&str>, timeout: Duration) -> Result<&mut Session> {
        let token = token.ok_or_else(|| CoreError::api(ErrorCode::Unauthenticated, "no session token"))?;
        let s = self
            .sessions
            .get_mut(token)
            .ok_or_else(|| CoreError::api(ErrorCode::SessionExpired, "unknown or expired session"))?;
        if !s.locked && s.last_activity.elapsed() >= timeout {
            s.locked = true;
            tracing::info!(user = %s.user.username, "session locked after inactivity");
        }
        Ok(s)
    }

    pub fn remove(&mut self, token: &str) -> Option<Session> {
        self.sessions.remove(token)
    }

    /// Ends every session of a user (deactivation, password reset).
    pub fn remove_user(&mut self, user_id: &str) {
        self.sessions.retain(|_, s| s.user.id != user_id);
    }

    /// Refreshes the cached profile/permissions after an admin edit.
    pub fn refresh_user(&mut self, user: &UserInfo, permissions: &HashSet<String>) {
        for s in self.sessions.values_mut().filter(|s| s.user.id == user.id) {
            s.user = user.clone();
            s.permissions = permissions.clone();
        }
    }

    #[cfg(test)]
    pub fn age(&mut self, token: &str, by: Duration) {
        if let Some(s) = self.sessions.get_mut(token) {
            s.last_activity -= by;
        }
    }
}

pub fn state_of(s: &Session, timeout: Duration) -> SessionState {
    let left = timeout.saturating_sub(s.last_activity.elapsed());
    SessionState { locked: s.locked, idle_seconds_left: if s.locked { 0 } else { left.as_secs() as u32 } }
}
