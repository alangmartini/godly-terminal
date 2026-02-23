use std::sync::Mutex;

/// Locks the server to the first device that registers.
/// Once a device token is set, all API requests must include it.
pub struct DeviceLock {
    token: Mutex<Option<String>>,
}

impl DeviceLock {
    pub fn new() -> Self {
        Self {
            token: Mutex::new(None),
        }
    }

    /// Try to register a device. Returns the device token on success.
    /// - First caller: generates and stores a token, returns it.
    /// - Subsequent callers: rejected with None.
    pub fn register(&self, candidate_token: &str) -> Result<(), &'static str> {
        let mut token = self.token.lock().unwrap();
        if token.is_some() {
            return Err("Device already registered");
        }
        *token = Some(candidate_token.to_string());
        tracing::info!("Device locked to token: {}...", &candidate_token[..8.min(candidate_token.len())]);
        Ok(())
    }

    /// Check if a request's device token matches the registered device.
    /// Returns true if no device is registered yet (pre-registration) or if the token matches.
    pub fn check(&self, provided: &str) -> bool {
        let token = self.token.lock().unwrap();
        match token.as_deref() {
            None => false, // no device registered yet, must register first
            Some(expected) => expected == provided,
        }
    }

    /// Whether a device has been registered.
    pub fn is_locked(&self) -> bool {
        self.token.lock().unwrap().is_some()
    }

    /// Reset the lock (e.g. for testing or admin reset).
    #[allow(dead_code)]
    pub fn reset(&self) {
        let mut token = self.token.lock().unwrap();
        *token = None;
        tracing::info!("Device lock reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_registration_succeeds() {
        let lock = DeviceLock::new();
        assert!(!lock.is_locked());
        assert!(lock.register("token-abc").is_ok());
        assert!(lock.is_locked());
    }

    #[test]
    fn second_registration_rejected() {
        let lock = DeviceLock::new();
        lock.register("token-abc").unwrap();
        assert!(lock.register("token-xyz").is_err());
    }

    #[test]
    fn check_matches_registered_token() {
        let lock = DeviceLock::new();
        lock.register("token-abc").unwrap();
        assert!(lock.check("token-abc"));
        assert!(!lock.check("token-xyz"));
    }

    #[test]
    fn check_fails_before_registration() {
        let lock = DeviceLock::new();
        assert!(!lock.check("anything"));
    }

    #[test]
    fn reset_allows_new_registration() {
        let lock = DeviceLock::new();
        lock.register("token-abc").unwrap();
        lock.reset();
        assert!(!lock.is_locked());
        assert!(lock.register("token-xyz").is_ok());
        assert!(lock.check("token-xyz"));
    }
}
