//! macOS Keychain storage for sensitive credentials (currently the OpenAI API key).
//!
//! Wraps the `keyring` crate so callers see plain `Option<String>` getters and
//! infallible-feeling setters. Entries live in the login keychain under
//! service="yap" and the account name we pass in.

use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "yap";
const OPENAI_KEY_ACCOUNT: &str = "openai_api_key";

fn entry(account: &str) -> Result<Entry> {
    Entry::new(SERVICE, account).context("open keychain entry")
}

pub fn get_openai_api_key() -> Option<String> {
    let entry = entry(OPENAI_KEY_ACCOUNT).ok()?;
    match entry.get_password() {
        Ok(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

pub fn set_openai_api_key(key: &str) -> Result<()> {
    let entry = entry(OPENAI_KEY_ACCOUNT)?;
    entry.set_password(key).context("save api key to keychain")
}

pub fn clear_openai_api_key() -> Result<()> {
    let entry = entry(OPENAI_KEY_ACCOUNT)?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("delete api key"),
    }
}

/// Return the last 4 chars of the stored key, prefixed for display
/// (e.g. "sk-…abcd"). Returns `None` if no key is stored.
pub fn openai_api_key_hint() -> Option<String> {
    let key = get_openai_api_key()?;
    let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    Some(format!("sk-…{tail}"))
}
