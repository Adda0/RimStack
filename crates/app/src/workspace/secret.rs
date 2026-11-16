use rimstack_core::base::locks::Held;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::say;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SERVICE: &str = "RimStack";

/// The secrets RimStack keeps, each under its own credential-store name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Secret {
    SteamKey,
    GithubToken,
}

impl Secret {
    fn name(self) -> &'static str {
        match self {
            Self::SteamKey => "Steam Web API key",
            Self::GithubToken => "GitHub token",
        }
    }
}

/// Longer than a keyring takes to be unlocked by its owner; shorter than a missing service that
/// never answers.
const PATIENCE: Duration = Duration::from_secs(30);

pub trait Vault: Send + Sync {
    /// `Ok(None)` when the store works and holds no key.
    fn get(&self, secret: Secret) -> Result<Option<String>, Sentence>;

    fn set(&self, secret: Secret, text: &str) -> Result<(), Sentence>;

    fn delete(&self, secret: Secret) -> Result<(), Sentence>;
}

#[derive(Default)]
pub struct Memory {
    held: Mutex<HashMap<Secret, String>>,
    broken: Option<Sentence>,
}

impl Memory {
    pub fn holding(secret: Secret, text: &str) -> Self {
        Memory {
            held: Mutex::new(HashMap::from([(secret, text.to_string())])),
            broken: None,
        }
    }

    pub fn broken(why: Sentence) -> Self {
        Memory {
            held: Mutex::default(),
            broken: Some(why),
        }
    }

    fn working(&self) -> Result<(), Sentence> {
        self.broken.clone().map_or(Ok(()), Err)
    }
}

impl Vault for Memory {
    fn get(&self, secret: Secret) -> Result<Option<String>, Sentence> {
        self.working()?;
        Ok(self.held.held().get(&secret).cloned())
    }

    fn set(&self, secret: Secret, text: &str) -> Result<(), Sentence> {
        self.working()?;
        self.held.held().insert(secret, text.to_string());
        Ok(())
    }

    fn delete(&self, secret: Secret) -> Result<(), Sentence> {
        self.working()?;
        self.held.held().remove(&secret);
        Ok(())
    }
}

fn unavailable() -> Sentence {
    say!("No credential store is available to keep the key in")
}

fn silent() -> Sentence {
    say!("The credential store did not answer")
}

#[cfg(target_os = "linux")]
fn open() -> keyring_core::Result<Arc<keyring_core::api::CredentialStore>> {
    Ok(zbus_secret_service_keyring_store::Store::new()?)
}

#[cfg(target_os = "macos")]
fn open() -> keyring_core::Result<Arc<keyring_core::api::CredentialStore>> {
    Ok(apple_native_keyring_store::keychain::Store::new()?)
}

#[cfg(target_os = "windows")]
fn open() -> keyring_core::Result<Arc<keyring_core::api::CredentialStore>> {
    Ok(windows_native_keyring_store::Store::new()?)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn open() -> keyring_core::Result<Arc<keyring_core::api::CredentialStore>> {
    Err(keyring_core::Error::NotSupportedByStore(
        "no credential store on this system".to_string(),
    ))
}

fn entry(secret: Secret) -> keyring_core::Result<keyring_core::Entry> {
    open()?.build(SERVICE, secret.name(), None)
}

/// The system's: Secret Service, Keychain or Credential Manager. Each ask runs on a thread of its
/// own, so a store that never answers costs a wait and not the window.
#[derive(Default)]
pub struct System;

fn within<T: Send + 'static>(
    ask: impl FnOnce() -> Result<T, Sentence> + Send + 'static,
) -> Result<T, Sentence> {
    waiting(PATIENCE, ask)
}

fn waiting<T: Send + 'static>(
    patience: Duration,
    ask: impl FnOnce() -> Result<T, Sentence> + Send + 'static,
) -> Result<T, Sentence> {
    let (tx, rx) = std::sync::mpsc::channel();
    let started = std::thread::Builder::new()
        .name("credential store".into())
        .spawn(move || {
            let _ = tx.send(ask());
        });
    if let Err(problem) = started {
        rimstack_core::problem!("credential store: {problem}");
        return Err(unavailable());
    }
    rx.recv_timeout(patience).unwrap_or_else(|_| Err(silent()))
}

fn failed(problem: &keyring_core::Error) -> Sentence {
    rimstack_core::problem!("credential store: {problem}");
    unavailable()
}

impl Vault for System {
    fn get(&self, secret: Secret) -> Result<Option<String>, Sentence> {
        within(move || match entry(secret).and_then(|e| e.get_password()) {
            Ok(text) => Ok(Some(text)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(problem) => Err(failed(&problem)),
        })
    }

    fn set(&self, secret: Secret, text: &str) -> Result<(), Sentence> {
        let text = text.to_string();
        within(move || {
            let kept = entry(secret).and_then(|e| e.set_password(&text));
            kept.map_err(|problem| failed(&problem))
        })
    }

    fn delete(&self, secret: Secret) -> Result<(), Sentence> {
        within(
            move || match entry(secret).and_then(|e| e.delete_credential()) {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(problem) => Err(failed(&problem)),
            },
        )
    }
}

pub fn system() -> Arc<dyn Vault> {
    Arc::new(System)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_that_never_answers_costs_a_wait_not_a_hang() {
        let slow = || {
            std::thread::sleep(Duration::from_millis(500));
            Ok(())
        };
        let started = std::time::Instant::now();
        assert_eq!(waiting(Duration::from_millis(20), slow), Err(silent()));
        assert!(started.elapsed() < Duration::from_millis(400));
        assert_eq!(waiting(Duration::from_secs(5), || Ok(7)), Ok(7));
    }
}
