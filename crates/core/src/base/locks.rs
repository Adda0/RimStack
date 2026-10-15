//! A lock taken after its holder panicked is cleared and reported, never refused.

use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait Shared<T> {
    fn reading(&self) -> RwLockReadGuard<'_, T>;
    fn writing(&self) -> RwLockWriteGuard<'_, T>;
}

fn taken_after_a_panic() {
    crate::problem!(
        "a lock was taken after its holder panicked: what it guards may be half changed"
    );
}

impl<T> Shared<T> for RwLock<T> {
    fn reading(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|poisoned| {
            self.clear_poison();
            taken_after_a_panic();
            poisoned.into_inner()
        })
    }

    fn writing(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|poisoned| {
            self.clear_poison();
            taken_after_a_panic();
            poisoned.into_inner()
        })
    }
}

pub trait Held<T> {
    fn held(&self) -> MutexGuard<'_, T>;
}

impl<T> Held<T> for Mutex<T> {
    fn held(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| {
            self.clear_poison();
            taken_after_a_panic();
            poisoned.into_inner()
        })
    }
}

pub(crate) fn quietly<T>(poisoned: PoisonError<T>) -> T {
    poisoned.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_is_still_taken_after_its_holder_panicked() {
        let kept = std::sync::Arc::new((RwLock::new(1), Mutex::new(2)));
        let held = kept.clone();
        let _ = std::thread::spawn(move || {
            let _both = (held.0.write().unwrap(), held.1.lock().unwrap());
            panic!("while holding both");
        })
        .join();
        *kept.0.writing() += 1;
        assert_eq!((*kept.0.reading(), *kept.1.held()), (2, 2));
        assert!(!kept.0.is_poisoned() && !kept.1.is_poisoned());
    }
}
