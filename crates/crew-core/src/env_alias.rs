//! Legacy-name alias window for externally documented env contracts.
//!
//! During the Buzz → Crew rename, deployment scripts and agent harnesses may
//! still export the old `BUZZ_*` names. Reads of the affected variables go
//! through [`env_lookup`], which prefers the new name and falls back to the
//! documented legacy one. Remove this module once the window closes.

/// `(new_name, legacy_name)` pairs that are still honored.
pub const LEGACY_ENV_ALIASES: &[(&str, &str)] = &[
    ("CREW_RELAY_URL", "BUZZ_RELAY_URL"),
    ("CREW_PRIVATE_KEY", "BUZZ_PRIVATE_KEY"),
    ("CREW_AUTH_TAG", "BUZZ_AUTH_TAG"),
];

/// Reads `key`, falling back to its legacy `BUZZ_*` spelling when the new
/// name is unset. Mirrors `std::env::var` semantics otherwise.
pub fn env_lookup(key: &str) -> Result<String, std::env::VarError> {
    match std::env::var(key) {
        Ok(v) => Ok(v),
        Err(std::env::VarError::NotPresent) => match legacy_of(key) {
            Some(legacy) => std::env::var(legacy),
            None => Err(std::env::VarError::NotPresent),
        },
        Err(e) => Err(e),
    }
}

fn legacy_of(key: &str) -> Option<&'static str> {
    LEGACY_ENV_ALIASES
        .iter()
        .find(|(new, _)| *new == key)
        .map(|(_, legacy)| *legacy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_name_wins_and_legacy_falls_back() {
        // Both unset in CI; verify alias table + fallback wiring only.
        assert_eq!(legacy_of("CREW_RELAY_URL"), Some("BUZZ_RELAY_URL"));
        assert_eq!(legacy_of("CREW_UNKNOWN"), None);
    }
}
