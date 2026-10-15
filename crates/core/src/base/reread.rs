use std::sync::{Arc, RwLock};

use crate::base::locks::Shared;

pub struct Reread<T> {
    now: RwLock<Option<Arc<T>>>,
}

impl<T> Default for Reread<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Reread<T> {
    pub const fn new() -> Self {
        Self {
            now: RwLock::new(None),
        }
    }

    pub fn get(&self, read: impl FnOnce() -> T) -> Arc<T> {
        let known = self.now.reading().clone();
        if let Some(known) = known {
            return known;
        }
        let fresh = Arc::new(read());
        let mut now = self.now.writing();
        // Two threads may both have read: the first one's stands.
        now.get_or_insert(fresh).clone()
    }

    pub fn put(&self, fresh: T) {
        *self.now.writing() = Some(Arc::new(fresh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_read_once_until_put_again_and_the_old_reading_is_freed() {
        let counted: Reread<usize> = Reread::new();
        let first = counted.get(|| 1);
        assert_eq!(*counted.get(|| unreachable!("read twice")), 1);
        let old = Arc::downgrade(&first);
        counted.put(2);
        assert_eq!(*counted.get(|| unreachable!("read after put")), 2);
        assert!(old.upgrade().is_some());
        drop(first);
        assert!(old.upgrade().is_none());
    }
}
