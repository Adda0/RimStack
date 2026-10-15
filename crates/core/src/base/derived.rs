use std::sync::{Arc, Mutex};

use crate::base::locks::Held;

pub struct Derived<K, T> {
    now: Mutex<Option<(K, Arc<T>)>>,
}

impl<K, T> Default for Derived<K, T> {
    fn default() -> Self {
        Self {
            now: Mutex::new(None),
        }
    }
}

impl<K: PartialEq, T> Derived<K, T> {
    pub fn get(&self, key: K, make: impl FnOnce() -> T) -> Arc<T> {
        let mut now = self.now.held();
        if let Some((made_for, value)) = now.as_ref() {
            if *made_for == key {
                return value.clone();
            }
        }
        let value = Arc::new(make());
        *now = Some((key, value.clone()));
        value
    }

    pub fn put(&self, key: K, value: T) {
        *self.now.held() = Some((key, Arc::new(value)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_made_again_only_when_its_key_changes() {
        let derived: Derived<u64, String> = Derived::default();
        assert_eq!(*derived.get(1, || "first".into()), "first");
        assert_eq!(*derived.get(1, || unreachable!("made twice")), "first");
        assert_eq!(*derived.get(2, || "second".into()), "second");
        derived.put(3, "given".into());
        assert_eq!(
            *derived.get(3, || unreachable!("made though given")),
            "given"
        );
    }
}
