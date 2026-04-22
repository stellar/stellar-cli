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
    pub fn new() -> Self {
        Self::default()
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

    pub fn set(key: &'static str, val: &std::path::Path) -> Self {
        let prev = std::env::var(key).ok();
        std::env::set_var(key, val);
        Self(vec![(key.to_string(), prev)])
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
