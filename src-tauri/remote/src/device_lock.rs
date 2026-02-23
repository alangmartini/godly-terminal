use std::sync::Mutex;
use std::time::Instant;

use subtle::ConstantTimeEq;

const MAX_FAILED_ATTEMPTS: u32 = 5;
const LOCKOUT_SECS: u64 = 300; // 5 minutes

/// Locks the server to the first device that registers.
/// Once a device token is set, all API requests must include it.
/// Registration requires a password and is rate-limited.
pub struct DeviceLock {
    token: Mutex<Option<String>>,
    password: Option<String>,
    failed_attempts: Mutex<u32>,
    lockout_until: Mutex<Option<Instant>>,
}

/// Constant-time string comparison to prevent timing attacks.
fn secure_eq(a: &str, b: &str) -> bool {
    // Length comparison leaks length info, but that's acceptable —
    // passwords are fixed-length and tokens are always 64 hex chars.
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

impl DeviceLock {
    pub fn new(password: Option<String>) -> Self {
        Self {
            token: Mutex::new(None),
            password,
            failed_attempts: Mutex::new(0),
            lockout_until: Mutex::new(None),
        }
    }

    /// Check the password. Returns Err if wrong or locked out.
    pub fn verify_password(&self, provided: &str) -> Result<(), &'static str> {
        // Check lockout
        {
            let lockout = self.lockout_until.lock().unwrap();
            if let Some(until) = *lockout {
                if Instant::now() < until {
                    return Err("Too many failed attempts. Try again later.");
                }
            }
        }

        match &self.password {
            None => Ok(()), // no password configured
            Some(expected) if secure_eq(expected, provided) => {
                // Reset failed attempts on success
                *self.failed_attempts.lock().unwrap() = 0;
                *self.lockout_until.lock().unwrap() = None;
                Ok(())
            }
            Some(_) => {
                // Wrong password — increment failures
                let mut attempts = self.failed_attempts.lock().unwrap();
                *attempts += 1;
                if *attempts >= MAX_FAILED_ATTEMPTS {
                    let mut lockout = self.lockout_until.lock().unwrap();
                    *lockout = Some(Instant::now() + std::time::Duration::from_secs(LOCKOUT_SECS));
                    tracing::warn!(
                        "Device registration locked out for {}s after {} failed attempts",
                        LOCKOUT_SECS,
                        attempts
                    );
                }
                Err("Invalid password")
            }
        }
    }

    /// Try to register a device.
    /// - First caller: stores token, returns Ok.
    /// - Subsequent callers: rejected.
    pub fn register(&self, candidate_token: &str) -> Result<(), &'static str> {
        let mut token = self.token.lock().unwrap();
        if token.is_some() {
            return Err("Device already registered");
        }
        *token = Some(candidate_token.to_string());
        tracing::info!("Device registered successfully");
        Ok(())
    }

    /// Check if a request's device token matches the registered device.
    pub fn check(&self, provided: &str) -> bool {
        let token = self.token.lock().unwrap();
        match token.as_deref() {
            None => false,
            Some(expected) => secure_eq(expected, provided),
        }
    }

    /// Whether a device has been registered.
    pub fn is_locked(&self) -> bool {
        self.token.lock().unwrap().is_some()
    }

    /// Whether a password is required for registration.
    pub fn has_password(&self) -> bool {
        self.password.is_some()
    }

    /// Reset the lock (e.g. for testing or admin reset).
    #[allow(dead_code)]
    pub fn reset(&self) {
        *self.token.lock().unwrap() = None;
        *self.failed_attempts.lock().unwrap() = 0;
        *self.lockout_until.lock().unwrap() = None;
        tracing::info!("Device lock reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_registration_succeeds() {
        let lock = DeviceLock::new(None);
        assert!(!lock.is_locked());
        assert!(lock.register("token-abc").is_ok());
        assert!(lock.is_locked());
    }

    #[test]
    fn second_registration_rejected() {
        let lock = DeviceLock::new(None);
        lock.register("token-abc").unwrap();
        assert!(lock.register("token-xyz").is_err());
    }

    #[test]
    fn check_matches_registered_token() {
        let lock = DeviceLock::new(None);
        lock.register("token-abc").unwrap();
        assert!(lock.check("token-abc"));
        assert!(!lock.check("token-xyz"));
    }

    #[test]
    fn check_fails_before_registration() {
        let lock = DeviceLock::new(None);
        assert!(!lock.check("anything"));
    }

    #[test]
    fn reset_allows_new_registration() {
        let lock = DeviceLock::new(None);
        lock.register("token-abc").unwrap();
        lock.reset();
        assert!(!lock.is_locked());
        assert!(lock.register("token-xyz").is_ok());
        assert!(lock.check("token-xyz"));
    }

    #[test]
    fn password_required_when_set() {
        let lock = DeviceLock::new(Some("secret123".into()));
        assert!(lock.has_password());
        assert!(lock.verify_password("wrong").is_err());
        assert!(lock.verify_password("secret123").is_ok());
    }

    #[test]
    fn no_password_always_passes() {
        let lock = DeviceLock::new(None);
        assert!(!lock.has_password());
        assert!(lock.verify_password("anything").is_ok());
    }

    #[test]
    fn lockout_after_max_failures() {
        let lock = DeviceLock::new(Some("secret".into()));
        for _ in 0..MAX_FAILED_ATTEMPTS {
            let _ = lock.verify_password("wrong");
        }
        // Even correct password is rejected during lockout
        assert!(lock.verify_password("secret").is_err());
    }

    #[test]
    fn success_resets_failure_count() {
        let lock = DeviceLock::new(Some("secret".into()));
        // 4 failures (one less than lockout)
        for _ in 0..MAX_FAILED_ATTEMPTS - 1 {
            let _ = lock.verify_password("wrong");
        }
        // Correct password resets counter
        assert!(lock.verify_password("secret").is_ok());
        // Can fail again without immediate lockout
        assert!(lock.verify_password("wrong").is_err());
        assert!(lock.verify_password("secret").is_ok());
    }

    #[test]
    fn timing_safe_comparison() {
        // Verify secure_eq works correctly
        assert!(secure_eq("hello", "hello"));
        assert!(!secure_eq("hello", "world"));
        assert!(!secure_eq("hello", "hell"));
        assert!(!secure_eq("", "a"));
        assert!(secure_eq("", ""));
    }
}
