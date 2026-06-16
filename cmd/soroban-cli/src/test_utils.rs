/// Saves and restores the current working directory on drop.
///
/// Useful in tests that call `set_current_dir` — guarantees cleanup even on panic.
pub struct CwdGuard(std::path::PathBuf);

impl Default for CwdGuard {
    fn default() -> Self {
        Self(std::env::current_dir().unwrap())
    }
}

impl CwdGuard {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Runs `f` with the current working directory automatically restored on return or panic.
pub fn with_cwd_guard<F: FnOnce()>(f: F) {
    let saved = std::env::current_dir().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    let _ = std::env::set_current_dir(saved);
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// Saves and restores environment variables on drop.
///
/// Useful in tests that mutate env vars — guarantees cleanup even on panic.
pub struct EnvGuard(Vec<(String, Option<String>)>);

impl EnvGuard {
    #[must_use]
    pub fn new(vars: &[&str]) -> Self {
        let saved = vars
            .iter()
            .map(|k| (k.to_string(), std::env::var(k).ok()))
            .collect();
        for k in vars {
            std::env::remove_var(k);
        }
        Self(saved)
    }

    #[must_use]
    pub fn set(key: &'static str, val: &std::path::Path) -> Self {
        let prev = std::env::var(key).ok();
        std::env::set_var(key, val);
        Self(vec![(key.to_string(), prev)])
    }
}

/// Runs `f` with the given env vars cleared, then restores them on return or panic.
pub fn with_env_guard<F: FnOnce()>(vars: &[&str], f: F) {
    let guard = EnvGuard::new(vars);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    drop(guard);
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

/// Runs `f` with `key` set to `val`, then restores the original value on return or panic.
pub fn with_env_set<F: FnOnce()>(key: &'static str, val: &std::path::Path, f: F) {
    let guard = EnvGuard::set(key, val);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    drop(guard);
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.0 {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }
}
